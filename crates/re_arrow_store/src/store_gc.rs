use std::{collections::HashMap, mem::needs_drop};

use arrow2::{
    array::{Array, ListArray},
    compute::aggregate::estimated_bytes_size,
};
use nohash_hasher::{IntMap, IntSet};
use re_log::{info, trace};
use re_log_types::{msg_bundle::Component, ComponentName, MsgId, TimeInt, TimeRange, Timeline};

use crate::{ComponentBucket, ComponentTable, DataStore, RowIndex, RowIndexKind};

// ---

// TODO: remove; dont need it in the end
#[derive(thiserror::Error, Debug)]
pub enum GarbageCollectionError {
    // Batches
    #[error("Percentage must be a float in the range [0.0 : 1.0], got {0}")]
    InvalidPercentage(GarbageCollectionTarget),
}

pub type GarbageCollectionResult<T> = ::std::result::Result<T, GarbageCollectionError>;

// ---

// TODO: invert responsibilities (this PR?)
// TODO: remove fine-grained locking (after discussions?)
// TODO: is it time to introduce changelogs? (next PR?)

#[derive(Debug, Clone, Copy)]
pub enum GarbageCollectionTarget {
    /// Try to drop _at most_ the given percentage.
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
    /// Returns the set of `MsgId`s that were removed from the store.
    //
    // TODO: pass the component driving the GC?
    pub fn gc(
        &mut self,
        primary: ComponentName,
        target: GarbageCollectionTarget,
    ) -> GarbageCollectionResult<Vec<Box<dyn Array>>> {
        puffin::profile_function!();

        let initial_size_bytes = self.total_temporal_component_size_bytes() as f64;

        let res = match target {
            GarbageCollectionTarget::DropAtLeastPercentage(p) => {
                if !(0.0..=1.0).contains(&p) {
                    return Err(GarbageCollectionError::InvalidPercentage(target));
                }

                let total_temporal_component_size_bytes =
                    self.total_temporal_component_size_bytes() as f64;

                let drop_at_least_size_bytes = total_temporal_component_size_bytes * p;
                let target_size_bytes =
                    total_temporal_component_size_bytes - drop_at_least_size_bytes;

                info!(
                    kind = "gc",
                    %target,
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

        let new_size_bytes = self.total_temporal_component_size_bytes() as f64;

        info!(
            kind = "gc",
            %target,
            initial_size_bytes = re_format::format_bytes(initial_size_bytes),
            new_size_bytes = re_format::format_bytes(new_size_bytes),
            "GC done"
        );

        Ok(res)
    }

    // TODO: doc
    fn gc_drop_at_least_size_bytes(
        &mut self,
        primary: ComponentName,
        mut drop_at_least_size_bytes: f64,
    ) -> Vec<Box<dyn Array>> {
        let mut dropped = Vec::<Box<dyn Array>>::new();

        while drop_at_least_size_bytes > 0.0 {
            let Some(primary_bucket) = self
                .components
                .get_mut(&primary)
                .and_then(|table| (table.buckets.len() > 1).then(|| table.buckets.pop_front()))
                .flatten()
            else {
                break
            };

            drop_at_least_size_bytes -= primary_bucket.total_size_bytes() as f64;

            for table in self.components.values_mut() {
                while table.buckets.len() > 1 {
                    let bucket = table.buckets.front().unwrap();
                    if primary_bucket.contains(&bucket.time_ranges) {
                        let bucket = table.buckets.pop_front().unwrap();
                        drop_at_least_size_bytes -= bucket.total_size_bytes() as f64;
                    } else {
                        // dbg!(&primary_bucket.time_ranges);
                        // dbg!(&bucket.time_ranges);
                        break;
                    }
                }
            }

            // TODO: remove indices... maybe? I kinda like those tombstones tho
            // TODO: make optional?
            for ((timeline, _), table) in &mut self.indices {
                while table.buckets.len() > 1 {
                    let time_range = table
                        .buckets
                        .values()
                        .next()
                        .unwrap()
                        .indices
                        .read()
                        .time_range;
                    if primary_bucket.contains(&[(*timeline, time_range)].into()) {
                        let bucket = table
                            .buckets
                            .remove(&table.buckets.keys().next().unwrap().clone())
                            .unwrap();
                        drop_at_least_size_bytes -= bucket.total_size_bytes() as f64;
                    } else {
                        break;
                    }

                    // TODO: i figure we just killed the -inf indexing time?
                }
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
    // TODO: doc
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
