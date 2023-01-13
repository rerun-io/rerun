use std::collections::HashMap;

use arrow2::array::{Array, ListArray};
use re_log::info;
use re_log_types::{ComponentName, TimeRange, Timeline};

use crate::{ComponentBucket, DataStore};

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
    /// `primary_component` and `primary_timeline`.
    /// Returns all the raw data that was removed from the store for the given `primary_component`.
    ///
    /// This only affects component tables, indices are left as-is, effectively behaving as
    /// tombstones.
    ///
    /// The garbage collection is based on _insertion order_, which makes it both very efficient
    /// and very simple from an implementation standpoint.
    /// The tradeoff is that the given `primary_timeline` is expected to roughly follow insertion
    /// order, otherwise the behaviour is essentially undefined.
    pub fn gc(
        &mut self,
        target: GarbageCollectionTarget,
        primary_timeline: Timeline,
        primary_component: ComponentName,
    ) -> Vec<Box<dyn Array>> {
        puffin::profile_function!();

        self.gc_id += 1;

        let initial_nb_rows = self.total_temporal_component_rows();
        let initial_size_bytes = self.total_temporal_component_size_bytes() as f64;

        let res = match target {
            GarbageCollectionTarget::DropAtLeastPercentage(p) => {
                assert!((0.0..=1.0).contains(&p));

                let drop_at_least_size_bytes = initial_size_bytes * p;
                let target_size_bytes = initial_size_bytes - drop_at_least_size_bytes;

                info!(
                    kind = "gc",
                    id = self.gc_id,
                    %target,
                    timeline = %primary_timeline.name(),
                    %primary_component,
                    initial_nb_rows = re_format::format_large_number(initial_nb_rows as _),
                    initial_size_bytes = re_format::format_bytes(initial_size_bytes),
                    target_size_bytes = re_format::format_bytes(target_size_bytes),
                    drop_at_least_size_bytes = re_format::format_bytes(drop_at_least_size_bytes),
                    "starting GC"
                );

                self.gc_drop_at_least_size_bytes(
                    primary_timeline,
                    primary_component,
                    drop_at_least_size_bytes,
                )
            }
        };

        #[cfg(debug_assertions)]
        self.sanity_check().unwrap();

        let new_nb_rows = self.total_temporal_component_rows();
        let new_size_bytes = self.total_temporal_component_size_bytes() as f64;

        info!(
            kind = "gc",
            id = self.gc_id,
            %target,
            timeline = %primary_timeline.name(),
            %primary_component,
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
        primary_timeline: Timeline,
        primary_component: ComponentName,
        mut drop_at_least_size_bytes: f64,
    ) -> Vec<Box<dyn Array>> {
        let mut dropped = Vec::<Box<dyn Array>>::new();

        while drop_at_least_size_bytes > 0.0 {
            // Find and drop the earliest (in terms of _insertion order_) primary component bucket
            // that we can find.
            let Some(primary_bucket) = self
                .components
                .get_mut(&primary_component)
                .and_then(|table| (table.buckets.len() > 1).then(|| table.buckets.pop_front()))
                .flatten()
            else {
                break;
            };

            drop_at_least_size_bytes -= primary_bucket.total_size_bytes() as f64;

            // From there, find and drop all component buckets (in _insertion order_) that do not
            // contain any data more recent than the time range covered by the primary
            // component bucket (for the primary timeline!).
            for table in self
                .components
                .iter_mut()
                .filter_map(|(component, table)| (*component != primary_component).then_some(table))
            {
                while table.buckets.len() > 1 {
                    let bucket = table.buckets.front().unwrap();
                    if primary_bucket.encompasses(primary_timeline, &bucket.time_ranges) {
                        let bucket = table.buckets.pop_front().unwrap();
                        drop_at_least_size_bytes -= bucket.total_size_bytes() as f64;
                    } else {
                        break;
                    }
                }
            }

            // We don't collect indices: they behave as tombstones.

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
    /// Does `self` fully encompass `time_ranges` for the given `primary_timeline`?
    fn encompasses(
        &self,
        primary_timeline: Timeline,
        time_ranges: &HashMap<Timeline, TimeRange>,
    ) -> bool {
        if let (Some(time_range1), Some(time_range2)) = (
            self.time_ranges.get(&primary_timeline),
            time_ranges.get(&primary_timeline),
        ) {
            return time_range1.max >= time_range2.max;
        }

        // There's only one way this can happen: this is a bucket that only holds the fake row at
        // offset #0.
        // Ignore it.
        true
    }
}
