use std::collections::HashMap;

use arrow2::array::{Array, ListArray};
use re_log::info;
use re_log_types::{ComponentName, TimeInt, TimeRange, Timeline};

use crate::{ComponentBucket, DataStore, IndexBucket, IndexTable};

// ---

#[derive(Debug, Clone, Copy)]
pub enum GarbageCollectionTarget {
    /// Try to drop _at least_ the given percentage.
    ///
    /// The percentage must be a float in the range [0.0 : 1.0].
    DropAtLeastPercentage(f64),
}

impl std::fmt::Display for GarbageCollectionTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GarbageCollectionTarget::DropAtLeastPercentage(p) => f.write_fmt(format_args!(
                "DropAtLeast({}%)",
                re_format::format_f64(*p * 100.0)
            )),
        }
    }
}

impl DataStore {
    /// Triggers a garbage collection according to the desired `target`, driven by the specified
    /// `primary` component.
    ///
    /// Returns all the raw data that was removed from the store for the given `primary` component.
    ///
    /// The garbage collection is based on _insertion order_, which makes it both very efficient
    /// and very simple from an implementation standpoint, but it does come with a tradeoff: data
    /// written far enough "into the future" might be unexpectedly collected.
    pub fn gc(
        &mut self,
        primary: ComponentName,
        target: GarbageCollectionTarget,
    ) -> Vec<Box<dyn Array>> {
        puffin::profile_function!();

        self.gc_id += 1;

        let initial_nb_rows =
            self.total_temporal_index_rows() + self.total_temporal_component_rows();
        let initial_size_bytes = (self.total_temporal_index_size_bytes()
            + self.total_temporal_component_size_bytes()) as f64;

        let res = match target {
            GarbageCollectionTarget::DropAtLeastPercentage(p) => {
                assert!((0.0..=1.0).contains(&p));

                let drop_at_least_size_bytes = initial_size_bytes * p;
                let target_size_bytes = initial_size_bytes - drop_at_least_size_bytes;

                info!(
                    kind = "gc",
                    id = self.gc_id,
                    %target,
                    initial_nb_rows = re_format::format_large_number(initial_nb_rows as _),
                    initial_size_bytes = re_format::format_bytes(initial_size_bytes),
                    target_size_bytes = re_format::format_bytes(target_size_bytes),
                    drop_at_least_size_bytes = re_format::format_bytes(drop_at_least_size_bytes),
                    "starting GC"
                );

                self.gc_drop_at_least_size_bytes(primary, drop_at_least_size_bytes)
            }
        };

        #[cfg(debug_assertions)]
        self.sanity_check().unwrap();

        let new_nb_rows = self.total_temporal_index_rows() + self.total_temporal_component_rows();
        let new_size_bytes = (self.total_temporal_index_size_bytes()
            + self.total_temporal_component_size_bytes()) as f64;

        info!(
            kind = "gc",
            id = self.gc_id,
            %target,
            initial_nb_rows = re_format::format_large_number(initial_nb_rows as _),
            initial_size_bytes = re_format::format_bytes(initial_size_bytes),
            new_nb_rows = re_format::format_large_number(new_nb_rows as _),
            new_size_bytes = re_format::format_bytes(new_size_bytes),
            "GC done"
        );

        res
    }

    fn gc_drop_at_least_size_bytes(
        &mut self,
        primary: ComponentName,
        mut drop_at_least_size_bytes: f64,
    ) -> Vec<Box<dyn Array>> {
        let mut dropped = Vec::<Box<dyn Array>>::new();

        while drop_at_least_size_bytes > 0.0 {
            // Find and drop the earliest (in _insertion order_) primary component bucket that we
            // can find.
            let Some(primary_bucket) = self
                .components
                .get_mut(&primary)
                .and_then(|table| (table.buckets.len() > 1).then(|| table.buckets.pop_front()))
                .flatten()
            else {
                break;
            };

            drop_at_least_size_bytes -= primary_bucket.total_size_bytes() as f64;

            // From there, find and drop all component buckets (in _insertion order_) that do not
            // contain any data that's more recent than the time range covered by the primary
            // component bucket.
            for table in self.components.values_mut() {
                while table.buckets.len() > 1 {
                    let bucket = table.buckets.front().unwrap();
                    if primary_bucket.contains(&bucket.time_ranges) {
                        let bucket = table.buckets.pop_front().unwrap();
                        drop_at_least_size_bytes -= bucket.total_size_bytes() as f64;
                    } else {
                        break;
                    }
                }
            }

            // Find and drop all index buckets (in _time order_) that are fully encompassed by the
            // time ranges of the primary bucket we've just dropped.
            //
            // There's a tradeoff here: if one or more buckets at further points in time still
            // refer to data within the dead primary component bucket (i.e. because the original
            // insertions were done for timepoints "far into the future"), they will now refer to
            // deleted data (read requests will return `None`s).
            for ((timeline, _), table) in &mut self.indices {
                while table.buckets.len() > 1 {
                    let time_range = table.first_time_range().unwrap();
                    if primary_bucket.contains(&[(*timeline, time_range)].into()) {
                        let bucket = table.pop_first_bucket().unwrap();
                        drop_at_least_size_bytes -= bucket.total_size_bytes() as f64;
                    } else {
                        break;
                    }
                }

                // The first index bucket of every table should always cover the smallest possible
                // indexing time: make sure this is still the case!
                let bucket = table.pop_first_bucket().unwrap();
                table.buckets.insert(TimeInt::MIN, bucket);
            }

            dropped.extend(primary_bucket.chunks.into_iter().map(|chunk| {
                chunk
                    .as_any()
                    .downcast_ref::<ListArray<i32>>()
                    .unwrap()
                    .values()
                    .clone()
            }));
        }

        dropped
    }
}

impl ComponentBucket {
    /// Does `self` fully encompass the given `time_ranges`?
    fn contains(&self, time_ranges: &HashMap<Timeline, TimeRange>) -> bool {
        for timeline2 in time_ranges.keys() {
            if !self.time_ranges.contains_key(timeline2) {
                return false;
            }
        }

        for (timeline1, time_range1) in &self.time_ranges {
            if let Some(time_range2) = time_ranges.get(timeline1) {
                if time_range2.max > time_range1.max {
                    return false;
                }
            }
        }

        true
    }
}

impl IndexTable {
    fn pop_first_bucket(&mut self) -> Option<IndexBucket> {
        self.buckets
            .keys()
            .next()
            .cloned()
            .and_then(|key| self.buckets.remove(&key))
    }

    fn first_time_range(&mut self) -> Option<TimeRange> {
        self.buckets
            .values()
            .next()
            .map(|bucket| bucket.indices.read().time_range)
    }
}
