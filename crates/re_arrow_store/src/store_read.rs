use std::sync::atomic::Ordering;

use arrow2::{
    array::{Array, Int64Array, ListArray, MutableArray, UInt64Array, UInt64Vec},
    datatypes::{DataType, TimeUnit},
};

use re_log::debug;
use re_log_types::{ComponentNameRef, ObjPath as EntityPath, TimeInt, TimeRange, Timeline};

use crate::{
    ComponentBucket, ComponentTable, DataStore, IndexBucket, IndexBucketIndices, IndexTable,
    RowIndex,
};

// ---

/// A query in time, for a given timeline.
#[derive(Clone)]
pub struct TimelineQuery {
    pub timeline: Timeline,
    pub query: TimeQuery,
}

impl std::fmt::Debug for TimelineQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.query {
            &TimeQuery::LatestAt(at) => f.write_fmt(format_args!(
                "TimeQuery<latest at {} on {:?}>",
                self.timeline.typ().format(at.into()),
                self.timeline.name(),
            )),
            TimeQuery::Range(range) => f.write_fmt(format_args!(
                "TimeQuery<ranging from {} to {} (all inclusive) on {:?}>",
                self.timeline.typ().format((*range.start()).into()),
                self.timeline.typ().format((*range.end()).into()),
                self.timeline.name(),
            )),
        }
    }
}

impl TimelineQuery {
    pub const fn new(timeline: Timeline, query: TimeQuery) -> Self {
        Self { timeline, query }
    }
}

/// A query in time.
// TODO(#544): include timeline in there.
#[derive(Clone, Debug)]
pub enum TimeQuery {
    /// Get the latest version of the data available at this time.
    LatestAt(i64),

    /// Get all the data within this time interval, plus the latest
    /// one before the start of the interval.
    ///
    /// Motivation: all data is considered alive until the next logging
    /// to the same data path.
    Range(std::ops::RangeInclusive<i64>),
}

impl TimeQuery {
    pub const EVERYTHING: Self = Self::Range(i64::MIN..=i64::MAX);
}

// --- Data store ---

impl DataStore {
    /// Queries the datastore for the internal row indices of the specified `components`, as seen
    /// from the point of view of the so-called `primary` component.
    ///
    /// Returns an array of row indices on success, or `None` otherwise.
    /// Success is defined by one thing and thing only: whether a row index could be found for the
    /// `primary` component.
    /// The presence or absence of secondary components has no effect on the success criteria.
    ///
    /// * On success, the returned array is filled with the internal row index of each and every
    ///   component in `components`, or `None` if said component isn't available at that point
    ///   in time.
    ///
    /// To actually retrieve the data associated with these indices, see [`Self::get`].
    ///
    /// Follows a complete example of querying indices, fetching the associated data and finally
    /// turning it all into a `polars::DataFrame`:
    /// ```rust
    /// # use polars_core::prelude::*;
    /// # use arrow2::array::Array;
    /// # use re_log_types::{*, ObjPath as EntityPath};
    /// # use re_arrow_store::*;
    ///
    /// fn fetch_components<const N: usize>(
    ///     store: &DataStore,
    ///     timeline_query: &TimelineQuery,
    ///     ent_path: &EntityPath,
    ///     primary: ComponentNameRef<'_>,
    ///     components: &[ComponentNameRef<'_>; N],
    /// ) -> DataFrame {
    ///     let row_indices = store
    ///         .query(timeline_query, ent_path, primary, components)
    ///         .unwrap_or([None; N]);
    ///     let results = store.get(components, &row_indices);
    ///
    ///     let df = {
    ///         let series: Vec<_> = components
    ///             .iter()
    ///             .zip(results)
    ///             .filter_map(|(component, col)| col.map(|col| (component, col)))
    ///             .map(|(&component, col)| Series::try_from((component, col)).unwrap())
    ///             .collect();
    ///
    ///         DataFrame::new(series).unwrap()
    ///     };
    ///
    ///     df
    /// }
    /// ```
    //
    // TODO(cmc): expose query_dyn at some point, to fetch an unknown number of component indices,
    // at the cost of extra dynamic allocations.
    pub fn query<const N: usize>(
        &self,
        timeline_query: &TimelineQuery,
        ent_path: &EntityPath,
        primary: ComponentNameRef<'_>,
        components: &[ComponentNameRef<'_>; N],
    ) -> Option<[Option<RowIndex>; N]> {
        // TODO(cmc): kind & query_id need to somehow propagate through the span system.
        self.query_id.fetch_add(1, Ordering::Relaxed);

        let ent_path_hash = ent_path.hash();
        let latest_at = match timeline_query.query {
            TimeQuery::LatestAt(latest_at) => latest_at,
            #[allow(clippy::todo)]
            TimeQuery::Range(_) => todo!("implement range queries!"),
        };

        debug!(
            kind = "query",
            id = self.query_id.load(Ordering::Relaxed),
            query = ?timeline_query,
            entity = %ent_path,
            primary,
            ?components,
            "query started..."
        );

        if let Some(index) = self.indices.get(&(timeline_query.timeline, *ent_path_hash)) {
            if let row_indices @ Some(_) = index.latest_at(latest_at, primary, components) {
                debug!(
                    kind = "query",
                    query = ?timeline_query,
                    entity = %ent_path,
                    primary,
                    ?components,
                    ?row_indices,
                    "row indices fetched"
                );
                return row_indices;
            }
        }

        debug!(
            kind = "query",
            query = ?timeline_query,
            entity = %ent_path,
            primary,
            ?components,
            "primary component not found"
        );

        None
    }

    /// Retrieves the data associated with a list of `components` at the specified `indices`.
    ///
    /// If the associated data is found, it will be written into returned array at the appropriate
    /// index, or `None` otherwise.
    ///
    /// `row_indices` takes a list of options so that one can easily re-use the results obtained
    /// from [`Self::query`].
    ///
    /// See [`Self::query`] for more information.
    pub fn get<const N: usize>(
        &self,
        components: &[ComponentNameRef<'_>; N],
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
                .get(component)
                .and_then(|table| table.get(row_idx));
            results[i] = row;
        }

        results
    }

    /// Force the sorting of all indices.
    pub fn sort_indices(&mut self) {
        for index in self.indices.values_mut() {
            index.sort_indices();
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
    pub fn latest_at<'a, const N: usize>(
        &self,
        time: i64,
        primary: ComponentNameRef<'a>,
        components: &[ComponentNameRef<'a>; N],
    ) -> Option<[Option<RowIndex>; N]> {
        let timeline = self.timeline;

        // The time we're looking for gives us an upper bound: all components must be indexed
        // in either this bucket _or any of those that come before_!
        //
        // That is because secondary indices allow for null values, which forces us to not only
        // walk backwards within an index bucket, but sometimes even walk backwards across
        // multiple index buckets within the same table!

        for (attempt, bucket) in self.iter_bucket(time).enumerate() {
            debug!(
                kind = "query",
                timeline = %timeline.name(),
                time = timeline.typ().format(time.into()),
                primary,
                ?components,
                attempt,
                time_range = ?{
                    let time_range = bucket.indices.read().time_range;
                    time_range.min.as_i64()..=time_range.max.as_i64()
                },
                "found candidate bucket"
            );
            if let row_indices @ Some(_) = bucket.latest_at(time, primary, components) {
                return row_indices; // found at least the primary component!
            }
        }

        None // primary component not found
    }

    /// Returns the index bucket whose time range covers the given `time`.
    pub fn find_bucket_mut(&mut self, time: i64) -> &mut IndexBucket {
        // This cannot fail, `iter_bucket_mut` is guaranteed to always yield at least one bucket,
        // since index tables always spawn with a default bucket that covers [-∞;+∞].
        self.iter_bucket_mut(time).next().unwrap()
    }

    /// Returns an iterator that is guaranteed to yield at least one bucket, which is the bucket
    /// whose time range covers the given `time`.
    ///
    /// It then continues yielding buckets until it runs out, in decreasing time range order.
    pub fn iter_bucket(&self, time: i64) -> impl Iterator<Item = &IndexBucket> {
        self.buckets
            .range(..=TimeInt::from(time))
            .rev()
            .map(|(_, bucket)| bucket)
    }

    /// Returns an iterator that is guaranteed to yield at least one bucket, which is the bucket
    /// whose time range covers the given `time`.
    ///
    /// It then continues yielding buckets until it runs out, in decreasing time range order.
    pub fn iter_bucket_mut(&mut self, time: i64) -> impl Iterator<Item = &mut IndexBucket> {
        self.buckets
            .range_mut(..=TimeInt::from(time))
            .rev()
            .map(|(_, bucket)| bucket)
    }

    /// Force the sorting of all buckets.
    pub fn sort_indices(&self) {
        for bucket in self.buckets.values() {
            bucket.sort_indices();
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
    /// Sort all indices by time.
    pub fn sort_indices(&self) {
        if self.indices.read().is_sorted {
            return; // early read-only exit
        }

        self.indices.write().sort();
    }

    /// Returns `None` iff no row index could be found for the `primary` component.
    pub fn latest_at<'a, const N: usize>(
        &self,
        time: i64,
        primary: ComponentNameRef<'a>,
        components: &[ComponentNameRef<'_>; N],
    ) -> Option<[Option<RowIndex>; N]> {
        self.sort_indices();

        let IndexBucketIndices {
            is_sorted: _,
            time_range: _,
            times,
            indices,
        } = &*self.indices.read();

        // Early-exit if this bucket is unaware of this component.
        let index = indices.get(primary)?;

        debug!(
            kind = "query",
            primary,
            ?components,
            timeline = %self.timeline.name(),
            time = self.timeline.typ().format(time.into()),
            "searching for primary & secondary row indices..."
        );

        // find the primary index's row.
        let times = times.values();
        let primary_idx = times.partition_point(|t| *t <= time) as i64;

        // The partition point is always _beyond_ the index that we're looking for.
        // A partition point of 0 thus means that we're trying to query for data that lives
        // _before_ the beginning of time... there's nothing to be found there.
        if primary_idx == 0 {
            return None;
        }

        // The partition point is always _beyond_ the index that we're looking for; we need
        // to step back to find what we came for.
        let primary_idx = primary_idx - 1;
        debug!(
            kind = "query",
            primary,
            ?components,
            timeline = %self.timeline.name(),
            time = self.timeline.typ().format(time.into()),
            %primary_idx,
            "found primary index",
        );

        // find the secondary indices' rows, and the associated row indices.
        let mut secondary_idx = primary_idx;
        while !index.is_valid(secondary_idx as _) {
            secondary_idx -= 1;
            if secondary_idx < 0 {
                debug!(
                    kind = "query",
                    primary,
                    ?components,
                    timeline = %self.timeline.name(),
                    time = self.timeline.typ().format(time.into()),
                    %primary_idx,
                    "no secondary index found",
                );
                return None;
            }
        }

        debug!(
            kind = "query",
            primary,
            ?components,
            timeline = %self.timeline.name(),
            time = self.timeline.typ().format(time.into()),
            %primary_idx, %secondary_idx,
            "found secondary index",
        );
        debug_assert!(index.is_valid(secondary_idx as usize));

        let mut row_indices = [None; N];
        for (i, component) in components.iter().enumerate() {
            if let Some(index) = indices.get(*component) {
                if index.is_valid(secondary_idx as _) {
                    let row_idx = index.values()[secondary_idx as usize];
                    debug!(
                        kind = "query",
                        primary,
                        component,
                        timeline = %self.timeline.name(),
                        time = self.timeline.typ().format(time.into()),
                        %primary_idx, %secondary_idx, %row_idx,
                        "found row index",
                    );
                    row_indices[i] = Some(RowIndex::from_u64(row_idx));
                }
            }
        }

        Some(row_indices)
    }

    /// Whether the indices in this `IndexBucket` are sorted
    pub fn is_sorted(&self) -> bool {
        self.indices.read().is_sorted
    }

    /// Returns an (name, [`Int64Array`]) with a logical type matching the timeline.
    pub fn times(&self) -> (String, Int64Array) {
        let times = Int64Array::from(self.indices.read().times.clone());
        let logical_type = match self.timeline.typ() {
            re_log_types::TimeType::Time => DataType::Timestamp(TimeUnit::Nanosecond, None),
            re_log_types::TimeType::Sequence => DataType::Int64,
        };
        (self.timeline.name().to_string(), times.to(logical_type))
    }

    /// Returns a Vec each of (name, array) for each index in the bucket
    pub fn named_indices(&self) -> (Vec<String>, Vec<UInt64Array>) {
        self.indices
            .read()
            .indices
            .iter()
            .map(|(name, index)| (name.clone(), UInt64Array::from(index.clone())))
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
            let times = times.values();
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
            // The time index must always be dense, thus it shouldn't even have a validity
            // bitmap attached to it to begin with.
            debug_assert!(times.validity().is_none());

            let source = times.values().clone();
            let values = times.values_mut_slice();

            for (from, to) in swaps.iter().copied() {
                values[to] = source[from];
            }
        }

        fn reshuffle_index(index: &mut UInt64Vec, swaps: &[(usize, usize)]) {
            // shuffle data
            {
                let source = index.values().clone();
                let values = index.values_mut_slice();

                for (from, to) in swaps.iter().copied() {
                    values[to] = source[from];
                }
            }

            // shuffle validity bitmaps
            let validity_before = index.validity().cloned();
            let validity_after = validity_before.clone();
            if let (Some(validity_before), Some(mut validity_after)) =
                (validity_before, validity_after)
            {
                for (from, to) in swaps.iter().copied() {
                    validity_after.set(to, validity_before.get(from));
                }

                // we expect as many nulls before and after.
                assert_eq!(validity_before.unset_bits(), validity_after.unset_bits());

                index.set_validity(Some(validity_after));
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
            .partition_point(|bucket| row_idx >= bucket.row_offset);

        // The partition point will give us the index of the first bucket that has a row offset
        // strictly greater than the row index we're looking for, therefore we need to take a
        // step back to find what we're looking for.
        //
        // Since component tables always spawn with a default bucket at offset 0, the smallest
        // partition point that can ever be returned is one, thus this operation is overflow-safe.
        debug_assert!(bucket_nr > 0);
        bucket_nr -= 1;

        if let Some(bucket) = self.buckets.get(bucket_nr) {
            debug!(
                kind = "query",
                component = self.name.as_str(),
                %row_idx,
                bucket_nr,
                %bucket.row_offset,
                "fetching component data"
            );
            Some(bucket.get(row_idx))
        } else {
            debug!(
                kind = "query",
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
    /// Get this `ComponentBucket`s debug name
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a shallow clone of the row data for the given `row_idx`.
    pub fn get(&self, row_idx: RowIndex) -> Box<dyn Array> {
        let row_idx = row_idx.as_u64() - self.row_offset.as_u64();
        if self.retired {
            self.chunks[0].slice(row_idx as _, 1)
        } else {
            self.chunks[row_idx as usize].slice(0, 1)
        }
    }

    /// Returns an iterator over the data chunks in this component.
    pub fn data(&self) -> Vec<Box<dyn Array>> {
        self.chunks.clone() // shallow
    }

    /// Return an iterator over the time ranges in this bucket.
    #[allow(dead_code)]
    pub fn iter_time_ranges(&self) -> impl Iterator<Item = (&Timeline, &TimeRange)> {
        self.time_ranges.iter()
    }
}
