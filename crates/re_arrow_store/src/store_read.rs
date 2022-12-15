use std::{collections::HashMap, sync::atomic::Ordering};

use arrow2::{
    array::{Array, Int64Array, MutableArray, UInt64Array, UInt64Vec},
    datatypes::{DataType, TimeUnit},
};

use re_log::debug;
use re_log_types::{ComponentNameRef, ObjPath as EntityPath, TimeInt, TimeRange, Timeline};

use crate::{
    ComponentBucket, ComponentTable, DataStore, IndexBucket, IndexBucketIndices, IndexTable,
    RowIndex,
};

// ---

/// A query in time.
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
    pub fn query(
        &self,
        timeline: &Timeline,
        time_query: &TimeQuery,
        ent_path: &EntityPath,
        components: &[ComponentNameRef<'_>],
    ) -> anyhow::Result<polars_core::frame::DataFrame> {
        // TODO(cmc): kind & query_id need to somehow propagate through the span system.
        self.query_id.fetch_add(1, Ordering::Relaxed);

        let ent_path_hash = ent_path.hash();

        let latest_at = match time_query {
            TimeQuery::LatestAt(latest_at) => *latest_at,
            #[allow(clippy::todo)]
            TimeQuery::Range(_) => todo!("implement range queries!"),
        };

        debug!(
            kind = "query",
            id = self.query_id.load(Ordering::Relaxed),
            timeline = %timeline.name(),
            time = timeline.typ().format(latest_at.into()),
            entity = %ent_path,
            ?components,
            "query started..."
        );

        let row_indices = self
            .indices
            .get(&(*timeline, *ent_path_hash))
            .map(|index| index.latest_at(latest_at, components))
            .unwrap_or_default();
        debug!(
            kind = "query",
            timeline = %timeline.name(),
            time = timeline.typ().format(latest_at.into()),
            entity = %ent_path,
            ?components,
            ?row_indices,
            "row indices fetched"
        );

        let mut series: HashMap<_, _> = row_indices
            .into_iter()
            .filter_map(|(name, row_idx)| {
                self.components.get(name).and_then(|table| {
                    table.get(row_idx).map(|data| {
                        (
                            name,
                            polars_core::series::Series::try_from((name, data)).unwrap(),
                        )
                    })
                })
            })
            .collect();

        let series_ordered = components
            .iter()
            .filter_map(|name| series.remove(name))
            .collect();
        let df = polars_core::frame::DataFrame::new(series_ordered)?;

        df.explode(df.get_column_names()).map_err(Into::into)
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

    /// Returns a read-only iterator over the raw component tables [`ComponentTable`].
    ///
    /// Do _not_ use this to try and test the internal state of the datastore.
    pub fn iter_components(&self) -> impl ExactSizeIterator<Item = (&String, &ComponentTable)> {
        self.components.iter()
    }
}

// --- Indices ---

impl IndexTable {
    pub fn latest_at<'a>(
        &self,
        time: i64,
        components: &[ComponentNameRef<'a>],
    ) -> HashMap<ComponentNameRef<'a>, RowIndex> {
        let mut results = HashMap::with_capacity(components.len());

        let timeline = self.timeline;

        // The time we're looking for gives us an upper bound: all components must be indexed
        // in either this bucket _or any of those that come before_!
        //
        // That is because secondary indices allow for null values, which forces us to not only
        // walk backwards within an index bucket, but sometimes even walk backwards across
        // multiple index buckets within the same table!
        //
        // Besides, components are _independently_ nullable, and so this two-level backwards walk
        // needs to be done on a per-component basis.
        //
        // TODO(#529): keep track of the components we know of, and fast early-break for the
        // components we don't.
        for &name in components {
            'for_each_bucket: for (i, bucket) in self.iter_bucket(time).enumerate() {
                debug!(
                    kind = "query",
                    component = name,
                    timeline = %timeline.name(),
                    time = timeline.typ().format(time.into()),
                    attempt = i,
                    time_range = ?{
                        let time_range = bucket.indices.read().time_range;
                        time_range.min.as_i64()..=time_range.max.as_i64()
                    },
                    "found candidate bucket"
                );
                if let Some(row_idx) = bucket.latest_at(time, name) {
                    results.insert(name, row_idx);
                    break 'for_each_bucket;
                }
            }
        }

        results
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

    pub fn latest_at<'a>(&self, time: i64, component: ComponentNameRef<'a>) -> Option<RowIndex> {
        self.sort_indices();

        let IndexBucketIndices {
            is_sorted: _,
            time_range: _,
            times,
            indices,
        } = &*self.indices.read();

        // Early-exit if this bucket is unaware of this component.
        let index = indices.get(component)?;

        debug!(
            kind = "query",
            component,
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
            component,
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
                    component,
                    timeline = %self.timeline.name(),
                    time = self.timeline.typ().format(time.into()),
                    %primary_idx,
                    "no secondary index found",
                );
                return None;
            }
        }

        debug_assert!(index.is_valid(secondary_idx as usize));
        let row_idx = index.values()[secondary_idx as usize];

        debug!(
            kind = "query",
            component,
            timeline = %self.timeline.name(),
            time = self.timeline.typ().format(time.into()),
            %primary_idx, %secondary_idx, %row_idx,
            "found secondary index + row index",
        );

        Some(row_idx)
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
    pub fn name(&self) -> &str {
        &self.name
    }

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

    /// Returns an iterator over the `ComponentBucket` in this table
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

    // Panics on out-of-bounds
    pub fn get(&self, row_idx: u64) -> Box<dyn Array> {
        let row_idx = row_idx - self.row_offset;
        self.data.slice(row_idx as usize, 1)
    }

    /// Returns the entire data Array in this component
    pub fn data(&self) -> Box<dyn Array> {
        // shallow copy
        self.data.clone()
    }

    /// Return an iterator over the time ranges in this bucket
    #[allow(dead_code)]
    pub fn iter_time_ranges(&self) -> impl Iterator<Item = (&Timeline, &TimeRange)> {
        self.time_ranges.iter()
    }
}
