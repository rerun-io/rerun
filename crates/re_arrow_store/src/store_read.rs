use std::{ops::RangeBounds, sync::atomic::Ordering};

use arrow2::{
    array::{Array, Int64Array, ListArray, UInt64Array},
    datatypes::{DataType, TimeUnit},
};

use re_log::trace;
use re_log_types::{ComponentName, ObjPath as EntityPath, TimeInt, TimeRange, Timeline};

use crate::{
    ComponentBucket, ComponentTable, DataStore, IndexBucket, IndexBucketIndices, IndexRowNr,
    IndexTable, RowIndex, SecondaryIndex,
};

// --- Queries ---

/// A query a given time, for a given timeline.
///
/// Get the latest version of the data available at this time.
#[derive(Clone)]
pub struct LatestAtQuery {
    pub timeline: Timeline,
    pub at: TimeInt,
}

impl std::fmt::Debug for LatestAtQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "<latest at {} on {:?}>",
            self.timeline.typ().format(self.at),
            self.timeline.name(),
        ))
    }
}

impl LatestAtQuery {
    pub const fn new(timeline: Timeline, at: TimeInt) -> Self {
        Self { timeline, at }
    }
}

/// A query over a time range, for a given timeline.
///
/// Get all the data within this time interval, plus the latest one before the start of the
/// interval.
///
/// Motivation: all data is considered alive until the next logging to the same data path.
#[derive(Clone)]
pub struct RangeQuery {
    pub timeline: Timeline,
    pub range: TimeRange,
}

impl std::fmt::Debug for RangeQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "<ranging from {} to {} (all inclusive) on {:?}>",
            self.timeline.typ().format(self.range.min),
            self.timeline.typ().format(self.range.max),
            self.timeline.name(),
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
    /// Retrieve all the `ComponentName`s that have been written to for a given `EntityPath`
    pub fn latest_components_at(
        &self,
        query: &LatestAtQuery,
        ent_path: &EntityPath,
    ) -> Option<Vec<ComponentName>> {
        // TODO(cmc): kind & query_id need to somehow propagate through the span system.
        self.query_id.fetch_add(1, Ordering::Relaxed);

        let ent_path_hash = ent_path.hash();

        trace!(
            kind = "latest_components_at",
            id = self.query_id.load(Ordering::Relaxed),
            query = ?query,
            entity = %ent_path,
            "query started..."
        );

        let index = self.indices.get(&(query.timeline, *ent_path_hash))?;
        let (_, bucket) = index.find_bucket(query.at);
        let components = bucket.named_indices().0;

        trace!(
            kind = "latest_components_at",
            id = self.query_id.load(Ordering::Relaxed),
            query = ?query,
            entity = %ent_path,
            ?components,
            "found components"
        );

        Some(components)
    }

    /// Queries the datastore for the internal row indices of the specified `components`, as seen
    /// from the point of view of the so-called `primary` component.
    ///
    /// Returns an array of row indices on success, or `None` otherwise.
    /// Success is defined by one thing and thing only: whether a row index could be found for the
    /// `primary` component.
    /// The presence or absence of secondary components has no effect on the success criteria.
    ///
    /// * On success, the returned array is filled with the internal row index of each and every
    ///   component in `components`, or `None` if said component is not available in that row.
    ///
    /// To actually retrieve the data associated with these indices, see [`Self::get`].
    ///
    /// ## Example
    ///
    /// The following example demonstrate how to fetch the latest row indices for a given
    /// component and the associated cluster key, then get the corresponding data using these row
    /// indices, and finally turn everything into a nice-to-work-with polars's dataframe.
    ///
    /// ```rust
    /// # use polars_core::{prelude::*, series::Series};
    /// # use re_log_types::{ComponentName, ObjPath as EntityPath, TimeInt};
    /// # use re_arrow_store::{DataStore, LatestAtQuery, RangeQuery};
    ///
    /// pub fn latest_component(
    ///     store: &DataStore,
    ///     query: &LatestAtQuery,
    ///     ent_path: &EntityPath,
    ///     primary: ComponentName,
    /// ) -> anyhow::Result<DataFrame> {
    ///     let cluster_key = store.cluster_key();
    ///
    ///     let components = &[cluster_key, primary];
    ///     let row_indices = store
    ///         .latest_at(query, ent_path, primary, components)
    ///         .unwrap_or([None; 2]);
    ///     let results = store.get(components, &row_indices);
    ///
    ///     let series: Result<Vec<_>, _> = components
    ///         .iter()
    ///         .zip(results)
    ///         .filter_map(|(component, col)| col.map(|col| (component, col)))
    ///         .map(|(&component, col)| Series::try_from((component.as_str(), col)))
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
    ) -> Option<[Option<RowIndex>; N]> {
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
            "query started..."
        );

        if let Some(index) = self.indices.get(&(query.timeline, *ent_path_hash)) {
            if let row_indices @ Some(_) = index.latest_at(query.at, primary, components) {
                trace!(
                    kind = "latest_at",
                    query = ?query,
                    entity = %ent_path,
                    %primary,
                    ?components,
                    ?row_indices,
                    "row indices fetched"
                );
                return row_indices;
            }
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

    /// Iterates the datastore in order to return the internal row indices of the the specified
    /// `components`, as seen from the point of view of the so-called `primary` component, for the
    /// given time range.
    ///
    /// For each and every relevant row that is found, the returned iterator will yield an array
    /// that is filled with the internal row index of each and every component in `components`,
    /// or `None` if said component is not available in that row.
    /// A row is considered iff it contains data for the `primary` component.
    ///
    /// This method cannot fail! If there's no data to return (whether that's due to a missing
    /// primary index, missing secondary components, an empty point-of-view...), then an empty
    /// iterator is returned.
    ///
    /// To actually retrieve the data associated with these indices, see [`Self::get`].
    ///
    /// ⚠ Contrary to latest-at queries, range queries can and will yield multiple rows for a
    /// single timestamp if that timestamp happens to hold multiple entries for the `primary`
    /// component.
    /// On the contrary, they won't yield any rows that don't contain an actual value for the
    /// `primary` component, _even if said rows do contain a value for one the secondaries_!
    ///
    /// ## Example
    ///
    /// The following example demonstrate how to range over the row indices of a given
    /// component and its associated cluster key, then get the corresponding data using these
    /// row indices, and finally turn everything into a nice-to-work-with iterator of
    /// polars's dataframe.
    /// Additionally, it yields the latest-at state of the component a the start of the time range,
    /// if available.
    ///
    /// ```rust
    /// # use arrow2::array::Array;
    /// # use polars_core::{prelude::*, series::Series};
    /// # use re_log_types::{ComponentName, ObjPath as EntityPath, TimeInt};
    /// # use re_arrow_store::{DataStore, LatestAtQuery, RangeQuery};
    ///
    /// # pub fn dataframe_from_results<const N: usize>(
    /// #     components: &[ComponentName; N],
    /// #     results: [Option<Box<dyn Array>>; N],
    /// # ) -> anyhow::Result<DataFrame> {
    /// #     let series: Result<Vec<_>, _> = components
    /// #         .iter()
    /// #         .zip(results)
    /// #         .filter_map(|(component, col)| col.map(|col| (component, col)))
    /// #         .map(|(&component, col)| Series::try_from((component.as_str(), col)))
    /// #         .collect();
    /// #
    /// #     DataFrame::new(series?).map_err(Into::into)
    /// # }
    ///
    /// pub fn range_component<'a>(
    ///     store: &'a DataStore,
    ///     query: &'a RangeQuery,
    ///     ent_path: &'a EntityPath,
    ///     primary: ComponentName,
    /// ) -> impl Iterator<Item = anyhow::Result<(TimeInt, DataFrame)>> + 'a {
    ///     let cluster_key = store.cluster_key();
    ///
    ///     let components = [cluster_key, primary];
    ///
    ///     // Fetch the latest-at data just before the start of the time range.
    ///     let latest_time = query.range.min.as_i64().saturating_sub(1).into();
    ///     let df_latest = {
    ///         let query = LatestAtQuery::new(query.timeline, latest_time);
    ///         let row_indices = store
    ///             .latest_at(&query, ent_path, primary, &components)
    ///             .unwrap_or([None; 2]);
    ///         let results = store.get(&components, &row_indices);
    ///         dataframe_from_results(&components, results)
    ///     };
    ///
    ///     // Send the latest-at state before anything else..
    ///     std::iter::once(df_latest.map(|df| (latest_time, df)))
    ///         // ..but only if it's not an empty dataframe.
    ///         .filter(|df| df.as_ref().map_or(true, |(_, df)| !df.is_empty()))
    ///         .chain(store.range(query, ent_path, components).map(
    ///             move |(time, _, row_indices)| {
    ///                 let results = store.get(&components, &row_indices);
    ///                 dataframe_from_results(&components, results).map(|df| (time, df))
    ///             },
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
    ) -> impl Iterator<Item = (TimeInt, IndexRowNr, [Option<RowIndex>; N])> + 'a {
        // TODO(cmc): kind & query_id need to somehow propagate through the span system.
        self.query_id.fetch_add(1, Ordering::Relaxed);

        let ent_path_hash = ent_path.hash();

        trace!(
            kind = "range",
            id = self.query_id.load(Ordering::Relaxed),
            query = ?query,
            entity = %ent_path,
            ?components,
            "query started..."
        );

        let index = self.indices.get(&(query.timeline, *ent_path_hash));

        index
            .map(|index| index.range(query.range, components))
            .into_iter()
            .flatten()
    }

    /// Retrieves the data associated with a list of `components` at the specified `indices`.
    ///
    /// If the associated data is found, it will be written into the returned array at the
    /// appropriate index, or `None` otherwise.
    ///
    /// `row_indices` takes a list of options so that one can easily re-use the results obtained
    /// from [`Self::latest_at`] & [`Self::range`].
    pub fn get<const N: usize>(
        &self,
        components: &[ComponentName; N],
        row_indices: &[Option<RowIndex>; N],
    ) -> [Option<Box<dyn Array>>; N] {
        let mut results = [(); N].map(|_| None); // work around non-Copy const initialization limitations

        for (i, &component, row_idx) in components
            .iter()
            .zip(row_indices)
            .enumerate()
            .filter_map(|(i, (comp, row_idx))| row_idx.map(|row_idx| (i, comp, row_idx)))
        {
            let row = self
                .components
                .get(&component)
                .and_then(|table| table.get(row_idx));
            results[i] = row;
        }

        results
    }

    /// Sort all unsorted indices in the store.
    pub fn sort_indices_if_needed(&mut self) {
        for index in self.indices.values_mut() {
            index.sort_indices_if_needed();
        }
    }

    /// Returns a read-only iterator over the raw index tables.
    ///
    /// Do _not_ use this to try and test the internal state of the datastore.
    pub fn iter_indices(
        &self,
    ) -> impl ExactSizeIterator<Item = ((Timeline, EntityPath), &IndexTable)> {
        self.indices.iter().map(|((timeline, _), table)| {
            ((*timeline, table.ent_path.clone() /* shallow */), table)
        })
    }
}

// --- Indices ---

impl IndexTable {
    /// Returns `None` iff no row index could be found for the `primary` component.
    pub fn latest_at<const N: usize>(
        &self,
        time: TimeInt,
        primary: ComponentName,
        components: &[ComponentName; N],
    ) -> Option<[Option<RowIndex>; N]> {
        let timeline = self.timeline;

        // The time we're looking for gives us an upper bound: all components must be indexed
        // in either this bucket _or any of those that come before_!
        //
        // That is because secondary indices allow for null values, which forces us to not only
        // walk backwards within an index bucket, but sometimes even walk backwards across
        // multiple index buckets within the same table!

        let buckets = self
            .range_buckets_rev(..=time)
            .map(|(_, bucket)| bucket)
            .enumerate();
        for (attempt, bucket) in buckets {
            trace!(
                kind = "latest_at",
                timeline = %timeline.name(),
                time = timeline.typ().format(time),
                %primary,
                ?components,
                attempt,
                bucket_time_range = timeline.typ().format_range(bucket.indices.read().time_range),
                "found candidate bucket"
            );
            if let row_indices @ Some(_) = bucket.latest_at(time, primary, components) {
                return row_indices; // found at least the primary component!
            }
        }

        None // primary component not found
    }

    /// Returns an empty iterator if no data could be found for any reason.
    pub fn range<const N: usize>(
        &self,
        time_range: TimeRange,
        components: [ComponentName; N],
    ) -> impl Iterator<Item = (TimeInt, IndexRowNr, [Option<RowIndex>; N])> + '_ {
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
                        timeline.typ().format_range(bucket.indices.read().time_range),
                    timeline = %timeline.name(),
                    ?time_range,
                    ?components,
                    "found bucket in range"
                );

                bucket.range(time_range, components)
            })
    }

    /// Returns the index bucket whose time range covers the given `time`.
    ///
    /// In addition to returning a reference to the `IndexBucket` itself, this also returns its
    /// _indexing time_, which is different from its minimum time range bound!
    /// See `IndexTable::buckets` for more information.
    pub fn find_bucket(&self, time: TimeInt) -> (TimeInt, &IndexBucket) {
        // This cannot fail, `iter_bucket` is guaranteed to always yield at least one bucket,
        // since index tables always spawn with a default bucket that covers [-∞;+∞].
        self.range_buckets_rev(..=time).next().unwrap()
    }

    /// Returns the index bucket whose time range covers the given `time`.
    ///
    /// In addition to returning a reference to the `IndexBucket` itself, this also returns its
    /// _indexing time_, which is different from its minimum time range bound!
    /// See `IndexTable::buckets` for more information.
    pub fn find_bucket_mut(&mut self, time: TimeInt) -> (TimeInt, &mut IndexBucket) {
        // This cannot fail, `iter_bucket_mut` is guaranteed to always yield at least one bucket,
        // since index tables always spawn with a default bucket that covers [-∞;+∞].
        self.range_bucket_rev_mut(..=time).next().unwrap()
    }

    /// Returns an iterator that is guaranteed to yield at least one bucket, which is the bucket
    /// whose time range covers the start bound of the given `time_range`.
    ///
    /// It then continues yielding buckets until it runs out, in increasing time range order.
    ///
    /// In addition to yielding references to the `IndexBucket`s themselves, this also returns
    /// their _indexing times_, which are different from their minimum time range bounds!
    /// See `IndexTable::buckets` for more information.
    pub fn range_buckets(
        &self,
        time_range: impl RangeBounds<TimeInt>,
    ) -> impl Iterator<Item = (TimeInt, &IndexBucket)> {
        self.buckets
            .range(time_range)
            .map(|(time, bucket)| (*time, bucket))
    }

    /// Returns an iterator that is guaranteed to yield at least one bucket, which is the bucket
    /// whose time range covers the end bound of the given `time_range`.
    ///
    /// It then continues yielding buckets until it runs out, in decreasing time range order.
    ///
    /// In addition to yielding references to the `IndexBucket`s themselves, this also returns
    /// their _indexing times_, which are different from their minimum time range bounds!
    /// See `IndexTable::buckets` for more information.
    pub fn range_buckets_rev(
        &self,
        time_range: impl RangeBounds<TimeInt>,
    ) -> impl Iterator<Item = (TimeInt, &IndexBucket)> {
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
    /// In addition to yielding references to the `IndexBucket`s themselves, this also returns
    /// their _indexing times_, which are different from their minimum time range bounds!
    /// See `IndexTable::buckets` for more information.
    pub fn range_bucket_rev_mut(
        &mut self,
        time_range: impl RangeBounds<TimeInt>,
    ) -> impl Iterator<Item = (TimeInt, &mut IndexBucket)> {
        self.buckets
            .range_mut(time_range)
            .rev()
            .map(|(time, bucket)| (*time, bucket))
    }

    /// Sort all unsorted index buckets in this table.
    pub fn sort_indices_if_needed(&self) {
        for bucket in self.buckets.values() {
            bucket.sort_indices_if_needed();
        }
    }

    /// Returns a read-only iterator over the raw buckets.
    ///
    /// Do _not_ use this to try and test the internal state of the datastore.
    pub fn iter_buckets(&self) -> impl ExactSizeIterator<Item = &IndexBucket> {
        self.buckets.values()
    }
}

impl IndexBucket {
    /// Sort all component indices by time, provided that's not already the case.
    pub fn sort_indices_if_needed(&self) {
        if self.indices.read().is_sorted {
            return; // early read-only exit
        }

        self.indices.write().sort();
    }

    /// Returns `None` iff no row index could be found for the `primary` component.
    pub fn latest_at<const N: usize>(
        &self,
        time: TimeInt,
        primary: ComponentName,
        components: &[ComponentName; N],
    ) -> Option<[Option<RowIndex>; N]> {
        self.sort_indices_if_needed();

        let IndexBucketIndices {
            is_sorted,
            time_range: _,
            times,
            indices,
        } = &*self.indices.read();
        debug_assert!(is_sorted);

        // Early-exit if this bucket is unaware of this component.
        let index = indices.get(&primary)?;

        trace!(
            kind = "latest_at",
            %primary,
            ?components,
            timeline = %self.timeline.name(),
            time = self.timeline.typ().format(time),
            "searching for primary & secondary row indices..."
        );

        // find the primary index's row.
        let primary_idx = times.partition_point(|t| *t <= time.as_i64()) as i64;

        // The partition point is always _beyond_ the index that we're looking for.
        // A partition point of 0 thus means that we're trying to query for data that lives
        // _before_ the beginning of time... there's nothing to be found there.
        if primary_idx == 0 {
            return None;
        }

        // The partition point is always _beyond_ the index that we're looking for; we need
        // to step back to find what we came for.
        let primary_idx = primary_idx - 1;
        trace!(
            kind = "latest_at",
            %primary,
            ?components,
            timeline = %self.timeline.name(),
            time = self.timeline.typ().format(time),
            %primary_idx,
            "found primary index",
        );

        // find the secondary indices' rows, and the associated row indices.
        let mut secondary_idx = primary_idx;
        while index[secondary_idx as usize].is_none() {
            secondary_idx -= 1;
            if secondary_idx < 0 {
                trace!(
                    kind = "latest_at",
                    %primary,
                    ?components,
                    timeline = %self.timeline.name(),
                    time = self.timeline.typ().format(time),
                    %primary_idx,
                    "no secondary index found",
                );
                return None;
            }
        }

        trace!(
            kind = "latest_at",
            %primary,
            ?components,
            timeline = %self.timeline.name(),
            time = self.timeline.typ().format(time),
            %primary_idx, %secondary_idx,
            "found secondary index",
        );
        debug_assert!(index[secondary_idx as usize].is_some());

        let mut row_indices = [None; N];
        for (i, component) in components.iter().enumerate() {
            if let Some(index) = indices.get(component) {
                if let Some(row_idx) = index[secondary_idx as usize] {
                    trace!(
                        kind = "latest_at",
                        %primary,
                        %component,
                        timeline = %self.timeline.name(),
                        time = self.timeline.typ().format(time),
                        %primary_idx, %secondary_idx, %row_idx,
                        "found row index",
                    );
                    row_indices[i] = Some(row_idx);
                }
            }
        }

        Some(row_indices)
    }

    /// Returns an empty iterator if no data could be found for any reason.
    pub fn range<'a, const N: usize>(
        &'a self,
        time_range: TimeRange,
        components: [ComponentName; N],
    ) -> impl Iterator<Item = (TimeInt, IndexRowNr, [Option<RowIndex>; N])> + 'a {
        self.sort_indices_if_needed();

        let IndexBucketIndices {
            is_sorted,
            time_range: bucket_time_range,
            times,
            indices,
        } = &*self.indices.read();
        debug_assert!(is_sorted);

        let bucket_time_range = *bucket_time_range;

        // Early-exit if this bucket is unaware of any of our components of interest.
        if components
            .iter()
            .all(|component| indices.get(component).is_none())
        {
            return itertools::Either::Right(std::iter::empty());
        }

        trace!(
            kind = "range",
            bucket_time_range = self.timeline.typ().format_range(bucket_time_range),
            ?components,
            timeline = %self.timeline.name(),
            time_range = self.timeline.typ().format_range(time_range),
            "searching for time & component row index numbers..."
        );

        // find the time index's row number
        let time_idx_row_nr: IndexRowNr =
            IndexRowNr(times.partition_point(|t| *t < time_range.min.as_i64()) as u64);

        trace!(
            kind = "range",
            bucket_time_range = self.timeline.typ().format_range(bucket_time_range),
            ?components,
            timeline = %self.timeline.name(),
            time_range = self.timeline.typ().format_range(time_range),
            %time_idx_row_nr,
            "found time index row number",
        );

        // TODO(cmc): Cloning these is obviously not great and will need to be addressed at
        // some point.
        // But, really, it's not _that_ bad either: these are integers and e.g. with the default
        // configuration there are only 1024 of them (times the number of components).
        let time_idx = times.clone();
        let comp_indices = indices.clone();

        // We have found the index of the first row that possibly contains data for any single one
        // of the components we're interested in.
        //
        // Now we need to iterate through every remaining rows in the bucket and yield any that
        // contains data for these components and is still within the time range.
        let row_indices = time_idx
            .into_iter()
            .skip(time_idx_row_nr.0 as usize)
            // don't go beyond the time range we're interested in!
            .filter(move |time| time_range.contains((*time).into()))
            .enumerate()
            .filter_map(move |(time_idx_offset, time)| {
                let comp_idx_row_nr = IndexRowNr(time_idx_row_nr.0 + time_idx_offset as u64);

                let mut row_indices = [None; N];
                for (i, component) in components.iter().enumerate() {
                    if let Some(index) = comp_indices.get(component) {
                        if let row_idx @ Some(_) = index[comp_idx_row_nr.0 as usize] {
                            row_indices[i] = row_idx;
                        }
                    }
                }

                // We only yield rows that contain data for at least one of the components of
                // interest.
                if row_indices.iter().all(Option::is_none) {
                    return None;
                }

                trace!(
                    kind = "range",
                    bucket_time_range =
                        self.timeline.typ().format_range(bucket_time_range),
                    ?components,
                    timeline = %self.timeline.name(),
                    time_range = self.timeline.typ().format_range(time_range),
                    %comp_idx_row_nr,
                    ?row_indices,
                    "yielding row indices",
                );

                Some((time.into(), comp_idx_row_nr, row_indices))
            });

        itertools::Either::Left(row_indices)
    }

    /// Whether the indices in this `IndexBucket` are sorted
    pub fn is_sorted(&self) -> bool {
        self.indices.read().is_sorted
    }

    /// Returns an (name, [`Int64Array`]) with a logical type matching the timeline.
    pub fn times(&self) -> (String, Int64Array) {
        let times = Int64Array::from_vec(self.indices.read().times.clone());
        let logical_type = match self.timeline.typ() {
            re_log_types::TimeType::Time => DataType::Timestamp(TimeUnit::Nanosecond, None),
            re_log_types::TimeType::Sequence => DataType::Int64,
        };
        (self.timeline.name().to_string(), times.to(logical_type))
    }

    /// Returns a Vec each of (name, array) for each index in the bucket
    pub fn named_indices(&self) -> (Vec<ComponentName>, Vec<UInt64Array>) {
        self.indices
            .read()
            .indices
            .iter()
            .map(|(name, index)| {
                (
                    name,
                    UInt64Array::from(
                        index
                            .iter()
                            .map(|row_idx| row_idx.map(|row_idx| row_idx.as_u64()))
                            .collect::<Vec<_>>(),
                    ),
                )
            })
            .unzip()
    }
}

impl IndexBucketIndices {
    pub fn sort(&mut self) {
        let Self {
            is_sorted,
            time_range: _,
            times,
            indices,
        } = self;

        if *is_sorted {
            return;
        }

        let swaps = {
            let mut swaps = (0..times.len()).collect::<Vec<_>>();
            swaps.sort_by_key(|&i| &times[i]);
            swaps
                .iter()
                .copied()
                .enumerate()
                .map(|(to, from)| (from, to))
                .collect::<Vec<_>>()
        };

        // Yep, the reshuffle implementation is very dumb and very slow :)
        // TODO(#442): re_datastore: implement efficient shuffling on the read path.

        // shuffle time index back into a sorted state
        {
            let source = times.clone();
            for (from, to) in swaps.iter().copied() {
                times[to] = source[from];
            }
        }

        fn reshuffle_index(index: &mut SecondaryIndex, swaps: &[(usize, usize)]) {
            // shuffle data
            {
                let source = index.clone();
                for (from, to) in swaps.iter().copied() {
                    index[to] = source[from];
                }
            }
        }

        // shuffle component indices back into a sorted state
        for index in indices.values_mut() {
            reshuffle_index(index, &swaps);
        }

        *is_sorted = true;
    }
}

// --- Components ---

impl ComponentTable {
    pub fn get(&self, row_idx: RowIndex) -> Option<Box<dyn Array>> {
        let mut bucket_nr = self
            .buckets
            .partition_point(|bucket| row_idx.as_u64() >= bucket.row_offset);

        // The partition point will give us the index of the first bucket that has a row offset
        // strictly greater than the row index we're looking for, therefore we need to take a
        // step back to find what we're looking for.
        //
        // Since component tables always spawn with a default bucket at offset 0, the smallest
        // partition point that can ever be returned is one, thus this operation is overflow-safe.
        debug_assert!(bucket_nr > 0);
        bucket_nr -= 1;

        if let Some(bucket) = self.buckets.get(bucket_nr) {
            trace!(
                kind = "get",
                component = self.name.as_str(),
                %row_idx,
                bucket_nr,
                %bucket.row_offset,
                "fetching component data"
            );
            Some(bucket.get(row_idx))
        } else {
            trace!(
                kind = "get",
                component = self.name.as_str(),
                %row_idx,
                bucket_nr,
                "row index is out of bounds"
            );
            None
        }
    }

    /// Returns an iterator over the `ComponentBucket` in this table
    #[allow(dead_code)]
    pub fn iter_buckets(&self) -> impl ExactSizeIterator<Item = &ComponentBucket> {
        self.buckets.iter()
    }
}

impl ComponentBucket {
    /// Returns the name of the component stored in this bucket.
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a shallow clone of the row data present at the given `row_idx`.
    pub fn get(&self, row_idx: RowIndex) -> Box<dyn Array> {
        let row_idx = row_idx.as_u64() - self.row_offset;
        // This has to be safe to unwrap, otherwise it would never have made it past insertion.
        if self.archived {
            debug_assert_eq!(self.chunks.len(), 1);
            self.chunks[0]
                .as_any()
                .downcast_ref::<ListArray<i32>>()
                .unwrap()
                .value(row_idx as _)
        } else {
            self.chunks[row_idx as usize]
                .as_any()
                .downcast_ref::<ListArray<i32>>()
                .unwrap()
                .value(0)
        }
    }

    /// Returns a shallow clone of all the chunks in this bucket.
    #[allow(dead_code)]
    pub fn data(&self) -> Vec<Box<dyn Array>> {
        self.chunks.clone() // shallow
    }

    /// Return an iterator over the time ranges in this bucket.
    #[allow(dead_code)]
    pub fn iter_time_ranges(&self) -> impl Iterator<Item = (&Timeline, &TimeRange)> {
        self.time_ranges.iter()
    }
}
