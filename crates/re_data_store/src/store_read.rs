use std::{collections::VecDeque, ops::RangeBounds, sync::atomic::Ordering};

use itertools::Itertools as _;

use re_log::trace;
use re_log_types::{
    DataCell, EntityPath, EntityPathHash, ResolvedTimeRange, RowId, TimeInt, TimePoint, Timeline,
};
use re_types_core::{ComponentName, ComponentNameSet};

use crate::{DataStore, IndexedBucket, IndexedBucketInner, IndexedTable};

// --- Queries ---

// TODO
pub use re_chunk::{LatestAtQuery, RangeQuery};

// --- Data store ---

impl DataStore {
    /// Retrieve all the [`ComponentName`]s that have been written to for a given [`EntityPath`] on
    /// the specified [`Timeline`].
    ///
    /// Static components are always included in the results.
    ///
    /// Returns `None` if the entity doesn't exist at all on this `timeline`.
    pub fn all_components(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
    ) -> Option<ComponentNameSet> {
        re_tracing::profile_function!();

        // TODO(cmc): kind & query_id need to somehow propagate through the span system.
        self.query_id.fetch_add(1, Ordering::Relaxed);

        let entity_path_hash = entity_path.hash();

        let static_components: Option<ComponentNameSet> = self
            .static_tables
            .get(&entity_path_hash)
            .map(|static_table| static_table.cells.keys().copied().collect());

        let temporal_components: Option<ComponentNameSet> = self
            .tables
            .get(&(entity_path_hash, *timeline))
            .map(|table| table.all_components.clone());

        match (static_components, temporal_components) {
            (None, None) => None,
            (None, comps @ Some(_)) | (comps @ Some(_), None) => comps,
            (Some(static_comps), Some(temporal_comps)) => {
                Some(static_comps.into_iter().chain(temporal_comps).collect())
            }
        }
    }

    /// Check whether a given entity has a specific [`ComponentName`] either on the specified
    /// timeline, or in its static data.
    #[inline]
    pub fn entity_has_component(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
        component_name: &ComponentName,
    ) -> bool {
        re_tracing::profile_function!();
        self.all_components(timeline, entity_path)
            .map_or(false, |components| components.contains(component_name))
    }

    /// Find the earliest time at which something was logged for a given entity on the specified
    /// timeline.
    ///
    /// Ignores static data.
    #[inline]
    pub fn entity_min_time(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
    ) -> Option<TimeInt> {
        let entity_path_hash = entity_path.hash();

        let min_time = self
            .tables
            .get(&(entity_path_hash, *timeline))?
            .buckets
            .first_key_value()?
            .1
            .inner
            .read()
            .time_range
            .min();

        // handle case where no data was logged
        if min_time == TimeInt::MIN {
            None
        } else {
            Some(min_time)
        }
    }

    /// Queries the datastore for the cells of the specified `component_names`, as seen from the point
    /// of view of the so-called `primary` component.
    ///
    /// Returns an array of [`DataCell`]s (as well as the associated _data_ time and [`RowId`], if
    /// the data is temporal) on success.
    ///
    /// Success is defined by one thing and one thing only: whether a cell could be found for the
    /// `primary` component.
    /// The presence or absence of secondary components has no effect on the success criteria.
    ///
    /// If the entity has static component data associated with it, it will unconditionally
    /// override any temporal component data.
    pub fn latest_at<const N: usize>(
        &self,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        primary: ComponentName,
        component_names: &[ComponentName; N],
    ) -> Option<(TimeInt, RowId, [Option<DataCell>; N])> {
        // TODO(cmc): kind & query_id need to somehow propagate through the span system.
        self.query_id.fetch_add(1, Ordering::Relaxed);

        let entity_path_hash = entity_path.hash();
        let primary_comp_pos = component_names
            .iter()
            .find_position(|component_name| **component_name == primary)
            .map(|(pos, _)| pos)?;

        let static_table = self.static_tables.get(&entity_path_hash);

        // Check which components have static data associated with them, and if so don't bother
        // querying for their temporal data.
        let mut component_names_opt = [(); N].map(|_| None);
        for (i, component_name) in component_names.iter().copied().enumerate() {
            let has_static_data = static_table.map_or(false, |static_table| {
                static_table.cells.contains_key(&component_name)
            });
            component_names_opt[i] = (!has_static_data).then_some(component_name);
        }

        // Grab the temporal results.
        let (mut data_time, mut max_row_id, mut results) = self
            .tables
            .get(&(entity_path_hash, query.timeline()))
            .and_then(|table| table.latest_at(query.at(), primary, &component_names_opt))
            .map_or_else(
                || (TimeInt::STATIC, RowId::ZERO, [(); N].map(|_| None)),
                |(data_time, row_id, cells)| (data_time, row_id, cells),
            );

        // Overwrite results with static data, where applicable.
        if let Some(static_table) = self.static_tables.get(&entity_path_hash) {
            for (i, component_name) in component_names.iter().enumerate() {
                if let Some(static_cell) = static_table.cells.get(component_name).cloned() {
                    results[i] = Some(static_cell.cell.clone());

                    // If and only if the primary is static, overwrite the returned index.
                    if *component_name == primary {
                        data_time = TimeInt::STATIC;
                        max_row_id = RowId::max(max_row_id, static_cell.row_id);
                    }
                }
            }
        }

        results[primary_comp_pos]
            .is_some()
            .then_some((data_time, max_row_id, results))
    }

    /// Iterates the datastore in order to return the cells of the specified `component_names` for
    /// the given time range.
    ///
    /// For each and every relevant row that is found, the returned iterator will yield an array
    /// that is filled with the cells of each and every component in `component_names`, or `None` if
    /// said component is not available in that row.
    ///
    /// This method cannot fail! If there's no data to return, an empty iterator is returned.
    ///
    /// ⚠ Contrary to latest-at queries, range queries can and will yield multiple rows for a
    /// single timestamp if it happens to hold multiple entries.
    ///
    /// If the entity has static component data associated with it, it will unconditionally
    /// override any temporal component data.
    pub fn range<'a, const N: usize>(
        &'a self,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component_names: [ComponentName; N],
    ) -> impl Iterator<Item = (TimeInt, RowId, [Option<DataCell>; N])> + 'a {
        // Beware! This merely measures the time it takes to gather all the necessary metadata
        // for building the returned iterator.
        re_tracing::profile_function!();

        // TODO(cmc): kind & query_id need to somehow propagate through the span system.
        self.query_id.fetch_add(1, Ordering::Relaxed);

        let entity_path_hash = entity_path.hash();

        let static_table = self.static_tables.get(&entity_path_hash);

        // Check which components have static data associated with them, and if so don't bother
        // querying for their temporal data.
        let mut component_names_opt = [(); N].map(|_| None);
        for (i, component_name) in component_names.iter().copied().enumerate() {
            let has_static_data = static_table.map_or(false, |static_table| {
                static_table.cells.contains_key(&component_name)
            });
            component_names_opt[i] = (!has_static_data).then_some(component_name);
        }

        // Yield the static data that's available first.
        let static_data = if let Some(static_table) = self.static_tables.get(&entity_path_hash) {
            let mut max_row_id = RowId::ZERO;
            let mut results = [(); N].map(|_| None);

            for (i, component_name) in component_names.iter().enumerate() {
                if let Some(static_cell) = static_table.cells.get(component_name).cloned() {
                    results[i] = Some(static_cell.cell.clone());

                    // There's no concept of a primary in low-level range queries, so we just give
                    // priority to whichever component has the most recent rowid when it comes to
                    // the returned index.
                    if static_cell.row_id > max_row_id {
                        max_row_id = RowId::max(max_row_id, static_cell.row_id);
                    }
                }
            }

            if results.iter().any(Option::is_some) {
                itertools::Either::Left(std::iter::once((TimeInt::STATIC, max_row_id, results)))
            } else {
                itertools::Either::Right(std::iter::empty())
            }
        } else {
            itertools::Either::Right(std::iter::empty())
        };

        static_data.chain(
            self.tables
                .get(&(entity_path_hash, query.timeline))
                .map(|index| index.range(query.range, component_names_opt))
                .into_iter()
                .flatten(),
        )
    }

    #[inline]
    pub fn row_metadata(&self, row_id: &RowId) -> Option<&(TimePoint, EntityPathHash)> {
        self.metadata_registry.get(row_id)
    }

    /// Sort all unsorted indices in the store.
    pub fn sort_indices_if_needed(&self) {
        re_tracing::profile_function!();
        for index in self.tables.values() {
            index.sort_indices_if_needed();
        }
    }
}

// --- Temporal ---

impl IndexedTable {
    /// Queries the table for the cells of the specified `component_names`, as seen from the point
    /// of view of the so-called `primary` component.
    ///
    /// Returns an array of [`DataCell`]s (as well as the associated _data_ time and `RowId`) on
    /// success, or `None` iff no cell could be found for the `primary` component.
    pub fn latest_at<const N: usize>(
        &self,
        query_time: TimeInt,
        primary: ComponentName,
        component_names: &[Option<ComponentName>; N],
    ) -> Option<(TimeInt, RowId, [Option<DataCell>; N])> {
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
            .range_buckets_rev(..=query_time)
            .map(|(_, bucket)| bucket)
            .enumerate();
        for (attempt, bucket) in buckets {
            trace!(
                kind = "latest_at",
                timeline = %timeline.name(),
                time = timeline.typ().format_utc(query_time),
                %primary,
                ?component_names,
                attempt,
                bucket_time_range = timeline.typ().format_range_utc(bucket.inner.read().time_range),
                "found candidate bucket"
            );
            if let ret @ Some(_) = bucket.latest_at(query_time, primary, component_names) {
                return ret; // found at least the primary component!
            }
        }

        None // primary component not found
    }

    /// Iterates the table in order to return the cells of the specified `component_names` for the
    /// given time range.
    ///
    /// For each and every relevant row that is found, the returned iterator will yield an array
    /// that is filled with the cells of each and every component in `component_names`, or `None` if
    /// said component is not available in that row.
    ///
    /// This method cannot fail! If there's no data to return, an empty iterator is returned.
    pub fn range<const N: usize>(
        &self,
        time_range: ResolvedTimeRange,
        component_names: [Option<ComponentName>; N],
    ) -> impl Iterator<Item = (TimeInt, RowId, [Option<DataCell>; N])> + '_ {
        // Beware! This merely measures the time it takes to gather all the necessary metadata
        // for building the returned iterator.
        re_tracing::profile_function!();

        let timeline = self.timeline;

        // We need to find the _indexing time_ that corresponds to this time range's minimum bound!
        let (time_range_min, _) = self.find_bucket(time_range.min());

        self.range_buckets(time_range_min..=time_range.max())
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
                    ?component_names,
                    "found bucket in range"
                );

                bucket.range(time_range, component_names)
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
        #[allow(clippy::unwrap_used)]
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
        #[allow(clippy::unwrap_used)]
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
    /// Sort all component indices by time and [`RowId`], provided that's not already the case.
    #[inline]
    pub fn sort_indices_if_needed(&self) {
        if self.inner.read().is_sorted {
            return; // early read-only exit
        }

        re_tracing::profile_scope!("sort");
        self.inner.write().sort();
    }

    /// Queries the bucket for the cells of the specified `component_names`, as seen from the point
    /// of view of the so-called `primary` component.
    ///
    /// Returns an array of [`DataCell`]s (as well as the associated _data_ time and `RowId`) on
    /// success, or `None` iff no cell could be found for the `primary` component.
    pub fn latest_at<const N: usize>(
        &self,
        query_time: TimeInt,
        primary: ComponentName,
        component_names: &[Option<ComponentName>; N],
    ) -> Option<(TimeInt, RowId, [Option<DataCell>; N])> {
        self.sort_indices_if_needed();

        let IndexedBucketInner {
            is_sorted,
            time_range: _,
            col_time,
            col_insert_id: _,
            col_row_id,
            max_row_id: _,
            columns,
            size_bytes: _,
        } = &*self.inner.read();
        debug_assert!(is_sorted);

        // Early-exit if this bucket is unaware of this component.
        let column = columns.get(&primary)?;

        trace!(
            kind = "latest_at",
            %primary,
            ?component_names,
            timeline = %self.timeline.name(),
            query_time = self.timeline.typ().format_utc(query_time),
            "searching for primary & secondary cells…"
        );

        let time_row_nr =
            col_time.partition_point(|data_time| *data_time <= query_time.as_i64()) as i64;

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
            ?component_names,
            timeline = %self.timeline.name(),
            query_time = self.timeline.typ().format_utc(query_time),
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
                    ?component_names,
                    timeline = %self.timeline.name(),
                    query_time = self.timeline.typ().format_utc(query_time),
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
            ?component_names,
            timeline = %self.timeline.name(),
            query_time = self.timeline.typ().format_utc(query_time),
            %primary_row_nr, %secondary_row_nr,
            "found secondary row number",
        );
        debug_assert!(column[secondary_row_nr as usize].is_some());

        let mut cells = [(); N].map(|_| None);
        for (i, component_name) in component_names.iter().enumerate() {
            let Some(component_name) = component_name else {
                // That component has static data.
                continue;
            };

            if let Some(column) = columns.get(component_name) {
                if let Some(cell) = &column[secondary_row_nr as usize] {
                    trace!(
                        kind = "latest_at",
                        %primary,
                        %component_name,
                        timeline = %self.timeline.name(),
                        query_time = self.timeline.typ().format_utc(query_time),
                        %primary_row_nr, %secondary_row_nr,
                        "found cell",
                    );
                    cells[i] = Some(cell.clone() /* shallow */);
                }
            }
        }

        Some((
            col_time[secondary_row_nr as usize]
                .try_into()
                .unwrap_or(TimeInt::MIN),
            col_row_id[secondary_row_nr as usize],
            cells,
        ))
    }

    /// Iterates the bucket in order to return the cells of the specified `component_names` for
    /// the given time range.
    ///
    /// For each and every relevant row that is found, the returned iterator will yield an array
    /// that is filled with the cells of each and every component in `component_names`, or `None` if
    /// said component is not available in that row.
    ///
    /// This method cannot fail! If there's no data to return, an empty iterator is returned.
    pub fn range<const N: usize>(
        &self,
        time_range: ResolvedTimeRange,
        component_names: [Option<ComponentName>; N],
    ) -> impl Iterator<Item = (TimeInt, RowId, [Option<DataCell>; N])> + '_ {
        self.sort_indices_if_needed();

        let IndexedBucketInner {
            is_sorted,
            time_range: bucket_time_range,
            col_time,
            col_insert_id: _,
            col_row_id,
            max_row_id: _,
            columns,
            size_bytes: _,
        } = &*self.inner.read();
        debug_assert!(is_sorted);

        let bucket_time_range = *bucket_time_range;

        // Early-exit if this bucket is unaware of any of our components of interest.
        if component_names
            .iter()
            .filter_map(|c| *c)
            .all(|component| columns.get(&component).is_none())
        {
            return itertools::Either::Right(std::iter::empty());
        }

        // Beware! This merely measures the time it takes to gather all the necessary metadata
        // for building the returned iterator.
        re_tracing::profile_function!();

        trace!(
            kind = "range",
            bucket_time_range = self.timeline.typ().format_range_utc(bucket_time_range),
            ?component_names,
            timeline = %self.timeline.name(),
            time_range = self.timeline.typ().format_range_utc(time_range),
            "searching for time & component cell numbers…"
        );

        let time_row_nr = col_time.partition_point(|t| *t < time_range.min().as_i64()) as u64;

        trace!(
            kind = "range",
            bucket_time_range = self.timeline.typ().format_range_utc(bucket_time_range),
            ?component_names,
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
            .filter(move |&data_time| time_range.contains(TimeInt::new_temporal(data_time)))
            .enumerate()
            .filter_map(move |(time_row_offset, data_time)| {
                let row_nr = time_row_nr + time_row_offset as u64;

                let mut cells = [(); N].map(|_| None);
                for (i, component_name) in component_names.iter().enumerate() {
                    let Some(component_name) = component_name else {
                        // That component has static data.
                        continue;
                    };

                    if let Some(column) = columns.get_mut(component_name) {
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
                    ?component_names,
                    timeline = %self.timeline.name(),
                    time_range = self.timeline.typ().format_range_utc(time_range),
                    %row_nr,
                    %row_id,
                    ?cells,
                    "yielding cells",
                );

                Some((TimeInt::new_temporal(data_time), row_id, cells))
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
            max_row_id: _,
            columns,
            size_bytes: _,
        } = self;

        if *is_sorted {
            return;
        }

        re_tracing::profile_function!();

        let swaps = {
            re_tracing::profile_scope!("swaps");
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
            re_tracing::profile_scope!("control");

            fn reshuffle_control_column<T: Copy>(
                column: &mut VecDeque<T>,
                swaps: &[(usize, usize)],
            ) {
                let source = {
                    re_tracing::profile_scope!("clone");
                    column.clone()
                };
                {
                    re_tracing::profile_scope!("rotate");
                    for (from, to) in swaps.iter().copied() {
                        column[to] = source[from];
                    }
                }
            }

            reshuffle_control_column(col_time, &swaps);
            if !col_insert_id.is_empty() {
                reshuffle_control_column(col_insert_id, &swaps);
            }
            reshuffle_control_column(col_row_id, &swaps);
        }

        {
            re_tracing::profile_scope!("data");
            // shuffle component columns back into a sorted state
            for column in columns.values_mut() {
                let mut source = column.clone();
                {
                    for (from, to) in swaps.iter().copied() {
                        column[to] = source[from].take();
                    }
                }
            }
        }

        *is_sorted = true;
    }
}
