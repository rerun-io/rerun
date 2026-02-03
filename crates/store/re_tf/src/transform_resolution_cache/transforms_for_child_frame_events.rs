use std::collections::BTreeSet;

use re_byte_size::{BookkeepingBTreeMap, SizeBytes};
use re_log_types::TimeInt;

use super::cached_transform_value::CachedTransformValue;
use super::parent_from_child_transform::ParentFromChildTransform;
use super::resolved_pinhole_projection::ResolvedPinholeProjection;

// TODO(RR-3539): replace this with a range-map, mapping non-overlapping
// time ranges to transforms. That way we can avoid storing the same value multiple times, saving a lot of memory.
// Then we probably wouldn't need the BookkeepingBTreeMap either.
pub type FrameTransformTimeMap =
    BookkeepingBTreeMap<TimeInt, CachedTransformValue<ParentFromChildTransform>>;

// TODO(RR-3539): replace this with a range-map, mapping non-overlapping
// time ranges to transforms. That way we can avoid storing the same value multiple times, saving a lot of memory.
// Then we probably wouldn't need the BookkeepingBTreeMap either.
pub type PinholeProjectionMap =
    BookkeepingBTreeMap<TimeInt, CachedTransformValue<ResolvedPinholeProjection>>;

#[derive(Clone, Debug, PartialEq)]
pub struct TransformsForChildFrameEvents {
    /// There can be only a single parent at any point in time, but it may change over time.
    /// Whenever it changes, the previous parent frame is no longer reachable.
    pub frame_transforms: FrameTransformTimeMap,

    pub pinhole_projections: PinholeProjectionMap,
}

impl TransformsForChildFrameEvents {
    pub fn new_empty() -> Self {
        Self {
            frame_transforms: Default::default(),
            pinhole_projections: Default::default(),
        }
    }

    /// Inserts a cleared transform for the given times.
    pub fn insert_clear(&mut self, time: TimeInt) {
        let Self {
            frame_transforms,
            pinhole_projections,
        } = self;

        frame_transforms.insert(time, CachedTransformValue::Cleared);
        pinhole_projections.insert(time, CachedTransformValue::Cleared);
    }

    /// Insert several cleared transforms for the given times.
    pub fn insert_clears(&mut self, times: &BTreeSet<TimeInt>) {
        for &time in times {
            self.insert_clear(time);
        }
    }

    /// Removes any events at a given time (if any).
    pub fn remove_at(&mut self, time: TimeInt) {
        let Self {
            frame_transforms,
            pinhole_projections,
        } = self;

        frame_transforms.remove(&time);
        pinhole_projections.remove(&time);
    }

    pub fn is_empty(&self) -> bool {
        let Self {
            frame_transforms,
            pinhole_projections,
        } = self;

        frame_transforms.is_empty() && pinhole_projections.is_empty()
    }
}

impl SizeBytes for TransformsForChildFrameEvents {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            frame_transforms,
            pinhole_projections,
        } = self;

        frame_transforms.heap_size_bytes() + pinhole_projections.heap_size_bytes()
    }
}
