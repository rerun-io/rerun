use std::{collections::VecDeque, ops::RangeBounds, sync::atomic::Ordering};

use itertools::Itertools;
use smallvec::SmallVec;

use re_log::trace;
use re_log_types::{
    DataCell, EntityPath, EntityPathHash, RowId, TimeInt, TimePoint, TimeRange, Timeline,
};
use re_types_core::{ComponentName, ComponentNameSet};

use crate::{DataStore, IndexedBucket, IndexedBucketInner, IndexedTable, PersistentIndexedTable};

// --- Queries ---

/// A query at a given time, for a given timeline.
///
/// Get the latest version of the data available at this time.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct LatestAtQuery {
    pub timeline: Timeline,
    pub at: TimeInt,
}

impl std::fmt::Debug for LatestAtQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "<latest at {} on {:?} (including timeless)>",
            self.timeline.typ().format_utc(self.at),
            self.timeline.name(),
        ))
    }
}

impl LatestAtQuery {
    pub const fn new(timeline: Timeline, at: TimeInt) -> Self {
        Self { timeline, at }
    }

    pub const fn latest(timeline: Timeline) -> Self {
        Self {
            timeline,
            at: TimeInt::MAX,
        }
    }
}

/// A query over a time range, for a given timeline.
///
/// Get all the data within this time interval, plus the latest one before the start of the
/// interval.
///
/// Motivation: all data is considered alive until the next logging to the same component path.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RangeQuery {
    pub timeline: Timeline,
    pub range: TimeRange,
}

impl std::fmt::Debug for RangeQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "<ranging from {} to {} (all inclusive) on {:?} ({} timeless)>",
            self.timeline.typ().format_utc(self.range.min),
            self.timeline.typ().format_utc(self.range.max),
            self.timeline.name(),
            if self.range.min == TimeInt::MIN {
                "including"
            } else {
                "excluding"
            }
        ))
    }
}

impl RangeQuery {
    pub const fn new(timeline: Timeline, range: TimeRange) -> Self {
        Self { timeline, range }
    }
}

// --- Data store ---

impl DataStore {
    /// Retrieve all the [`ComponentName`]s that have been written to for a given [`EntityPath`] on
    /// a specific [`Timeline`].
    ///
    /// # Temporal semantics
    ///
    /// In addition to the temporal results, this also includes all [`ComponentName`]s present in
    /// the timeless tables for this entity.
    pub fn all_components(
        &self,
        timeline: &Timeline,
        ent_path: &EntityPath,
    ) -> Option<Vec<ComponentName>> {
        re_tracing::profile_function!();

        // TODO(cmc): kind & query_id need to somehow propagate through the span system.
        self.query_id.fetch_add(1, Ordering::Relaxed);

        let ent_path_hash = ent_path.hash();

        trace!(
            kind = "all_components",
            id = self.query_id.load(Ordering::Relaxed),
            timeline = ?timeline,
            entity = %ent_path,
            "query started…"
        );

        let timeless: Option<ComponentNameSet> = self
            .timeless_tables
            .get(&ent_path_hash)
            .map(|table| table.columns.keys().cloned().collect());

        let temporal = self
            .tables
            .get(&(*timeline, ent_path_hash))
            .map(|table| &table.all_components);

        let components = match (timeless, temporal) {
            (None, Some(temporal)) => temporal.iter().cloned().collect_vec(),
            (Some(timeless), None) => timeless.iter().cloned().collect_vec(),
            (Some(timeless), Some(temporal)) => timeless.union(temporal).cloned().collect_vec(),
            (None, None) => return None,
        };

        trace!(
            kind = "latest_components_at",
            id = self.query_id.load(Ordering::Relaxed),
            timeline = ?timeline,
            entity = %ent_path,
            ?components,
            "found components"
        );

        Some(components)
    }

    /// Check whether a given entity has a specific [`ComponentName`] on the specified timeline.
    ///
    /// # Temporal semantics
    ///
    /// In addition to the temporal results, this also checks whether the [`ComponentName`] is present
    /// in timeless table.
    pub fn entity_has_component(
        &self,
        timeline: &Timeline,
        ent_path: &EntityPath,
        component: &ComponentName,
    ) -> bool {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        let ent_path_hash = ent_path.hash();

        // First see if the component exists in the timeless table
        if self
            .timeless_tables
            .get(&ent_path_hash)
            .map_or(false, |table| table.columns.contains_key(component))
        {
            return true;
        }

        // Otherwise see if it exists in the specified timeline
        self.tables
            .get(&(*timeline, ent_path_hash))
            .map_or(false, |table| table.all_components.contains(component))
    }

    /// Find the earliest time at which something was logged for a given entity on the specified
    /// timeline.
    ///
    /// # Temporal semantics
    ///
    /// Only considers temporal results—timeless data is ignored.
    pub fn entity_min_time(&self, timeline: &Timeline, ent_path: &EntityPath) -> Option<TimeInt> {
        let ent_path_hash = ent_path.hash();

        let min_time = self
            .tables
            .get(&(*timeline, ent_path_hash))?
            .buckets
            .first_key_value()?
            .1
            .inner
            .read()
            .time_range
            .min;

        // handle case where no data was logged
        if min_time == TimeInt::MIN {
            None
        } else {
            Some(min_time)
        }
    }

    /// Queries the datastore for the cells of the specified `components`, as seen from the point
    /// of view of the so-called `primary` component.
    ///
    /// Returns an array of [`DataCell`]s on success, or `None` otherwise.
    /// Success is defined by one thing and thing only: whether a cell could be found for the
    /// `primary` component.
    /// The presence or absence of secondary components has no effect on the success criteria.
    ///
    /// # Temporal semantics
    ///
    /// Temporal indices take precedence, then timeless tables are queried to fill the holes left
    /// by missing temporal data.
    ///
    /// ## Example
    ///
    /// The following example demonstrate how to fetch the latest cells for a given component
    /// and its associated cluster key, and wrap the result into a nice-to-work-with polars's
    /// dataframe.
    ///
    /// ```rust
    /// # use polars_core::{prelude::*, series::Series};
    /// # use re_log_types::{EntityPath, RowId, TimeInt};
    /// # use re_types_core::{ComponentName};
    /// # use re_arrow_store::{DataStore, LatestAtQuery, RangeQuery};
    /// #
    /// pub fn latest_component(
    ///     store: &DataStore,
    ///     query: &LatestAtQuery,
    ///     ent_path: &EntityPath,
    ///     primary: ComponentName,
    /// ) -> anyhow::Result<DataFrame> {
    ///     let cluster_key = store.cluster_key();
    ///
    ///     let components = &[cluster_key, primary];
    ///     let (_, cells) = store
    ///         .latest_at(&query, ent_path, primary, components)
    ///         .unwrap_or((RowId::ZERO, [(); 2].map(|_| None)));
    ///
    ///     let series: Result<Vec<_>, _> = cells
    ///         .iter()
    ///         .flatten()
    ///         .map(|cell| {
    ///             Series::try_from((
    ///                 cell.component_name().as_str(),
    ///                 cell.to_arrow(),
    ///             ))
    ///         })
    ///         .collect();
    ///
    ///     DataFrame::new(series?).map_err(Into::into)
    /// }
    /// ```
    ///
    /// Thanks to the cluster key, one is free to repeat this process as many times as they wish,
    /// then reduce the resulting dataframes down to one by joining them as they see fit.
    /// This is what our `latest_components` polars helper does.
    ///
    /// For more information about working with dataframes, see the `polars` feature.
    pub fn latest_at<const N: usize>(
        &self,
        query: &LatestAtQuery,
        ent_path: &EntityPath,
        primary: ComponentName,
        components: &[ComponentName; N],
    ) -> Option<(RowId, [Option<DataCell>; N])> {
        // TODO(cmc): kind & query_id need to somehow propagate through the span system.
        self.query_id.fetch_add(1, Ordering::Relaxed);

        let ent_path_hash = ent_path.hash();

        trace!(
            kind = "latest_at",
            id = self.query_id.load(Ordering::Relaxed),
            query = ?query,
            entity = %ent_path,
            %primary,
            ?components,
            "query started…"
        );

        let cells = self
            .tables
            .get(&(query.timeline, ent_path_hash))
            .and_then(|table| {
                let cells = table.latest_at(query.at, primary, components);
                trace!(
                    kind = "latest_at",
                    query = ?query,
                    entity = %ent_path,
                    %primary,
                    ?components,
                    timeless = false,
                    "row cells fetched"
                );
                cells
            });

        // If we've found everything we were looking for in the temporal table, then we can
        // return the results immediately.
        if cells
            .as_ref()
            .map_or(false, |(_, cells)| cells.iter().all(Option::is_some))
        {
            return cells;
        }

        let cells_timeless = self.timeless_tables.get(&ent_path_hash).and_then(|table| {
            let cells = table.latest_at(primary, components);
            trace!(
                kind = "latest_at",
                query = ?query,
                entity = %ent_path,
                %primary,
                ?components,
                ?cells,
                timeless = true,
                "cells fetched"
            );
            cells
        });

        // Otherwise, let's see what's in the timeless table, and then..:
        match (cells, cells_timeless) {
            // nothing in the timeless table: return those partial cells we got.
            (Some(cells), None) => return Some(cells),
            // no temporal cells, but some timeless ones: return those as-is.
            (None, Some(cells_timeless)) => return Some(cells_timeless),
            // we have both temporal & timeless cells: let's merge the two when it makes sense
            // and return the end result.
            (Some((row_id, mut cells)), Some((_, cells_timeless))) => {
                for (i, row_idx) in cells_timeless.into_iter().enumerate() {
                    if cells[i].is_none() {
                        cells[i] = row_idx;
                    }
                }
                return Some((row_id, cells));
            }
            // no cells at all.
            (None, None) => {}
        }

        trace!(
            kind = "latest_at",
            query = ?query,
            entity = %ent_path,
            %primary,
            ?components,
            "primary component not found"
        );

        None
    }

    /// Iterates the datastore in order to return the cells of the specified `components`,
    /// as seen from the point of view of the so-called `primary` component, for the given time
    /// range.
    ///
    /// For each and every relevant row that is found, the returned iterator will yield an array
    /// that is filled with the cells of each and every component in `components`, or `None` if
    /// said component is not available in that row.
    /// A row is considered iff it contains data for the `primary` component.
    ///
    /// This method cannot fail! If there's no data to return, an empty iterator is returned.
    ///
    /// ⚠ Contrary to latest-at queries, range queries can and will yield multiple rows for a
    /// single timestamp if that timestamp happens to hold multiple entries for the `primary`
    /// component.
    /// On the contrary, they won't yield any rows that don't contain an actual value for the
    /// `primary` component, _even if said rows do contain a value for one the secondaries_!
    ///
    /// # Temporal semantics
    ///
    /// Yields the contents of the temporal indices.
    /// Iff the query's time range starts at `TimeInt::MIN`, this will yield the contents of the
    /// timeless tables before anything else.
    ///
    /// When yielding timeless entries, the associated time will be `None`.
    ///
    /// ## Example
    ///
    /// The following example demonstrate how to range over the cells of a given
    /// component and its associated cluster key, and turn the results into a nice-to-work-with
    /// iterator of polars's dataframe.
    /// Additionally, it yields the latest-at state of the component at the start of the time range,
    /// if available.
    ///
    /// ```rust
    /// # use arrow2::array::Array;
    /// # use polars_core::{prelude::*, series::Series};
    /// # use re_log_types::{DataCell, EntityPath, RowId, TimeInt};
    /// # use re_arrow_store::{DataStore, LatestAtQuery, RangeQuery};
    /// # use re_types_core::ComponentName;
    /// #
    /// # pub fn dataframe_from_cells<const N: usize>(
    /// #     cells: [Option<DataCell>; N],
    /// # ) -> anyhow::Result<DataFrame> {
    /// #     let series: Result<Vec<_>, _> = cells
    /// #         .iter()
    /// #         .flatten()
    /// #         .map(|cell| {
    /// #             Series::try_from((
    /// #                 cell.component_name().as_ref(),
    /// #                 cell.to_arrow(),
    /// #             ))
    /// #         })
    /// #         .collect();
    /// #
    /// #     DataFrame::new(series?).map_err(Into::into)
    /// # }
    /// #
    /// pub fn range_component<'a>(
    ///     store: &'a DataStore,
    ///     query: &'a RangeQuery,
    ///     ent_path: &'a EntityPath,
    ///     primary: ComponentName,
    /// ) -> impl Iterator<Item = anyhow::Result<(Option<TimeInt>, DataFrame)>> + 'a {
    ///     let cluster_key = store.cluster_key();
    ///
    ///     let components = [cluster_key, primary];
    ///
    ///     // Fetch the latest-at data just before the start of the time range.
    ///     let latest_time = query.range.min.as_i64().saturating_sub(1).into();
    ///     let df_latest = {
    ///         let query = LatestAtQuery::new(query.timeline, latest_time);
    ///         let (_, cells) = store
    ///             .latest_at(&query, ent_path, primary, &components)
    ///             .unwrap_or((RowId::ZERO, [(); 2].map(|_| None)));
    ///         dataframe_from_cells(cells)
    ///     };
    ///
    ///     // Send the latest-at state before anything else..
    ///     std::iter::once(df_latest.map(|df| (Some(latest_time), df)))
    ///         // ..but only if it's not an empty dataframe.
    ///         .filter(|df| df.as_ref().map_or(true, |(_, df)| !df.is_empty()))
    ///         .chain(store.range(query, ent_path, components).map(
    ///             move |(time, _, cells)| dataframe_from_cells(cells).map(|df| (time, df))
    ///         ))
    /// }
    /// ```
    ///
    /// Thanks to the cluster key, one is free to repeat this process as many times as they wish,
    /// then join the resulting streams to yield a full-fledged dataframe for every update of the
    /// primary component.
    /// This is what our `range_components` polars helper does.
    ///
    /// For more information about working with dataframes, see the `polars` feature.
    pub fn range<'a, const N: usize>(
        &'a self,
        query: &RangeQuery,
        ent_path: &EntityPath,
        components: [ComponentName; N],
    ) -> impl Iterator<Item = (Option<TimeInt>, RowId, [Option<DataCell>; N])> + 'a {
        // Beware! This merely measures the time it takes to gather all the necessary metadata
        // for building the returned iterator.
        re_tracing::profile_function!();

        // TODO(cmc): kind & query_id need to somehow propagate through the span system.
        self.query_id.fetch_add(1, Ordering::Relaxed);

        let ent_path_hash = ent_path.hash();

        trace!(
            kind = "range",
            id = self.query_id.load(Ordering::Relaxed),
            query = ?query,
            entity = %ent_path,
            ?components,
            "query started…"
        );

        let temporal = self
            .tables
            .get(&(query.timeline, ent_path_hash))
            .map(|index| index.range(query.range, components))
            .into_iter()
            .flatten()
            .map(|(time, row_id, cells)| (Some(time), row_id, cells));

        if query.range.min == TimeInt::MIN {
            let timeless = self
                .timeless_tables
                .get(&ent_path_hash)
                .map(|index| {
                    index
                        .range(components)
                        .map(|(row_id, cells)| (None, row_id, cells))
                })
                .into_iter()
                .flatten();
            itertools::Either::Left(timeless.chain(temporal))
        } else {
            itertools::Either::Right(temporal)
        }
    }

    #[inline]
    pub fn get_msg_metadata(&self, row_id: &RowId) -> Option<&(TimePoint, EntityPathHash)> {
        self.metadata_registry.get(row_id)
    }

    /// Sort all unsorted indices in the store.
    #[inline]
    pub fn sort_indices_if_needed(&mut self) {
        for index in self.tables.values_mut() {
            index.sort_indices_if_needed();
        }
    }
}

// --- Temporal ---

impl IndexedTable {
    /// Queries the table for the cells of the specified `components`, as seen from the point
    /// of view of the so-called `primary` component.
    ///
    /// Returns an array of [`DataCell`]s on success, or `None` iff no cell could be found for
    /// the `primary` component.
    pub fn latest_at<const N: usize>(
        &self,
        time: TimeInt,
        primary: ComponentName,
        components: &[ComponentName; N],
    ) -> Option<(RowId, [Option<DataCell>; N])> {
        // Early-exit if this entire table is unaware of this component.
        if !self.all_components.contains(&primary) {
            return None;
        }

        let timeline = self.timeline;

        // The time we're looking for gives us an upper bound: all components must be indexed
        // in either this bucket _or any of those that come before_!
        //
        // That is because secondary columns allow for null values, which forces us to not only
        // walk backwards within an indexed bucket, but sometimes even walk backwards across
        // multiple indexed buckets within the same table!

        let buckets = self
            .range_buckets_rev(..=time)
            .map(|(_, bucket)| bucket)
            .enumerate();
        for (attempt, bucket) in buckets {
            trace!(
                kind = "latest_at",
                timeline = %timeline.name(),
                time = timeline.typ().format_utc(time),
                %primary,
                ?components,
                attempt,
                bucket_time_range = timeline.typ().format_range_utc(bucket.inner.read().time_range),
                "found candidate bucket"
            );
            if let cells @ Some(_) = bucket.latest_at(time, primary, components) {
                return cells; // found at least the primary component!
            }
        }

        None // primary component not found
    }

    /// Iterates the table in order to return the cells of the specified `components`,
    /// as seen from the point of view of the so-called `primary` component, for the given time
    /// range.
    ///
    /// For each and every relevant row that is found, the returned iterator will yield an array
    /// that is filled with the cells of each and every component in `components`, or `None` if
    /// said component is not available in that row.
    /// A row is considered iff it contains data for the `primary` component.
    ///
    /// This method cannot fail! If there's no data to return, an empty iterator is returned.
    pub fn range<const N: usize>(
        &self,
        time_range: TimeRange,
        components: [ComponentName; N],
    ) -> impl Iterator<Item = (TimeInt, RowId, [Option<DataCell>; N])> + '_ {
        // Beware! This merely measures the time it takes to gather all the necessary metadata
        // for building the returned iterator.
        re_tracing::profile_function!();

        let timeline = self.timeline;

        // We need to find the _indexing time_ that corresponds to this time range's minimum bound!
        let (time_range_min, _) = self.find_bucket(time_range.min);

        self.range_buckets(time_range_min..=time_range.max)
            .map(|(_, bucket)| bucket)
            .enumerate()
            .flat_map(move |(bucket_nr, bucket)| {
                trace!(
                    kind = "range",
                    bucket_nr,
                    bucket_time_range =
                        timeline.typ().format_range_utc(bucket.inner.read().time_range),
                    timeline = %timeline.name(),
                    ?time_range,
                    ?components,
                    "found bucket in range"
                );

                bucket.range(time_range, components)
            })
    }

    /// Returns the indexed bucket whose time range covers the given `time`.
    ///
    /// In addition to returning a reference to the `IndexedBucket` itself, this also returns its
    /// _indexing time_, which is different from its minimum time range bound!
    ///
    /// See [`IndexedTable::buckets`] for more information.
    pub fn find_bucket(&self, time: TimeInt) -> (TimeInt, &IndexedBucket) {
        // This cannot fail, `iter_bucket` is guaranteed to always yield at least one bucket,
        // since indexed tables always spawn with a default bucket that covers [-∞;+∞].
        self.range_buckets_rev(..=time).next().unwrap()
    }

    /// Returns the indexed bucket whose time range covers the given `time`.
    ///
    /// In addition to returning a reference to the `IndexedBucket` itself, this also returns its
    /// _indexing time_, which is different from its minimum time range bound!
    ///
    /// See [`IndexedTable::buckets`] for more information.
    pub fn find_bucket_mut(&mut self, time: TimeInt) -> (TimeInt, &mut IndexedBucket) {
        // This cannot fail, `iter_bucket_mut` is guaranteed to always yield at least one bucket,
        // since indexed tables always spawn with a default bucket that covers [-∞;+∞].
        self.range_bucket_rev_mut(..=time).next().unwrap()
    }

    /// Returns an iterator that is guaranteed to yield at least one bucket, which is the bucket
    /// whose time range covers the start bound of the given `time_range`.
    ///
    /// It then continues yielding buckets until it runs out, in increasing time range order.
    ///
    /// In addition to yielding references to the `IndexedBucket`s themselves, this also returns
    /// their _indexing times_, which are different from their minimum time range bounds!
    ///
    /// See [`IndexedTable::buckets`] for more information.
    pub fn range_buckets(
        &self,
        time_range: impl RangeBounds<TimeInt>,
    ) -> impl Iterator<Item = (TimeInt, &IndexedBucket)> {
        // Beware! This merely measures the time it takes to gather all the necessary metadata
        // for building the returned iterator.
        re_tracing::profile_function!();

        self.buckets
            .range(time_range)
            .map(|(time, bucket)| (*time, bucket))
    }

    /// Returns an iterator that is guaranteed to yield at least one bucket, which is the bucket
    /// whose time range covers the end bound of the given `time_range`.
    ///
    /// It then continues yielding buckets until it runs out, in decreasing time range order.
    ///
    /// In addition to yielding references to the `IndexedBucket`s themselves, this also returns
    /// their _indexing times_, which are different from their minimum time range bounds!
    ///
    /// See [`IndexedTable::buckets`] for more information.
    pub fn range_buckets_rev(
        &self,
        time_range: impl RangeBounds<TimeInt>,
    ) -> impl Iterator<Item = (TimeInt, &IndexedBucket)> {
        // Beware! This merely measures the time it takes to gather all the necessary metadata
        // for building the returned iterator.
        re_tracing::profile_function!();

        self.buckets
            .range(time_range)
            .rev()
            .map(|(time, bucket)| (*time, bucket))
    }

    /// Returns an iterator that is guaranteed to yield at least one bucket, which is the bucket
    /// whose time range covers the end bound of the given `time_range`.
    ///
    /// It then continues yielding buckets until it runs out, in decreasing time range order.
    ///
    /// In addition to yielding references to the `IndexedBucket`s themselves, this also returns
    /// their _indexing times_, which are different from their minimum time range bounds!
    ///
    /// See [`IndexedTable::buckets`] for more information.
    pub fn range_bucket_rev_mut(
        &mut self,
        time_range: impl RangeBounds<TimeInt>,
    ) -> impl Iterator<Item = (TimeInt, &mut IndexedBucket)> {
        self.buckets
            .range_mut(time_range)
            .rev()
            .map(|(time, bucket)| (*time, bucket))
    }

    /// Sort all unsorted indexed buckets in this table.
    pub fn sort_indices_if_needed(&self) {
        for bucket in self.buckets.values() {
            bucket.sort_indices_if_needed();
        }
    }
}

impl IndexedBucket {
    /// Sort all component indices by time, provided that's not already the case.
    pub fn sort_indices_if_needed(&self) {
        if self.inner.read().is_sorted {
            return; // early read-only exit
        }

        re_tracing::profile_scope!("sort");
        self.inner.write().sort();
    }

    /// Queries the bucket for the cells of the specified `components`, as seen from the point
    /// of view of the so-called `primary` component.
    ///
    /// Returns an array of [`DataCell`]s on success, or `None` iff no cell could be found for
    /// the `primary` component.
    pub fn latest_at<const N: usize>(
        &self,
        time: TimeInt,
        primary: ComponentName,
        components: &[ComponentName; N],
    ) -> Option<(RowId, [Option<DataCell>; N])> {
        self.sort_indices_if_needed();

        let IndexedBucketInner {
            is_sorted,
            time_range: _,
            col_time,
            col_insert_id: _,
            col_row_id,
            newest_row_id: _,
            col_num_instances: _,
            columns,
            size_bytes: _,
        } = &*self.inner.read();
        debug_assert!(is_sorted);

        // Early-exit if this bucket is unaware of this component.
        let column = columns.get(&primary)?;

        trace!(
            kind = "latest_at",
            %primary,
            ?components,
            timeline = %self.timeline.name(),
            time = self.timeline.typ().format_utc(time),
            "searching for primary & secondary cells…"
        );

        let time_row_nr = col_time.partition_point(|t| *t <= time.as_i64()) as i64;

        // The partition point is always _beyond_ the index that we're looking for.
        // A partition point of 0 thus means that we're trying to query for data that lives
        // _before_ the beginning of time… there's nothing to be found there.
        if time_row_nr == 0 {
            return None;
        }

        // The partition point is always _beyond_ the index that we're looking for; we need
        // to step back to find what we came for.
        let primary_row_nr = time_row_nr - 1;
        trace!(
            kind = "latest_at",
            %primary,
            ?components,
            timeline = %self.timeline.name(),
            time = self.timeline.typ().format_utc(time),
            %primary_row_nr,
            "found primary row number",
        );

        // find the secondary row number, and the associated cells.
        let mut secondary_row_nr = primary_row_nr;
        while column[secondary_row_nr as usize].is_none() {
            if secondary_row_nr == 0 {
                trace!(
                    kind = "latest_at",
                    %primary,
                    ?components,
                    timeline = %self.timeline.name(),
                    time = self.timeline.typ().format_utc(time),
                    %primary_row_nr,
                    "no secondary row number found",
                );
                return None;
            }
            secondary_row_nr -= 1;
        }

        trace!(
            kind = "latest_at",
            %primary,
            ?components,
            timeline = %self.timeline.name(),
            time = self.timeline.typ().format_utc(time),
            %primary_row_nr, %secondary_row_nr,
            "found secondary row number",
        );
        debug_assert!(column[secondary_row_nr as usize].is_some());

        let mut cells = [(); N].map(|_| None);
        for (i, component) in components.iter().enumerate() {
            if let Some(column) = columns.get(component) {
                if let Some(cell) = &column[secondary_row_nr as usize] {
                    trace!(
                        kind = "latest_at",
                        %primary,
                        %component,
                        timeline = %self.timeline.name(),
                        time = self.timeline.typ().format_utc(time),
                        %primary_row_nr, %secondary_row_nr,
                        "found cell",
                    );
                    cells[i] = Some(cell.clone() /* shallow */);
                }
            }
        }

        Some((col_row_id[secondary_row_nr as usize], cells))
    }

    /// Iterates the bucket in order to return the cells of the specified `components`,
    /// as seen from the point of view of the so-called `primary` component, for the given time
    /// range.
    ///
    /// For each and every relevant row that is found, the returned iterator will yield an array
    /// that is filled with the cells of each and every component in `components`, or `None` if
    /// said component is not available in that row.
    /// A row is considered iff it contains data for the `primary` component.
    ///
    /// This method cannot fail! If there's no data to return, an empty iterator is returned.
    pub fn range<const N: usize>(
        &self,
        time_range: TimeRange,
        components: [ComponentName; N],
    ) -> impl Iterator<Item = (TimeInt, RowId, [Option<DataCell>; N])> + '_ {
        self.sort_indices_if_needed();

        let IndexedBucketInner {
            is_sorted,
            time_range: bucket_time_range,
            col_time,
            col_insert_id: _,
            col_row_id,
            newest_row_id: _,
            col_num_instances: _,
            columns,
            size_bytes: _,
        } = &*self.inner.read();
        debug_assert!(is_sorted);

        let bucket_time_range = *bucket_time_range;

        // Early-exit if this bucket is unaware of any of our components of interest.
        if components
            .iter()
            .all(|component| columns.get(component).is_none())
        {
            return itertools::Either::Right(std::iter::empty());
        }

        // Beware! This merely measures the time it takes to gather all the necessary metadata
        // for building the returned iterator.
        re_tracing::profile_function!();

        trace!(
            kind = "range",
            bucket_time_range = self.timeline.typ().format_range_utc(bucket_time_range),
            ?components,
            timeline = %self.timeline.name(),
            time_range = self.timeline.typ().format_range_utc(time_range),
            "searching for time & component cell numbers…"
        );

        let time_row_nr = col_time.partition_point(|t| *t < time_range.min.as_i64()) as u64;

        trace!(
            kind = "range",
            bucket_time_range = self.timeline.typ().format_range_utc(bucket_time_range),
            ?components,
            timeline = %self.timeline.name(),
            time_range = self.timeline.typ().format_range_utc(time_range),
            %time_row_nr,
            "found time row number",
        );

        // TODO(cmc): Cloning these is obviously not great and will need to be addressed at
        // some point.
        // But, really, it's not _that_ bad either: these are either integers or erased pointers,
        // and e.g. with the default configuration there are only 1024 of them (times the number
        // of components).
        let col_time = col_time.clone();
        let col_row_id = col_row_id.clone();
        let mut columns = columns.clone(); // shallow

        // We have found the index of the first row that possibly contains data for any single one
        // of the components we're interested in.
        //
        // Now we need to iterate through every remaining rows in the bucket and yield any that
        // contains data for these components and is still within the time range.
        let cells = col_time
            .into_iter()
            .skip(time_row_nr as usize)
            // don't go beyond the time range we're interested in!
            .filter(move |time| time_range.contains((*time).into()))
            .enumerate()
            .filter_map(move |(time_row_offset, time)| {
                let row_nr = time_row_nr + time_row_offset as u64;

                let mut cells = [(); N].map(|_| None);
                for (i, component) in components.iter().enumerate() {
                    if let Some(column) = columns.get_mut(component) {
                        cells[i] = column[row_nr as usize].take();
                    }
                }

                // We only yield rows that contain data for at least one of the components of
                // interest.
                if cells.iter().all(Option::is_none) {
                    return None;
                }

                let row_id = col_row_id[row_nr as usize];

                trace!(
                    kind = "range",
                    bucket_time_range =
                        self.timeline.typ().format_range_utc(bucket_time_range),
                    ?components,
                    timeline = %self.timeline.name(),
                    time_range = self.timeline.typ().format_range_utc(time_range),
                    %row_nr,
                    %row_id,
                    ?cells,
                    "yielding cells",
                );

                Some((time.into(), row_id, cells))
            });

        itertools::Either::Left(cells)
    }

    /// Whether the indices in this `IndexedBucket` are sorted
    pub fn is_sorted(&self) -> bool {
        self.inner.read().is_sorted
    }
}

impl IndexedBucketInner {
    pub fn sort(&mut self) {
        let Self {
            is_sorted,
            time_range: _,
            col_time,
            col_insert_id,
            col_row_id,
            newest_row_id: _,
            col_num_instances,
            columns,
            size_bytes: _,
        } = self;

        if *is_sorted {
            return;
        }

        re_tracing::profile_function!(); // TODO: too costly

        let swaps = {
            // re_tracing::profile_scope!("swaps"); // TODO: too costly
            let mut swaps = (0..col_time.len()).collect::<Vec<_>>();
            // NOTE: Within a single timestamp, we must use the Row ID as tie-breaker!
            // The Row ID is how we define ordering within a client's thread, and our public APIs
            // guarantee that logging order is respected within a single thread!
            swaps.sort_by_key(|&i| (&col_time[i], &col_row_id[i]));
            swaps
                .iter()
                .copied()
                .enumerate()
                .map(|(to, from)| (from, to))
                .collect::<Vec<_>>()
        };

        // Yep, the reshuffle implementation is very dumb and very slow :)
        // TODO(#442): re_datastore: implement efficient shuffling on the read path.

        {
            // re_tracing::profile_scope!("control"); // TODO: too costly

            fn reshuffle_control_column<T: Copy>(
                column: &mut VecDeque<T>,
                swaps: &[(usize, usize)],
            ) {
                let source = {
                    // re_tracing::profile_scope!("clone"); // TODO: too costly
                    column.clone()
                };
                {
                    // re_tracing::profile_scope!("rotate"); // TODO: too costly

                    column.make_contiguous();
                    let (column, &mut []) = column.as_mut_slices() else {
                        unreachable!();
                    };

                    for (from, to) in swaps {
                        column[*to] = source[*from];
                    }
                }
            }

            reshuffle_control_column(col_time, &swaps);
            if !col_insert_id.is_empty() {
                reshuffle_control_column(col_insert_id, &swaps);
            }
            reshuffle_control_column(col_row_id, &swaps);
            reshuffle_control_column(col_num_instances, &swaps);
        }

        {
            // re_tracing::profile_scope!("data"); // TODO: too costly
            // shuffle component columns back into a sorted state
            for column in columns.values_mut() {
                let mut source = column.clone();
                {
                    for (from, to) in swaps.iter().copied() {
                        column[to] = source[from].take();
                    }
                }
                column.make_contiguous();
            }
        }

        *is_sorted = true;
    }
}

// --- Timeless ---

impl PersistentIndexedTable {
    /// Queries the table for the cells of the specified `components`, as seen from the point
    /// of view of the so-called `primary` component.
    ///
    /// Returns an array of [`DataCell`]s on success, or `None` iff no cell could be found for
    /// the `primary` component.
    fn latest_at<const N: usize>(
        &self,
        primary: ComponentName,
        components: &[ComponentName; N],
    ) -> Option<(RowId, [Option<DataCell>; N])> {
        if self.is_empty() {
            return None;
        }

        // Early-exit if this bucket is unaware of this component.
        let column = self.columns.get(&primary)?;

        re_tracing::profile_function!();

        trace!(
            kind = "latest_at",
            %primary,
            ?components,
            timeless = true,
            "searching for primary & secondary cells…"
        );

        // find the primary row number's row.
        let primary_row_nr = self.num_rows() - 1;

        trace!(
            kind = "latest_at",
            %primary,
            ?components,
            %primary_row_nr,
            timeless = true,
            "found primary row number",
        );

        // find the secondary indices' rows, and the associated cells.
        let mut secondary_row_nr = primary_row_nr;
        while column[secondary_row_nr as usize].is_none() {
            if secondary_row_nr == 0 {
                trace!(
                    kind = "latest_at",
                    %primary,
                    ?components,
                    timeless = true,
                    %primary_row_nr,
                    "no secondary row number found",
                );
                return None;
            }
            secondary_row_nr -= 1;
        }

        trace!(
            kind = "latest_at",
            %primary,
            ?components,
            timeless = true,
            %primary_row_nr, %secondary_row_nr,
            "found secondary row number",
        );
        debug_assert!(column[secondary_row_nr as usize].is_some());

        let mut cells = [(); N].map(|_| None);
        for (i, component) in components.iter().enumerate() {
            if let Some(column) = self.columns.get(component) {
                if let Some(cell) = &column[secondary_row_nr as usize] {
                    trace!(
                        kind = "latest_at",
                        %primary,
                        %component,
                        timeless = true,
                        %primary_row_nr, %secondary_row_nr,
                        "found cell",
                    );
                    cells[i] = Some(cell.clone() /* shallow */);
                }
            }
        }

        Some((self.col_row_id[secondary_row_nr as usize], cells))
    }

    /// Iterates the table in order to return the cells of the specified `components`,
    /// as seen from the point of view of the so-called `primary` component, for the given time
    /// range.
    ///
    /// For each and every relevant row that is found, the returned iterator will yield an array
    /// that is filled with the cells of each and every component in `components`, or `None` if
    /// said component is not available in that row.
    /// A row is considered iff it contains data for the `primary` component.
    ///
    /// This method cannot fail! If there's no data to return, an empty iterator is returned.
    pub fn range<const N: usize>(
        &self,
        components: [ComponentName; N],
    ) -> impl Iterator<Item = (RowId, [Option<DataCell>; N])> + '_ {
        // Early-exit if the table is unaware of any of our components of interest.
        if components
            .iter()
            .all(|component| self.columns.get(component).is_none())
        {
            return itertools::Either::Right(std::iter::empty());
        }

        // Beware! This merely measures the time it takes to gather all the necessary metadata
        // for building the returned iterator.
        re_tracing::profile_function!();

        let cells = (0..self.num_rows()).filter_map(move |row_nr| {
            let mut cells = [(); N].map(|_| None);
            for (i, component) in components.iter().enumerate() {
                if let Some(column) = self.columns.get(component) {
                    cells[i] = column[row_nr as usize].clone();
                }
            }

            // We only yield rows that contain data for at least one of the components of
            // interest.
            if cells.iter().all(Option::is_none) {
                return None;
            }

            let row_id = self.col_row_id[row_nr as usize];

            trace!(
                kind = "range",
                ?components,
                timeless = true,
                %row_nr,
                ?cells,
                "yielding cells",
            );

            Some((row_id, cells))
        });

        itertools::Either::Left(cells)
    }
}
