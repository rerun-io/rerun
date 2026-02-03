use std::collections::BTreeSet;

use glam::DAffine3;
use nohash_hasher::IntMap;
use re_byte_size::{BookkeepingBTreeMap, SizeBytes};
use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, TimeInt};
use re_mutex::Mutex;

use crate::transform_queries::query_and_resolve_instance_poses_at_entity;

use super::cached_transform_value::{
    CachedTransformValue, add_invalidated_entry_if_not_already_cleared,
};
use super::cached_transforms_for_timeline::CachedTransformsForTimeline;

/// All instance poses for a given entity over time.
///
/// Similar to [`super::tree_transforms_for_child_frame::TreeTransformsForChildFrame`], but for poses associated with an entity path.
#[derive(Debug)]
pub struct PoseTransformForEntity {
    pub entity_path: EntityPath,
    pub poses_per_time: Mutex<BookkeepingBTreeMap<TimeInt, CachedTransformValue<Vec<DAffine3>>>>,
}

impl Clone for PoseTransformForEntity {
    fn clone(&self) -> Self {
        Self {
            entity_path: self.entity_path.clone(),
            poses_per_time: Mutex::new(self.poses_per_time.lock().clone()),
        }
    }
}

impl SizeBytes for PoseTransformForEntity {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            entity_path,
            poses_per_time,
        } = self;

        entity_path.heap_size_bytes() + poses_per_time.lock().heap_size_bytes()
    }
}

impl PoseTransformForEntity {
    pub fn new(
        entity_path: EntityPath,
        static_timeline: &CachedTransformsForTimeline,
        non_recursive_clears: &IntMap<EntityPath, BTreeSet<TimeInt>>,
        recursive_clears: &IntMap<EntityPath, BTreeSet<TimeInt>>,
    ) -> Self {
        let mut poses = Self::new_empty(entity_path);

        // Take over static events.
        if let Some(static_transforms) = static_timeline
            .per_entity_poses
            .get(&poses.entity_path.hash())
        {
            *poses.poses_per_time.get_mut() = static_transforms.poses_per_time.lock().clone();
        }

        // Take over clear events.
        if let Some(cleared_times) = non_recursive_clears.get(&poses.entity_path) {
            poses.insert_clears(cleared_times);
        }
        for (recursively_cleared_path, times) in recursive_clears {
            if poses.entity_path.starts_with(recursively_cleared_path) {
                poses.insert_clears(times);
            }
        }

        poses
    }

    pub fn new_empty(entity_path: EntityPath) -> Self {
        Self {
            entity_path,
            poses_per_time: Mutex::new(BookkeepingBTreeMap::new()),
        }
    }

    pub fn latest_at_instance_poses(
        &self,
        entity_db: &EntityDb,
        query: &LatestAtQuery,
    ) -> Vec<DAffine3> {
        let mut poses_per_time = self.poses_per_time.lock();

        poses_per_time
            .mutate_latest_at(&query.at(), |_t, pose_transform| {
                // Separate check to work around borrow checker issues.
                if pose_transform == &CachedTransformValue::Invalidated {
                    *pose_transform =
                        CachedTransformValue::Resident(query_and_resolve_instance_poses_at_entity(
                            &self.entity_path,
                            entity_db,
                            query,
                        ));
                }

                match pose_transform {
                    CachedTransformValue::Resident(transform) => transform.clone(),
                    CachedTransformValue::Cleared => Vec::new(),
                    CachedTransformValue::Invalidated => {
                        unreachable!("Just made transform cache-resident")
                    }
                }
            })
            .unwrap_or_default()
    }

    /// Inserts a cleared transform for the given times.
    pub fn insert_clear(&mut self, time: TimeInt) {
        self.poses_per_time
            .get_mut()
            .insert(time, CachedTransformValue::Cleared);
    }

    /// Insert several cleared transforms for the given times.
    pub fn insert_clears(&mut self, time: &BTreeSet<TimeInt>) {
        self.poses_per_time
            .get_mut()
            .extend(time.iter().map(|t| (*t, CachedTransformValue::Cleared)));
    }

    /// Inserts an invalidation point for poses.
    pub fn invalidate_at(&mut self, time: TimeInt) {
        add_invalidated_entry_if_not_already_cleared(self.poses_per_time.get_mut(), time);
    }
}
