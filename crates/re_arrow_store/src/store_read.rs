use std::sync::atomic::Ordering;

use arrow2::array::{Array, MutableArray, UInt64Vec};

use re_log::debug;
use re_log_types::{ComponentNameRef, ObjPath as EntityPath, TimeInt, Timeline};

use crate::{
    ComponentBucket, ComponentTable, DataStore, IndexBucket, IndexBucketIndices, IndexTable,
    RowIndex,
};

// ---

/// A query in time.
// TODO: include timeline in there, it really is just weird otherwise (new issue?).
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
    /// Queries the datastore to retrieve the internal indices of the specified `components`,
    /// as seen from the point of view of the so-called `primary` component.
    ///
    /// Returns `true` on success, `false` otherwise.
    /// Success is defined by one thing and thing only: whether a row index could be found for the
    /// `primary` component.
    ///
    /// * On success, this fills `row_indices` with the internal row index of each and every
    ///   component in `components`, or `None` if said component isn't available at that point
    ///   in time.
    ///
    /// * On failure, the contents of `row_indices` is left untouched.
    ///
    /// If `components` is left empty, the indices for all components available at this point in
    /// time and from this point of view will be queried for.
    /// TODO: that's actually an issue, should we go this route: maybe another method that allocs?
    ///
    /// Panics if `components.len() > indices.len()`.
    //
    // TODO: do we want another route for query_any? requires dynamic allocations tho
    // TODO: add missing test confirming latest_at behavior
    // TODO: add new test confirming refined model
    pub fn query(
        &self,
        timeline: &Timeline,
        time_query: &TimeQuery,
        ent_path: &EntityPath,
        primary: ComponentNameRef<'_>,
        components: &[ComponentNameRef<'_>],
        row_indices: &mut [Option<RowIndex>],
    ) -> bool {
        debug_assert!(
            components.len() == row_indices.len(),
            "expect components.len() == row_indices.len()"
        );

        // TODO(cmc): kind & query_id need to somehow propagate through the span system.
        self.query_id.fetch_add(1, Ordering::Relaxed);

        let ent_path_hash = ent_path.hash();
        let latest_at = match time_query {
            TimeQuery::LatestAt(latest_at) => *latest_at,
            #[allow(clippy::todo)]
            TimeQuery::Range(_) => todo!("implement range queries!"),
        };

        row_indices.fill(None);

        debug!(
            kind = "query",
            id = self.query_id.load(Ordering::Relaxed),
            timeline = %timeline.name(),
            time = timeline.typ().format(latest_at.into()),
            entity = %ent_path,
            primary,
            ?components,
            "query started..."
        );

        if let Some(index) = self.indices.get(&(*timeline, *ent_path_hash)) {
            if index.latest_at(latest_at, primary, components, row_indices) {
                debug!(
                    kind = "query",
                    timeline = %timeline.name(),
                    time = timeline.typ().format(latest_at.into()),
                    entity = %ent_path,
                    primary,
                    ?components,
                    ?row_indices,
                    "row indices fetched"
                );
                return true;
            }
        }

        debug!(
            kind = "query",
            timeline = %timeline.name(),
            time = timeline.typ().format(latest_at.into()),
            entity = %ent_path,
            primary,
            ?components,
            ?row_indices,
            "primary component not found"
        );

        false
    }

    // TODO: doc
    // TODO: explain why row_indices takes options
    pub fn get(
        &self,
        components: &[ComponentNameRef<'_>],
        row_indices: &[Option<RowIndex>],
        results: &mut [Option<Box<dyn Array>>],
    ) {
        debug_assert!(
            components.len() == row_indices.len() && row_indices.len() == results.len(),
            "expect components.len() == row_indices.len() == results.len()"
        );

        results.fill(None);

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
    pub fn iter_indices(&self) -> impl Iterator<Item = ((Timeline, EntityPath), &IndexTable)> {
        self.indices.iter().map(|((timeline, _), table)| {
            ((*timeline, table.ent_path.clone() /* shallow */), table)
        })
    }
}

// --- Indices ---

impl IndexTable {
    // TODO: doc
    // TODO: `indices` is left untouched if not found
    pub fn latest_at<'a>(
        &self,
        time: i64,
        primary: ComponentNameRef<'a>,
        components: &[ComponentNameRef<'a>],
        row_indices: &mut [Option<RowIndex>],
    ) -> bool {
        debug_assert!(
            components.len() == row_indices.len(),
            "expect components.len() == row_indices.len()"
        );

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
            if bucket.latest_at(time, primary, components, row_indices) {
                return true; // found at least the primary component!
            }
        }

        false // even the primary component doesn't exist
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
    pub fn iter_buckets(&self) -> impl Iterator<Item = &IndexBucket> {
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

    // TODO: doc
    // TODO: return true when found
    // TODO: return false if nothing could happen because the primary component doesn't exist
    pub fn latest_at<'a>(
        &self,
        time: i64,
        primary: ComponentNameRef<'a>,
        components: &[ComponentNameRef<'_>],
        row_indices: &mut [Option<RowIndex>],
    ) -> bool {
        debug_assert!(
            components.len() == row_indices.len(),
            "expect components.len() == row_indices.len()"
        );

        self.sort_indices();

        let IndexBucketIndices {
            is_sorted: _,
            time_range: _,
            times,
            indices,
        } = &*self.indices.read();

        // Early-exit if this bucket is unaware of this component.
        let Some(index) = indices.get(primary) else { return false; };

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
            return false;
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
                return false;
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

        for (i, component) in components.iter().enumerate() {
            if let Some(index) = indices.get(*component) {
                if index.is_valid(secondary_idx as _) {
                    row_indices[i] = Some(index.values()[secondary_idx as usize]);
                }
            }
        }

        true
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
                row_idx,
                bucket_nr,
                bucket.row_offset,
                "fetching component data"
            );
            Some(bucket.get(row_idx))
        } else {
            debug!(
                kind = "query",
                component = self.name.as_str(),
                row_idx,
                bucket_nr,
                "row index is out of bounds"
            );
            None
        }
    }
}
impl ComponentBucket {
    // Panics on out-of-bounds
    pub fn get(&self, row_idx: u64) -> Box<dyn Array> {
        let row_idx = row_idx - self.row_offset;
        self.data.slice(row_idx as usize, 1)
    }
}
