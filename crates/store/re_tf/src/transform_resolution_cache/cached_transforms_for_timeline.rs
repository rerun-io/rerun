use std::collections::BTreeSet;

use nohash_hasher::IntMap;
use re_byte_size::SizeBytes;
use re_chunk_store::ChunkStore;
use re_log_types::{EntityPath, EntityPathHash, TimeInt, TimelineName};

use crate::TransformFrameIdHash;
use crate::frame_id_registry::FrameIdRegistry;
use crate::transform_aspect::TransformAspect;

use super::iter_child_frames_in_chunk;
use super::pose_transform_for_entity::PoseTransformForEntity;
use super::tree_transforms_for_child_frame::TreeTransformsForChildFrame;

/// Cached transforms for a single timeline.
///
/// Includes any static transforms that may apply globally.
/// Therefore, this can't be trivially constructed.
pub struct CachedTransformsForTimeline {
    /// Transforms information for each child frame to a parent frame over time.
    // Note that these are potentially a lot of mutexes, but `parking_lot`-Mutex are incredibly lightweight on all platforms, so not a memory concern.
    pub per_child_frame_transforms: IntMap<TransformFrameIdHash, TreeTransformsForChildFrame>,

    /// Instance pose information for each entity over time.
    ///
    /// Unlike all other transforms, poses are associated with an entity path, not a frame.
    pub per_entity_poses: IntMap<EntityPathHash, PoseTransformForEntity>,

    /// We need to keep track of all clears that ever happened and when.
    /// Otherwise, new incoming frames may not correctly change their transform at the time of clear.
    pub non_recursive_clears: IntMap<EntityPath, BTreeSet<TimeInt>>,

    /// We need to keep track of all recursive clears that ever happened and when.
    /// Otherwise, new incoming frames may not correctly change their transform at the time of clear.
    pub recursive_clears: IntMap<EntityPath, BTreeSet<TimeInt>>,
}

impl CachedTransformsForTimeline {
    // `CachedTransformsForTimeline` intentionally doesn't implement `Default`
    // to not accidentally create it without considering static transforms.
    pub fn new_static() -> Self {
        Self {
            per_child_frame_transforms: Default::default(),
            per_entity_poses: Default::default(),
            non_recursive_clears: Default::default(),
            recursive_clears: Default::default(), // Unused for static timeline.
        }
    }

    pub fn new_temporal(
        timeline: TimelineName,
        static_transforms: &Self,
        frame_id_registry: &FrameIdRegistry,
        chunk_store: &ChunkStore,
    ) -> Self {
        re_tracing::profile_function!(timeline);

        // First create the base structure from static transforms.
        let mut result = Self {
            per_child_frame_transforms: static_transforms
                .per_child_frame_transforms
                .iter()
                .map(|(transform_frame, static_transforms)| {
                    (
                        *transform_frame,
                        TreeTransformsForChildFrame::new_for_new_empty_timeline(
                            timeline,
                            static_transforms,
                        ),
                    )
                })
                .collect(),
            per_entity_poses: static_transforms.per_entity_poses.clone(),
            non_recursive_clears: IntMap::default(),
            recursive_clears: IntMap::default(),
        };

        re_tracing::profile_scope!("chunks");

        // Then process all temporal chunks for this timeline.
        for chunk in chunk_store.iter_physical_chunks() {
            if chunk.is_static() || !chunk.timelines().contains_key(&timeline) {
                continue;
            }

            let aspects = TransformAspect::transform_aspects_of(chunk);
            if aspects.is_empty() {
                continue;
            }

            result.add_temporal_chunk(
                chunk,
                aspects,
                timeline,
                static_transforms,
                frame_id_registry,
            );
        }

        result
    }

    /// Adds a temporal chunk to this timeline's cache.
    pub fn add_temporal_chunk(
        &mut self,
        chunk: &re_chunk_store::Chunk,
        aspects: TransformAspect,
        timeline: TimelineName,
        static_timeline: &Self,
        frame_id_registry: &FrameIdRegistry,
    ) {
        re_tracing::profile_function!();

        let entity_path = chunk.entity_path();

        let transform_child_frame_component =
            re_sdk_types::archetypes::Transform3D::descriptor_child_frame().component;
        let pinhole_child_frame_component =
            re_sdk_types::archetypes::Pinhole::descriptor_child_frame().component;

        if aspects.contains(TransformAspect::Frame) {
            for (time, frame) in
                iter_child_frames_in_chunk(chunk, timeline, transform_child_frame_component)
            {
                self.get_or_create_tree_transforms_temporal(
                    entity_path,
                    frame,
                    timeline,
                    static_timeline,
                    frame_id_registry,
                )
                .invalidate_transform_at(time);
            }
        }
        if aspects.contains(TransformAspect::Pose) {
            let poses = self.get_or_create_pose_transforms_temporal(entity_path, static_timeline);
            for (time, _) in chunk.iter_indices(&timeline) {
                poses.invalidate_at(time);
            }
        }
        if aspects.contains(TransformAspect::PinholeOrViewCoordinates) {
            for (time, frame) in
                iter_child_frames_in_chunk(chunk, timeline, pinhole_child_frame_component)
            {
                self.get_or_create_tree_transforms_temporal(
                    entity_path,
                    frame,
                    timeline,
                    static_timeline,
                    frame_id_registry,
                )
                .invalidate_pinhole_projection_at(time);
            }
        }

        // Keep track of clears.
        if aspects.contains(TransformAspect::Clear) {
            let component = re_sdk_types::archetypes::Clear::descriptor_is_recursive().component;

            for ((time, _row_id), is_recursive_slice) in chunk
                .iter_component_indices(timeline, component)
                .zip(chunk.iter_slices::<bool>(component))
            {
                if let Some(is_recursive) = is_recursive_slice.values().first()
                    && *is_recursive != 0
                {
                    self.add_recursive_clear(entity_path, time);
                } else {
                    self.add_clear(entity_path, time);
                }
            }
        }
    }

    /// Removes a temporal chunk from this timeline's cache.
    pub fn remove_chunk(
        &mut self,
        chunk: &re_chunk_store::Chunk,
        aspects: TransformAspect,
        timeline: TimelineName,
    ) {
        re_tracing::profile_function!();

        let entity_path = chunk.entity_path();

        let transform_child_frame_component =
            re_sdk_types::archetypes::Transform3D::descriptor_child_frame().component;
        let pinhole_child_frame_component =
            re_sdk_types::archetypes::Pinhole::descriptor_child_frame().component;

        // Remove any affected clears.
        if aspects.contains(TransformAspect::Clear) {
            let component = re_sdk_types::archetypes::Clear::descriptor_is_recursive().component;

            for ((time, _row_id), is_recursive_slice) in chunk
                .iter_component_indices(timeline, component)
                .zip(chunk.iter_slices::<bool>(component))
            {
                if let Some(is_recursive) = is_recursive_slice.values().first()
                    && *is_recursive != 0
                {
                    self.remove_recursive_clear(entity_path, time);
                } else {
                    self.remove_clear(entity_path, time);
                }
            }
        }

        // Remove existing data.
        if aspects.contains(TransformAspect::Frame) {
            for (time, frame) in
                iter_child_frames_in_chunk(chunk, timeline, transform_child_frame_component)
            {
                if let Some(transforms) = self.per_child_frame_transforms.get_mut(&frame) {
                    transforms.events.get_mut().frame_transforms.remove(&time);
                }
            }
        }
        if aspects.contains(TransformAspect::Pose)
            && let Some(poses) = self.per_entity_poses.get_mut(&entity_path.hash())
        {
            for (time, _) in chunk.iter_indices(&timeline) {
                poses.poses_per_time.get_mut().remove(&time);
            }
        }
        if aspects.contains(TransformAspect::PinholeOrViewCoordinates) {
            for (time, frame) in
                iter_child_frames_in_chunk(chunk, timeline, pinhole_child_frame_component)
            {
                if let Some(transforms) = self.per_child_frame_transforms.get_mut(&frame) {
                    transforms
                        .events
                        .get_mut()
                        .pinhole_projections
                        .remove(&time);
                }
            }
        }

        // Remove any empty transform collection.
        self.per_child_frame_transforms
            .retain(|_frame, transforms| !transforms.events.get_mut().is_empty());
    }

    pub fn get_or_create_tree_transforms_temporal(
        &mut self,
        entity_path: &EntityPath,
        child_frame: TransformFrameIdHash,
        timeline: TimelineName,
        static_timeline: &Self,
        frame_registry: &FrameIdRegistry,
    ) -> &mut TreeTransformsForChildFrame {
        match self.per_child_frame_transforms.entry(child_frame) {
            std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                let transforms = occupied_entry.into_mut();

                // Make sure we have the right associated path (we may only have a static association so far).
                match transforms.associated_entity_path_temporal.as_mut() {
                    Some(existing_path) => {
                        if existing_path != entity_path {
                            re_log::error_once!(
                                "The entity path associated with a child frame mustn't change except for static vs temporal data. The frame {:?} was previously logged temporally at the path {existing_path:?} and was now logged on {entity_path:?}.",
                                frame_registry.lookup_frame_id(child_frame).map_or_else(
                                    || format!("{child_frame:?}"),
                                    ToString::to_string
                                )
                            );
                        }
                    }
                    None => {
                        transforms.associated_entity_path_temporal = Some(entity_path.clone());
                    }
                }

                transforms
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(TreeTransformsForChildFrame::new_temporal(
                    entity_path.clone(),
                    child_frame,
                    timeline,
                    static_timeline,
                    &self.non_recursive_clears,
                    &self.recursive_clears,
                ))
            }
        }
    }

    pub fn get_or_create_tree_transforms_static(
        &mut self,
        entity_path: &EntityPath,
        child_frame: TransformFrameIdHash,
        frame_registry: &FrameIdRegistry,
    ) -> &mut TreeTransformsForChildFrame {
        match self.per_child_frame_transforms.entry(child_frame) {
            std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                let transforms = occupied_entry.into_mut();

                // Make sure we have the right associated path (we may only have a temporal association so far).
                match transforms.associated_entity_path_static.as_mut() {
                    Some(existing_path) => {
                        if existing_path != entity_path {
                            re_log::error_once!(
                                "The entity path associated with a child frame mustn't change except for static vs temporal data. The frame {:?} was previously logged statically at the path {existing_path:?} and was now logged on {entity_path:?}.",
                                frame_registry.lookup_frame_id(child_frame).map_or_else(
                                    || format!("{child_frame:?}"),
                                    ToString::to_string
                                )
                            );
                        }
                    }
                    None => {
                        transforms.associated_entity_path_static = Some(entity_path.clone());
                    }
                }

                transforms
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => vacant_entry.insert(
                TreeTransformsForChildFrame::new_static(entity_path.clone(), child_frame),
            ),
        }
    }

    pub fn get_or_create_pose_transforms_temporal(
        &mut self,
        entity_path: &EntityPath,
        static_timeline: &Self,
    ) -> &mut PoseTransformForEntity {
        self.per_entity_poses
            .entry(entity_path.hash())
            .or_insert_with(|| {
                PoseTransformForEntity::new(
                    entity_path.clone(),
                    static_timeline,
                    &self.non_recursive_clears,
                    &self.recursive_clears,
                )
            })
    }

    pub fn get_or_create_pose_transforms_static(
        &mut self,
        entity_path: &EntityPath,
    ) -> &mut PoseTransformForEntity {
        self.per_entity_poses
            .entry(entity_path.hash())
            .or_insert_with(|| PoseTransformForEntity::new_empty(entity_path.clone()))
    }

    pub fn add_clear(&mut self, cleared_path: &EntityPath, cleared_time: TimeInt) {
        // Figure out who is affected by this new clear.
        // We generally assume Clears are quite rare, so just loop over all frames that it affects
        // and insert a cleared transform if necessary.
        for transforms in self.per_child_frame_transforms.values_mut() {
            if transforms.associated_entity_path_temporal.as_ref() == Some(cleared_path) {
                transforms.events.get_mut().insert_clear(cleared_time);
            }
        }
        if let Some(poses) = self.per_entity_poses.get_mut(&cleared_path.hash()) {
            poses.insert_clear(cleared_time);
        }

        // Store for future reference, so we can apply this on incoming.
        self.non_recursive_clears
            .entry(cleared_path.clone())
            .or_default()
            .insert(cleared_time);
    }

    pub fn add_recursive_clear(
        &mut self,
        recursively_cleared_path: &EntityPath,
        cleared_time: TimeInt,
    ) {
        // Figure out who is affected by this new clear.
        // We generally assume Clears are quite rare, so just loop over all frames that it affects
        // and insert a cleared transform if necessary.
        for transforms in self.per_child_frame_transforms.values_mut() {
            if transforms
                .associated_entity_path_temporal
                .as_ref()
                .is_some_and(|path| path.starts_with(recursively_cleared_path))
            {
                transforms.events.get_mut().insert_clear(cleared_time);
            }
        }

        for poses in self.per_entity_poses.values_mut() {
            if poses.entity_path.starts_with(recursively_cleared_path) {
                poses.insert_clear(cleared_time);
            }
        }

        // Store for future reference.
        self.recursive_clears
            .entry(recursively_cleared_path.clone())
            .or_default()
            .insert(cleared_time);
    }

    pub fn remove_clear(&mut self, cleared_path: &EntityPath, cleared_time: TimeInt) {
        let std::collections::hash_map::Entry::Occupied(mut clear_entry) =
            self.non_recursive_clears.entry(cleared_path.clone())
        else {
            return;
        };
        clear_entry.get_mut().remove(&cleared_time);
        if clear_entry.get().is_empty() {
            clear_entry.remove();
        }

        // Figure out who is no longer affected by this Clear and remove entry.
        for transforms in self.per_child_frame_transforms.values_mut() {
            if transforms.associated_entity_path_temporal.as_ref() == Some(cleared_path) {
                transforms.events.get_mut().remove_at(cleared_time);
            }
        }
        if let Some(poses) = self.per_entity_poses.get_mut(&cleared_path.hash()) {
            poses.poses_per_time.get_mut().remove(&cleared_time);
        }
    }

    pub fn remove_recursive_clear(
        &mut self,
        recursively_cleared_path: &EntityPath,
        cleared_time: TimeInt,
    ) {
        let std::collections::hash_map::Entry::Occupied(mut clear_entry) = self
            .recursive_clears
            .entry(recursively_cleared_path.clone())
        else {
            return;
        };
        clear_entry.get_mut().remove(&cleared_time);
        if clear_entry.get().is_empty() {
            clear_entry.remove();
        }

        // Figure out who is no longer affected by this recursive Clear and remove entry.
        for transforms in self.per_child_frame_transforms.values_mut() {
            if transforms
                .associated_entity_path_temporal
                .as_ref()
                .is_some_and(|path| path.starts_with(recursively_cleared_path))
            {
                transforms.events.get_mut().remove_at(cleared_time);
            }
        }
        for poses in self.per_entity_poses.values_mut() {
            if poses.entity_path.starts_with(recursively_cleared_path) {
                poses.poses_per_time.get_mut().remove(&cleared_time);
            }
        }
    }

    /// Returns all transforms for a given child frame.
    #[inline]
    pub fn frame_transforms(
        &self,
        source_frame: TransformFrameIdHash,
    ) -> Option<&TreeTransformsForChildFrame> {
        self.per_child_frame_transforms.get(&source_frame)
    }

    /// Returns all instance poses for a given entity path.
    #[inline]
    pub fn pose_transforms(&self, entity_path: EntityPathHash) -> Option<&PoseTransformForEntity> {
        self.per_entity_poses.get(&entity_path)
    }

    /// All child frames for which we have connections to a parent.
    pub fn all_child_frames(&self) -> impl Iterator<Item = TransformFrameIdHash> {
        self.per_child_frame_transforms.keys().copied()
    }
}

impl SizeBytes for CachedTransformsForTimeline {
    fn heap_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        let Self {
            per_child_frame_transforms,
            non_recursive_clears,
            recursive_clears,
            per_entity_poses,
        } = self;

        per_child_frame_transforms.heap_size_bytes()
            + non_recursive_clears.heap_size_bytes()
            + recursive_clears.heap_size_bytes()
            + per_entity_poses.heap_size_bytes()
    }
}

impl re_byte_size::MemUsageTreeCapture for CachedTransformsForTimeline {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_tracing::profile_function!();

        let Self {
            per_child_frame_transforms,
            non_recursive_clears,
            recursive_clears,
            per_entity_poses,
        } = self;

        re_byte_size::MemUsageNode::new()
            .with_child(
                "per_child_frame_transforms",
                per_child_frame_transforms.total_size_bytes(),
            )
            .with_child(
                "non_recursive_clears",
                non_recursive_clears.total_size_bytes(),
            )
            .with_child("recursive_clears", recursive_clears.total_size_bytes())
            .with_child("per_entity_poses", per_entity_poses.total_size_bytes())
            .into_tree()
    }
}
