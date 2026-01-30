use std::collections::BTreeSet;

use ahash::HashMap;
use glam::DAffine3;
use itertools::{Either, izip};
use nohash_hasher::IntMap;
use parking_lot::Mutex;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_byte_size::{BookkeepingBTreeMap, SizeBytes};
use re_chunk_store::external::arrow;
use re_chunk_store::{Chunk, LatestAtQuery};
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, EntityPathHash, TimeInt, TimelineName};
use re_sdk_types::{ComponentIdentifier, archetypes, components};

use crate::TransformFrameIdHash;
use crate::frame_id_registry::FrameIdRegistry;
use crate::transform_aspect::TransformAspect;
use crate::transform_queries::{
    query_and_resolve_instance_poses_at_entity, query_and_resolve_pinhole_projection_at_entity,
    query_and_resolve_tree_transform_at_entity,
};

/// Resolves all transform relationship defining components to affine transforms for fast lookup.
///
/// It only handles resulting transforms individually to each frame connection, not how these transforms propagate in the tree.
/// For transform tree propagation see [`crate::TransformForest`].
///
/// There are different kinds of transforms handled here:
/// * [`archetypes::Transform3D`]
///   Tree transforms that should propagate in the tree (via [`crate::TransformForest`]).
/// * [`components::PinholeProjection`] and [`components::ViewCoordinates`]
///   Pinhole projections & associated view coordinates used for visualizing cameras in 3D and embedding 2D in 3D
/// * [`archetypes::InstancePoses3D`]
///   Instance poses that should be applied to the tree transforms (via [`crate::TransformForest`]) but not propagate.
///   Also unlike tree transforms, these are not associated with transform frames but rather with entity paths.
pub struct TransformResolutionCache {
    /// The frame id registry is co-located in the resolution cache for convenience:
    /// the resolution cache is often the lowest level of transform access and
    /// thus allowing us to access debug information across the stack.
    frame_id_registry: FrameIdRegistry,

    per_timeline: HashMap<TimelineName, CachedTransformsForTimeline>,
    static_timeline: CachedTransformsForTimeline,
}

impl Default for TransformResolutionCache {
    #[inline]
    fn default() -> Self {
        Self {
            frame_id_registry: Default::default(),
            per_timeline: Default::default(),
            // `CachedTransformsForTimeline` intentionally doesn't implement Default to not accidentally create it without considering static transforms.
            static_timeline: CachedTransformsForTimeline {
                per_child_frame_transforms: Default::default(),
                per_entity_poses: Default::default(),
                non_recursive_clears: Default::default(),
                recursive_clears: Default::default(), // Unused for static timeline.
            },
        }
    }
}

impl SizeBytes for TransformResolutionCache {
    fn heap_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        let Self {
            frame_id_registry,
            per_timeline,
            static_timeline,
        } = self;

        frame_id_registry.heap_size_bytes()
            + per_timeline.heap_size_bytes()
            + static_timeline.heap_size_bytes()
    }
}

impl re_byte_size::MemUsageTreeCapture for TransformResolutionCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_tracing::profile_function!();

        let Self {
            frame_id_registry,
            per_timeline,
            static_timeline,
        } = self;

        re_byte_size::MemUsageNode::new()
            .with_child("frame_id_registry", frame_id_registry.total_size_bytes())
            .with_child("per_timeline", per_timeline.capture_mem_usage_tree())
            .with_child("static_timeline", static_timeline.total_size_bytes())
            .into_tree()
    }
}

/// A transform from a child frame to a parent frame.
#[derive(Clone, Debug, PartialEq)]
pub struct ParentFromChildTransform {
    /// The frame we're transforming into.
    pub parent: TransformFrameIdHash,

    /// The transform from the child frame to the parent frame.
    pub transform: DAffine3,
}

impl SizeBytes for ParentFromChildTransform {
    fn heap_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        let Self { parent, transform } = self;

        parent.heap_size_bytes() + transform.heap_size_bytes()
    }
}

/// Cached transforms for a single timeline.
///
/// Includes any static transforms that may apply globally.
/// Therefore, this can't be trivially constructed.
pub struct CachedTransformsForTimeline {
    /// Transforms information for each child frame to a parent frame over time.
    // Note that these are potentially a lot of mutexes, but `parking_lot`-Mutex are incredibly lightweight on all platforms, so not a memory concern.
    per_child_frame_transforms: IntMap<TransformFrameIdHash, TreeTransformsForChildFrame>,

    /// Instance pose information for each entity over time.
    ///
    /// Unlike all other transforms, poses are associated with an entity path, not a frame.
    per_entity_poses: IntMap<EntityPathHash, PoseTransformForEntity>,

    /// We need to keep track of all clears that ever happened and when.
    /// Otherwise, new incoming frames may not correctly change their transform at the time of clear.
    non_recursive_clears: IntMap<EntityPath, BTreeSet<TimeInt>>,

    /// We need to keep track of all recursive clears that ever happened and when.
    /// Otherwise, new incoming frames may not correctly change their transform at the time of clear.
    recursive_clears: IntMap<EntityPath, BTreeSet<TimeInt>>,
}

impl CachedTransformsForTimeline {
    fn new(timeline: &TimelineName, static_transforms: &Self) -> Self {
        Self {
            per_child_frame_transforms: static_transforms
                .per_child_frame_transforms
                .iter()
                .map(|(transform_frame, static_transforms)| {
                    (
                        *transform_frame,
                        TreeTransformsForChildFrame::new_for_new_empty_timeline(
                            *timeline,
                            static_transforms,
                        ),
                    )
                })
                .collect(),
            per_entity_poses: static_transforms.per_entity_poses.clone(),
            non_recursive_clears: IntMap::default(),
            recursive_clears: IntMap::default(),
        }
    }

    fn get_or_create_tree_transforms_temporal(
        &mut self,
        entity_path: &EntityPath,
        child_frame: TransformFrameIdHash,
        timeline: TimelineName,
        static_timeline: &mut Self,
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

    fn get_or_create_tree_transforms_static(
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
                                "The entity path associated with a child frame mustn't change except for static vs temporal data. The frame {} was previously logged statically at the path {existing_path:?} and was now logged on {entity_path:?}.",
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

    fn get_or_create_pose_transforms_temporal(
        &mut self,
        entity_path: &EntityPath,
        static_timeline: &mut Self,
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

    fn get_or_create_pose_transforms_static(
        &mut self,
        entity_path: &EntityPath,
    ) -> &mut PoseTransformForEntity {
        self.per_entity_poses
            .entry(entity_path.hash())
            .or_insert_with(|| PoseTransformForEntity::new_empty(entity_path.clone()))
    }

    fn add_clear(&mut self, cleared_path: &EntityPath, cleared_time: TimeInt) {
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

    fn add_recursive_clear(
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

    fn remove_clear(&mut self, cleared_path: &EntityPath, cleared_time: TimeInt) {
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

    fn remove_recursive_clear(
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

#[derive(Clone, Debug, PartialEq)]
enum CachedTransformValue<T> {
    /// Cache is invalidated, we don't know what state we're in.
    Invalidated,

    /// There's a transform at this time.
    Resident(T),

    /// The value has been cleared out at this time.
    Cleared,
}

impl<T: SizeBytes> SizeBytes for CachedTransformValue<T> {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::Resident(item) => item.heap_size_bytes(),
            Self::Invalidated | Self::Cleared => 0,
        }
    }
}

type FrameTransformTimeMap =
    BookkeepingBTreeMap<TimeInt, CachedTransformValue<ParentFromChildTransform>>;

type PinholeProjectionMap =
    BookkeepingBTreeMap<TimeInt, CachedTransformValue<ResolvedPinholeProjection>>;

#[derive(Clone, Debug, PartialEq)]
struct TransformsForChildFrameEvents {
    /// There can be only a single parent at any point in time, but it may change over time.
    /// Whenever it changes, the previous parent frame is no longer reachable.
    frame_transforms: FrameTransformTimeMap,

    pinhole_projections: PinholeProjectionMap,
}

impl TransformsForChildFrameEvents {
    fn new_empty() -> Self {
        Self {
            frame_transforms: Default::default(),
            pinhole_projections: Default::default(),
        }
    }

    /// Inserts a cleared transform for the given times.
    fn insert_clear(&mut self, time: TimeInt) {
        let Self {
            frame_transforms,
            pinhole_projections,
        } = self;

        frame_transforms.insert(time, CachedTransformValue::Cleared);
        pinhole_projections.insert(time, CachedTransformValue::Cleared);
    }

    /// Insert several cleared transforms for the given times.
    fn insert_clears(&mut self, times: &BTreeSet<TimeInt>) {
        for &time in times {
            self.insert_clear(time);
        }
    }

    /// Removes any events at a given time (if any).
    fn remove_at(&mut self, time: TimeInt) {
        let Self {
            frame_transforms,
            pinhole_projections,
        } = self;

        frame_transforms.remove(&time);
        pinhole_projections.remove(&time);
    }

    fn is_empty(&self) -> bool {
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

/// Cached transforms from a single child frame to a (potentially changing) parent frame over time.
///
/// Incorporates any static transforms that may apply to this entity.
///
/// Time points are conservative: it can happen that we generate new events (==cache slots) despite no change
/// occurring for this child frame.
/// However, we mustn't ever note down timepoints at which the given child frame is not "active" on its entity.
/// Doing so would mean that queries using `re_query` yield information about a _different_ child frame
/// which we then can't add to the cache entries of the current frame.
#[derive(Debug)]
pub struct TreeTransformsForChildFrame {
    // Is None if this is about static time.
    #[cfg(debug_assertions)]
    timeline: Option<TimelineName>,

    /// The entity path that produces temporal information for this frame.
    ///
    /// Note that it is a user-data error to change the entity path a frame relationship is defined on.
    /// I.e., given a frame relationship `A -> B` logged on entity `/my_path`, all future changes
    /// to the relation of `A ->` must be logged on the same entity `/my_path`.
    ///
    /// This greatly simplifies clearing and tracking of transforms.
    associated_entity_path_temporal: Option<EntityPath>,

    /// Like [`Self::associated_entity_path_temporal`] but for static chunks.
    associated_entity_path_static: Option<EntityPath>,

    child_frame: TransformFrameIdHash,

    events: Mutex<TransformsForChildFrameEvents>,
}

impl Clone for TreeTransformsForChildFrame {
    fn clone(&self) -> Self {
        Self {
            #[cfg(debug_assertions)]
            timeline: self.timeline,
            associated_entity_path_temporal: self.associated_entity_path_temporal.clone(),
            associated_entity_path_static: self.associated_entity_path_static.clone(),
            child_frame: self.child_frame,
            events: Mutex::new(self.events.lock().clone()),
        }
    }
}

impl PartialEq for TreeTransformsForChildFrame {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            #[cfg(debug_assertions)]
                timeline: _,
            associated_entity_path_temporal,
            associated_entity_path_static,
            child_frame,
            events,
        } = self;

        associated_entity_path_temporal == &other.associated_entity_path_temporal
            && associated_entity_path_static == &other.associated_entity_path_static
            && child_frame == &other.child_frame
            && *events.lock() == *other.events.lock()
    }
}

impl SizeBytes for TreeTransformsForChildFrame {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            associated_entity_path_temporal,
            associated_entity_path_static,
            child_frame,
            events,

            #[cfg(debug_assertions)]
                timeline: _,
        } = self;

        associated_entity_path_temporal.heap_size_bytes()
            + associated_entity_path_static.heap_size_bytes()
            + child_frame.heap_size_bytes()
            + events.lock().heap_size_bytes()
    }
}

fn add_invalidated_entry_if_not_already_cleared<T: PartialEq + SizeBytes>(
    transforms: &mut BookkeepingBTreeMap<TimeInt, CachedTransformValue<T>>,
    time: TimeInt,
) {
    transforms.mutate_entry(time, CachedTransformValue::Invalidated, |value| {
        if *value != CachedTransformValue::Cleared {
            *value = CachedTransformValue::Invalidated;
        }
    });
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPinholeProjection {
    /// The parent frame of the pinhole projection.
    pub parent: TransformFrameIdHash,

    pub image_from_camera: components::PinholeProjection,

    pub resolution: Option<components::Resolution>,

    /// View coordinates at this pinhole camera.
    ///
    /// This is needed to orient 2D in 3D and 3D in 2D the right way around
    /// (answering questions like which axis is distance to viewer increasing).
    /// If no view coordinates were logged, this is set to [`archetypes::Pinhole::DEFAULT_CAMERA_XYZ`].
    pub view_coordinates: components::ViewCoordinates,
}

impl SizeBytes for ResolvedPinholeProjection {
    fn heap_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        let Self {
            parent,
            image_from_camera,
            resolution,
            view_coordinates,
        } = self;

        parent.heap_size_bytes()
            + image_from_camera.heap_size_bytes()
            + resolution.heap_size_bytes()
            + view_coordinates.heap_size_bytes()
    }
}

impl TreeTransformsForChildFrame {
    fn new_temporal(
        associated_entity_path: EntityPath,
        child_frame: TransformFrameIdHash,
        _timeline: TimelineName,
        static_timeline: &mut CachedTransformsForTimeline,
        non_recursive_clears: &IntMap<EntityPath, BTreeSet<TimeInt>>,
        recursive_clears: &IntMap<EntityPath, BTreeSet<TimeInt>>,
    ) -> Self {
        let mut events = TransformsForChildFrameEvents::new_empty();

        // Take over static events.
        let associated_entity_path_static = if let Some(static_transforms) = static_timeline
            .per_child_frame_transforms
            .get_mut(&child_frame)
        {
            events = static_transforms.events.get_mut().clone();

            debug_assert!(static_transforms.associated_entity_path_static.is_some());
            static_transforms.associated_entity_path_static.clone()
        } else {
            None
        };

        // Take over clear events.
        if let Some(cleared_times) = non_recursive_clears.get(&associated_entity_path) {
            events.insert_clears(cleared_times);
        }
        for (recursively_cleared_path, times) in recursive_clears {
            if associated_entity_path.starts_with(recursively_cleared_path) {
                events.insert_clears(times);
            }
        }

        Self {
            #[cfg(debug_assertions)]
            timeline: Some(_timeline),
            associated_entity_path_temporal: Some(associated_entity_path),
            associated_entity_path_static,
            child_frame,
            events: Mutex::new(events),
        }
    }

    fn new_for_new_empty_timeline(_timeline: TimelineName, static_timeline_entry: &Self) -> Self {
        Self {
            #[cfg(debug_assertions)]
            timeline: Some(_timeline),
            ..static_timeline_entry.clone()
        }
    }

    fn new_static(associated_entity_path: EntityPath, child_frame: TransformFrameIdHash) -> Self {
        Self {
            #[cfg(debug_assertions)]
            timeline: None,
            associated_entity_path_temporal: None,
            associated_entity_path_static: Some(associated_entity_path),
            child_frame,
            events: Mutex::new(TransformsForChildFrameEvents::new_empty()),
        }
    }

    /// The entity path that produces information for this frame.
    pub fn associated_entity_path(&self, time: TimeInt) -> &EntityPath {
        if time == TimeInt::STATIC {
            // Use static path if it exists.
            self.associated_entity_path_static
                .as_ref()
                .or(self.associated_entity_path_temporal.as_ref())
                .expect("Either temporal or static associated entity path must be set")
        } else {
            // Use temporal path if it exists.
            self.associated_entity_path_temporal
                .as_ref()
                .or(self.associated_entity_path_static.as_ref())
                .expect("Either temporal or static associated entity path must be set")
        }
    }

    /// Inserts an invalidation point for transforms.
    fn invalidate_transform_at(&mut self, time: TimeInt) {
        let events = self.events.get_mut();
        add_invalidated_entry_if_not_already_cleared(&mut events.frame_transforms, time);
    }

    /// Inserts an invalidation point for pinhole projections.
    fn invalidate_pinhole_projection_at(&mut self, time: TimeInt) {
        let events = self.events.get_mut();
        add_invalidated_entry_if_not_already_cleared(&mut events.pinhole_projections, time);
    }

    #[inline]
    pub fn latest_at_transform(
        &self,
        entity_db: &EntityDb,
        query: &LatestAtQuery,
    ) -> Option<ParentFromChildTransform> {
        #[cfg(debug_assertions)] // `self.timeline` is only present with `debug_assertions` enabled.
        debug_assert!(Some(query.timeline()) == self.timeline || self.timeline.is_none());

        let mut events = self.events.lock();

        events
            .frame_transforms
            .mutate_latest_at(
                &query.at(),
                |time_of_last_update_to_this_frame, frame_transform| {
                    // Separate check to work around borrow checker issues.
                    if frame_transform == &CachedTransformValue::Invalidated {
                        let transform = query_and_resolve_tree_transform_at_entity(
                            self.associated_entity_path(*time_of_last_update_to_this_frame),
                            self.child_frame,
                            entity_db,
                            // Do NOT use the original query time since that may give us information about a different child frame!
                            &LatestAtQuery::new(
                                query.timeline(),
                                *time_of_last_update_to_this_frame,
                            ),
                        );

                        // First, we update the cache value.
                        *frame_transform = match &transform {
                            Ok(transform) => CachedTransformValue::Resident(transform.clone()),

                            Err(crate::transform_queries::TransformError::MissingTransform {
                                ..
                            }) => {
                                // This can happen if we conservatively added a timepoint before any transform event happened.
                                CachedTransformValue::Cleared
                            }

                            Err(err) => {
                                // Only warn since we can still work just fine if a transform didn't work.
                                re_log::warn_once!("Failed to query transformations: {err}");
                                CachedTransformValue::Cleared
                            }
                        };
                    }

                    match frame_transform {
                        CachedTransformValue::Resident(transform) => Some(transform.clone()),
                        CachedTransformValue::Cleared => None,
                        CachedTransformValue::Invalidated => {
                            unreachable!("Just made transform cache-resident")
                        }
                    }
                },
            )
            .flatten()
    }

    #[inline]
    pub fn latest_at_pinhole(
        &self,
        entity_db: &EntityDb,
        query: &LatestAtQuery,
    ) -> Option<ResolvedPinholeProjection> {
        #[cfg(debug_assertions)] // `self.timeline` is only present with `debug_assertions` enabled.
        debug_assert!(Some(query.timeline()) == self.timeline || self.timeline.is_none());

        let mut events = self.events.lock();

        events
            .pinhole_projections
            .mutate_latest_at(
                &query.at(),
                |time_of_last_update_to_this_frame, pinhole_projection| {
                    // Separate check to work around borrow checker issues.
                    if pinhole_projection == &CachedTransformValue::Invalidated {
                        let transform = query_and_resolve_pinhole_projection_at_entity(
                            self.associated_entity_path(*time_of_last_update_to_this_frame),
                            self.child_frame,
                            entity_db,
                            // Do NOT use the original query time since that may give us information about a different child frame!
                            &LatestAtQuery::new(
                                query.timeline(),
                                *time_of_last_update_to_this_frame,
                            ),
                        );

                        *pinhole_projection = match &transform {
                            Ok(transform) => CachedTransformValue::Resident(transform.clone()),

                            Err(crate::transform_queries::TransformError::MissingTransform {
                                ..
                            }) => {
                                // This can happen if we conservatively added a timepoint before any transform event happened.
                                CachedTransformValue::Cleared
                            }

                            Err(err) => {
                                // Only warn since we can still work just fine if a transform didn't work.
                                re_log::warn_once!("Failed to query transformations: {err}");
                                CachedTransformValue::Cleared
                            }
                        };
                    }

                    match pinhole_projection {
                        CachedTransformValue::Resident(transform) => Some(transform.clone()),
                        CachedTransformValue::Cleared => None,
                        CachedTransformValue::Invalidated => {
                            unreachable!("Just made transform cache-resident")
                        }
                    }
                },
            )
            .flatten()
    }
}

/// All instance poses for a given entity over time.
///
/// Similar to [`TreeTransformsForChildFrame`], but for poses associated with an entity path.
#[derive(Debug)]
pub struct PoseTransformForEntity {
    entity_path: EntityPath,
    poses_per_time: Mutex<BookkeepingBTreeMap<TimeInt, CachedTransformValue<Vec<DAffine3>>>>,
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
    fn new(
        entity_path: EntityPath,
        static_timeline: &mut CachedTransformsForTimeline,
        non_recursive_clears: &IntMap<EntityPath, BTreeSet<TimeInt>>,
        recursive_clears: &IntMap<EntityPath, BTreeSet<TimeInt>>,
    ) -> Self {
        let mut poses = Self::new_empty(entity_path);

        // Take over static events.
        if let Some(static_transforms) = static_timeline
            .per_entity_poses
            .get_mut(&poses.entity_path.hash())
        {
            *poses.poses_per_time.get_mut() = static_transforms.poses_per_time.get_mut().clone();
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

    fn new_empty(entity_path: EntityPath) -> Self {
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
    fn insert_clear(&mut self, time: TimeInt) {
        self.poses_per_time
            .get_mut()
            .insert(time, CachedTransformValue::Cleared);
    }

    /// Insert several cleared transforms for the given times.
    fn insert_clears(&mut self, time: &BTreeSet<TimeInt>) {
        self.poses_per_time
            .get_mut()
            .extend(time.iter().map(|t| (*t, CachedTransformValue::Cleared)));
    }

    /// Inserts an invalidation point for poses.
    fn invalidate_at(&mut self, time: TimeInt) {
        add_invalidated_entry_if_not_already_cleared(self.poses_per_time.get_mut(), time);
    }
}

impl TransformResolutionCache {
    /// Returns the registry of all known frame ids.
    #[inline]
    pub fn frame_id_registry(&self) -> &FrameIdRegistry {
        &self.frame_id_registry
    }

    /// Accesses the transform component tracking data for a given timeline.
    #[inline]
    pub fn transforms_for_timeline(&self, timeline: TimelineName) -> &CachedTransformsForTimeline {
        self.per_timeline
            .get(&timeline)
            .unwrap_or(&self.static_timeline)
    }

    /// Makes sure the internal transform index is up to date and outdated cache entries are discarded.
    ///
    /// This needs to be called once per frame prior to any transform propagation.
    /// (which is done by [`crate::TransformForest`])
    ///
    /// This will internally…
    /// * keep track of which child frames are influenced by which entity
    /// * create empty entries for where transforms may change over time (may happen conservatively - creating more entries than needed)
    ///     * this may invalidate previous entries at the same position
    /// * remove cached entries if chunks were GC'ed
    ///
    /// See also [`Self::add_chunks`].
    pub fn process_store_events<'a>(
        &mut self,
        events: impl Iterator<Item = &'a re_chunk_store::ChunkStoreEvent>,
    ) {
        re_tracing::profile_function!();

        // TODO(andreas): We eagerly index for all timelines even if they're never used.
        // Instead, we should do so lazily when results for a timeline are queried.

        for event in events {
            // This doesn't maintain a collection of chunks that needs to be kept in sync 1:1 with
            // the store, rather it just keeps track of what entities have what properties, and for
            // that a delta chunk is all we need.
            let Some(delta_chunk) = event.delta_chunk() else {
                continue; // virtual event, we don't care
            };

            // Since entity paths lead to implicit frames, we have to prime our lookup table
            // with them even if this chunk doesn't have transform data.
            self.frame_id_registry
                .register_all_frames_in_chunk(delta_chunk);

            let aspects = TransformAspect::transform_aspects_of(delta_chunk);
            if aspects.is_empty() {
                continue;
            }

            if event.is_deletion() {
                self.remove_chunk(delta_chunk, aspects);
            } else if delta_chunk.is_static() {
                self.add_static_chunk(delta_chunk, aspects);
            } else {
                self.add_temporal_chunk(delta_chunk, aspects);
            }
        }
    }

    /// Adds chunks to the transform cache.
    ///
    /// This will internally…
    /// * keep track of which child frames are influenced by which entity
    /// * create empty entries for where transforms may change over time (may happen conservatively - creating more entries than needed)
    ///     * this may invalidate previous entries at the same position
    ///
    /// See also [`Self::process_store_events`].
    pub fn add_chunks<'a>(&mut self, chunks: impl Iterator<Item = &'a std::sync::Arc<Chunk>>) {
        re_tracing::profile_function!();

        // TODO(andreas): We eagerly index for all timelines even if they're never used.
        // Instead, we should do so lazily when results for a timeline are queried.

        for chunk in chunks {
            // Since entity paths lead to implicit frames, we have to prime our lookup table with them even if this chunk doesn't have transform data.
            self.frame_id_registry.register_all_frames_in_chunk(chunk);

            let aspects = TransformAspect::transform_aspects_of(chunk);
            if aspects.is_empty() {
                continue;
            }

            if chunk.is_static() {
                self.add_static_chunk(chunk, aspects);
            } else {
                self.add_temporal_chunk(chunk, aspects);
            }
        }
    }

    fn add_temporal_chunk(&mut self, chunk: &Chunk, aspects: TransformAspect) {
        re_tracing::profile_function!(format!(
            "{} rows, {}",
            chunk.num_rows(),
            chunk.entity_path()
        ));

        debug_assert!(!chunk.is_static());

        let entity_path = chunk.entity_path();

        let transform_child_frame_component =
            archetypes::Transform3D::descriptor_child_frame().component;
        let pinhole_child_frame_component = archetypes::Pinhole::descriptor_child_frame().component;

        let static_timeline = &mut self.static_timeline;

        for timeline in chunk.timelines().keys() {
            let per_timeline = self
                .per_timeline
                .entry(*timeline)
                .or_insert_with(|| CachedTransformsForTimeline::new(timeline, static_timeline));

            if aspects.contains(TransformAspect::Frame) {
                re_tracing::profile_scope!("TransformAspect::Frame");
                for (time, frame) in
                    iter_child_frames_in_chunk(chunk, *timeline, transform_child_frame_component)
                {
                    per_timeline
                        .get_or_create_tree_transforms_temporal(
                            entity_path,
                            frame,
                            *timeline,
                            static_timeline,
                            &self.frame_id_registry,
                        )
                        .invalidate_transform_at(time);
                }
            }
            if aspects.contains(TransformAspect::Pose) {
                re_tracing::profile_scope!("TransformAspect::Pose");
                let poses = per_timeline
                    .get_or_create_pose_transforms_temporal(entity_path, static_timeline);
                for (time, _) in chunk.iter_indices(timeline) {
                    poses.invalidate_at(time);
                }
            }
            if aspects.contains(TransformAspect::PinholeOrViewCoordinates) {
                re_tracing::profile_scope!("TransformAspect::PinholeOrViewCoordinates");
                for (time, frame) in
                    iter_child_frames_in_chunk(chunk, *timeline, pinhole_child_frame_component)
                {
                    per_timeline
                        .get_or_create_tree_transforms_temporal(
                            entity_path,
                            frame,
                            *timeline,
                            static_timeline,
                            &self.frame_id_registry,
                        )
                        .invalidate_pinhole_projection_at(time);
                }
            }

            // Keep track of clears.
            if aspects.contains(TransformAspect::Clear) {
                re_tracing::profile_scope!("TransformAspect::Clear");
                let component = archetypes::Clear::descriptor_is_recursive().component;

                for ((time, _row_id), is_recursive_slice) in chunk
                    .iter_component_indices(*timeline, component)
                    .zip(chunk.iter_slices::<bool>(component))
                {
                    if let Some(is_recursive) = is_recursive_slice.values().first()
                        && *is_recursive != 0
                    {
                        per_timeline.add_recursive_clear(entity_path, time);
                    } else {
                        per_timeline.add_clear(entity_path, time);
                    }
                }
            }
        }
    }

    fn add_static_chunk(&mut self, chunk: &Chunk, aspects: TransformAspect) {
        re_tracing::profile_function!();

        debug_assert!(chunk.is_static());

        let entity_path = chunk.entity_path();
        let place_holder_timeline = TimelineName::new("ignored for static chunk");

        let transform_child_frame_component =
            archetypes::Transform3D::descriptor_child_frame().component;
        let pinhole_child_frame_component = archetypes::Pinhole::descriptor_child_frame().component;

        let static_timeline = &mut self.static_timeline;

        // Add a static transform invalidation to affected child frames on ALL timelines.

        if aspects.contains(TransformAspect::Frame) {
            for (time, frame) in iter_child_frames_in_chunk(
                chunk,
                place_holder_timeline,
                transform_child_frame_component,
            ) {
                debug_assert_eq!(time, TimeInt::STATIC);

                let frame_transforms = static_timeline.get_or_create_tree_transforms_static(
                    entity_path,
                    frame,
                    &self.frame_id_registry,
                );
                frame_transforms.invalidate_transform_at(TimeInt::STATIC);

                #[cfg_attr(not(debug_assertions), expect(clippy::for_kv_map))]
                for (_timeline, per_timeline) in &mut self.per_timeline {
                    // Don't call `get_or_create_tree_transforms_temporal` here since we may not yet know a temporal entity that this is associated with.
                    // Also, this may be the first time we associate with a static entity instead which `get_or_create_tree_transforms_static` takes care of.
                    let transforms = per_timeline.get_or_create_tree_transforms_static(
                        entity_path,
                        frame,
                        &self.frame_id_registry,
                    );
                    transforms.invalidate_transform_at(TimeInt::STATIC);

                    // Entry might have been newly created. Have to ensure that its associated with the right timeline.
                    #[cfg(debug_assertions)]
                    {
                        transforms.timeline = Some(*_timeline);
                    }
                }
            }
        }
        if aspects.contains(TransformAspect::Pose) {
            let frame_transforms =
                static_timeline.get_or_create_pose_transforms_static(entity_path);
            frame_transforms.invalidate_at(TimeInt::STATIC);

            for per_timeline in self.per_timeline.values_mut() {
                per_timeline
                    .get_or_create_pose_transforms_temporal(entity_path, static_timeline)
                    .invalidate_at(TimeInt::STATIC);
            }
        }
        if aspects.contains(TransformAspect::PinholeOrViewCoordinates) {
            for (time, frame) in iter_child_frames_in_chunk(
                chunk,
                place_holder_timeline,
                pinhole_child_frame_component,
            ) {
                debug_assert_eq!(time, TimeInt::STATIC);

                let frame_transforms = static_timeline.get_or_create_tree_transforms_static(
                    entity_path,
                    frame,
                    &self.frame_id_registry,
                );
                frame_transforms.invalidate_pinhole_projection_at(TimeInt::STATIC);

                #[cfg_attr(not(debug_assertions), expect(clippy::for_kv_map))]
                for (_timeline, per_timeline) in &mut self.per_timeline {
                    // Don't call `get_or_create_tree_transforms_temporal` here since we may not yet know a temporal entity that this is associated with.
                    // Also, this may be the first time we associate with a static entity instead which `get_or_create_tree_transforms_static` takes care of.
                    let transforms = per_timeline.get_or_create_tree_transforms_static(
                        entity_path,
                        frame,
                        &self.frame_id_registry,
                    );
                    transforms.invalidate_pinhole_projection_at(TimeInt::STATIC);

                    // Entry might have been newly created. Have to ensure that its associated with the right timeline.
                    #[cfg(debug_assertions)]
                    {
                        transforms.timeline = Some(*_timeline);
                    }
                }
            }
        }

        // Don't care about clears here, they don't have any effect for keeping track of changes when logged static.
    }

    fn remove_chunk(&mut self, chunk: &Chunk, aspects: TransformAspect) {
        re_tracing::profile_function!();

        let entity_path = chunk.entity_path();

        let transform_child_frame_component =
            archetypes::Transform3D::descriptor_child_frame().component;
        let pinhole_child_frame_component = archetypes::Pinhole::descriptor_child_frame().component;

        // TODO(andreas): handle removal of static chunks?
        for timeline in chunk.timelines().keys() {
            let Some(per_timeline) = self.per_timeline.get_mut(timeline) else {
                continue;
            };

            // Remove any affected recursive clears.
            if aspects.contains(TransformAspect::Clear) {
                re_tracing::profile_scope!("check for recursive clears");

                let component = archetypes::Clear::descriptor_is_recursive().component;

                for ((time, _row_id), is_recursive_slice) in chunk
                    .iter_component_indices(*timeline, component)
                    .zip(chunk.iter_slices::<bool>(component))
                {
                    if let Some(is_recursive) = is_recursive_slice.values().first()
                        && *is_recursive != 0
                    {
                        per_timeline.remove_recursive_clear(entity_path, time);
                    } else {
                        per_timeline.remove_clear(entity_path, time);
                    }
                }
            }

            // Remove existing data.
            if aspects.contains(TransformAspect::Frame) {
                for (time, frame) in
                    iter_child_frames_in_chunk(chunk, *timeline, transform_child_frame_component)
                {
                    if let Some(transforms) =
                        per_timeline.per_child_frame_transforms.get_mut(&frame)
                    {
                        let events = transforms.events.get_mut();
                        events.frame_transforms.remove(&time);
                    }
                }
            }
            if aspects.contains(TransformAspect::Pose)
                && let Some(poses) = per_timeline.per_entity_poses.get_mut(&entity_path.hash())
            {
                for (time, _) in chunk.iter_indices(timeline) {
                    poses.poses_per_time.get_mut().remove(&time);
                }
            }
            if aspects.contains(TransformAspect::PinholeOrViewCoordinates) {
                for (time, frame) in
                    iter_child_frames_in_chunk(chunk, *timeline, pinhole_child_frame_component)
                {
                    if let Some(transforms) =
                        per_timeline.per_child_frame_transforms.get_mut(&frame)
                    {
                        let events = transforms.events.get_mut();
                        events.pinhole_projections.remove(&time);
                    }
                }
            }

            // Remove any empty transform collection.
            per_timeline
                .per_child_frame_transforms
                .retain(|_frame, transforms| !transforms.events.get_mut().is_empty());

            // Remove the entire timeline if it's empty.
            if per_timeline.per_child_frame_transforms.is_empty() {
                self.per_timeline.remove(timeline);
            }
        }
    }
}

/// Iterates over all frames of a given component type that are in a chunk.
///
/// If the chunk is static, `timeline` will be ignored.
///
/// Yields an entry for every row. Note that there may be many entries per time though.
/// (Currently, there can be only a single frame id per row)
fn iter_child_frames_in_chunk(
    chunk: &Chunk,
    timeline: TimelineName,
    frame_component: ComponentIdentifier,
) -> impl Iterator<Item = (TimeInt, TransformFrameIdHash)> {
    let implicit_frame = TransformFrameIdHash::from_entity_path(chunk.entity_path());

    // This is similar to `iter_slices` but it also yields elements for rows where the component is null.
    let frame_ids_per_row =
    chunk.components().get_array(frame_component).map_or_else(
        || Either::Left(std::iter::repeat(implicit_frame)),
        |list_array| {
            let values_raw = list_array.values();
            let Some(values) =
                values_raw.downcast_array_ref::<arrow::array::StringArray>()
            else {
                re_log::error_once!("Expected at {frame_component:?} @ {:?} to be a string array, but its type is instead {:?}",
                                         chunk.entity_path(), values_raw.data_type());
                return Either::Left(std::iter::repeat(implicit_frame));
            };

            let offsets = list_array.offsets().iter().map(|idx| *idx as usize);
            let lengths = list_array.offsets().lengths();

            Either::Right(izip!(offsets, lengths).map(move |(offset, length)| {
                // No need to check for nulls since we treat nulls and empty arrays both as the implicit frame.
                if length == 0 {
                    implicit_frame
                } else {
                    // There can only be a single frame id per row today, so only look at the first element.
                    TransformFrameIdHash::from_str(values.value(offset))
                }
            }))
        }
    );

    izip!(
        chunk.iter_indices(&timeline).map(|(t, _)| t),
        frame_ids_per_row
    )
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, OnceLock};

    use re_chunk_store::{
        Chunk, ChunkStore, ChunkStoreEvent, ChunkStoreSubscriberHandle, GarbageCollectionOptions,
        PerStoreChunkSubscriber,
    };
    use re_log_types::{
        StoreId, StoreInfo, TimePoint, Timeline,
        example_components::{MyPoint, MyPoints},
    };
    use re_sdk_types::{
        ChunkId,
        archetypes::{self, InstancePoses3D, Pinhole, Transform3D},
        components::PinholeProjection,
    };

    use super::*;
    use crate::convert;

    #[derive(Debug, Clone, Copy)]
    enum StaticTestFlavor {
        /// First log a static chunk and then a regular chunk.
        StaticThenRegular { update_inbetween: bool },

        /// First log a regular chunk and then a static chunk.
        RegularThenStatic { update_inbetween: bool },

        /// Test case where we first log a static chunk and regular chunk and then later swap out the static chunk.
        /// This tests that we're able to invalidate the cache on static changes after the fact.
        PriorStaticThenRegularThenStatic { update_inbetween: bool },
    }

    const ALL_STATIC_TEST_FLAVOURS: [StaticTestFlavor; 6] = [
        StaticTestFlavor::StaticThenRegular {
            update_inbetween: true,
        },
        StaticTestFlavor::RegularThenStatic {
            update_inbetween: true,
        },
        StaticTestFlavor::PriorStaticThenRegularThenStatic {
            update_inbetween: true,
        },
        StaticTestFlavor::StaticThenRegular {
            update_inbetween: false,
        },
        StaticTestFlavor::RegularThenStatic {
            update_inbetween: false,
        },
        StaticTestFlavor::PriorStaticThenRegularThenStatic {
            update_inbetween: false,
        },
    ];

    #[derive(Default)]
    pub struct TestStoreSubscriber {
        unprocessed_events: Vec<ChunkStoreEvent>,
    }

    impl TestStoreSubscriber {
        /// Accesses the global store subscriber.
        ///
        /// Lazily registers the subscriber if it hasn't been registered yet.
        pub fn subscription_handle() -> ChunkStoreSubscriberHandle {
            static SUBSCRIPTION: OnceLock<ChunkStoreSubscriberHandle> = OnceLock::new();
            *SUBSCRIPTION.get_or_init(ChunkStore::register_per_store_subscriber::<Self>)
        }

        /// Retrieves all transform events that have not been processed yet since the last call to this function.
        pub fn take_transform_events(store_id: &StoreId) -> Vec<ChunkStoreEvent> {
            ChunkStore::with_per_store_subscriber_mut(
                Self::subscription_handle(),
                store_id,
                |subscriber: &mut Self| std::mem::take(&mut subscriber.unprocessed_events),
            )
            .unwrap_or_default()
        }
    }

    impl PerStoreChunkSubscriber for TestStoreSubscriber {
        fn name() -> String {
            "TestStoreSubscriber".to_owned()
        }

        fn on_events<'a>(&mut self, events: impl Iterator<Item = &'a ChunkStoreEvent>) {
            self.unprocessed_events.extend(events.cloned());
        }
    }

    fn apply_store_subscriber_events(cache: &mut TransformResolutionCache, entity_db: &EntityDb) {
        let events = TestStoreSubscriber::take_transform_events(entity_db.store_id());
        cache.process_store_events(events.iter());
    }

    fn static_test_setup_store(
        cache: &mut TransformResolutionCache,
        prior_static_chunk: Chunk,
        final_static_chunk: Chunk,
        regular_chunk: Chunk,
        flavor: StaticTestFlavor,
    ) -> Result<EntityDb, Box<dyn std::error::Error>> {
        // Print the flavor to its shown on test failure.
        println!("{flavor:?}");

        let mut entity_db = new_entity_db_with_subscriber_registered();

        match flavor {
            StaticTestFlavor::StaticThenRegular { update_inbetween } => {
                entity_db.add_chunk(&Arc::new(final_static_chunk))?;
                if update_inbetween {
                    apply_store_subscriber_events(cache, &entity_db);
                }
                entity_db.add_chunk(&Arc::new(regular_chunk))?;
            }

            StaticTestFlavor::RegularThenStatic { update_inbetween } => {
                entity_db.add_chunk(&Arc::new(regular_chunk))?;
                if update_inbetween {
                    apply_store_subscriber_events(cache, &entity_db);
                }
                entity_db.add_chunk(&Arc::new(final_static_chunk))?;
            }

            StaticTestFlavor::PriorStaticThenRegularThenStatic { update_inbetween } => {
                entity_db.add_chunk(&Arc::new(prior_static_chunk))?;
                entity_db.add_chunk(&Arc::new(regular_chunk))?;
                if update_inbetween {
                    apply_store_subscriber_events(cache, &entity_db);
                }
                entity_db.add_chunk(&Arc::new(final_static_chunk))?;
            }
        }

        Ok(entity_db)
    }

    fn new_entity_db_with_subscriber_registered() -> EntityDb {
        let entity_db = EntityDb::new(StoreInfo::testing().store_id);
        let _ = TestStoreSubscriber::subscription_handle();
        entity_db
    }

    #[test]
    fn test_transforms_per_timeline_access() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk0 = Chunk::builder(EntityPath::from("with_transform"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &Transform3D::from_translation([1.0, 2.0, 3.0]),
            )
            .build()?;
        let chunk1 = Chunk::builder(EntityPath::from("without_transform"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                // Anything that doesn't have components the transform cache is interested in.
                &archetypes::Points3D::new([[1.0, 2.0, 3.0]]),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk0))?;
        entity_db.add_chunk(&Arc::new(chunk1))?;

        apply_store_subscriber_events(&mut cache, &entity_db);
        let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
        assert!(
            transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "without_transform"
                )))
                .is_none()
        );
        assert!(
            transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "rando"
                )))
                .is_none()
        );
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "with_transform",
            )))
            .unwrap();
        #[cfg(debug_assertions)]
        assert_eq!(transforms.timeline, Some(*timeline.name()));
        assert_eq!(transforms.events.lock().frame_transforms.len(), 1);
        assert_eq!(transforms.events.lock().pinhole_projections.len(), 0);
        Ok(())
    }

    #[test]
    fn test_static_tree_transforms() -> Result<(), Box<dyn std::error::Error>> {
        for flavor in &ALL_STATIC_TEST_FLAVOURS {
            // Log a few tree transforms at different times.
            let timeline = Timeline::new_sequence("t");
            let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    TimePoint::default(),
                    // Make sure only translation is logged (no null arrays for everything else).
                    &Transform3D::from_translation([123.0, 234.0, 345.0]),
                )
                .build()?;
            let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    TimePoint::default(),
                    // Make sure only translation is logged (no null arrays for everything else).
                    &Transform3D::from_translation([1.0, 2.0, 3.0]),
                )
                .build()?;
            let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    [(timeline, 1)],
                    &Transform3D::from_scale([123.0, 234.0, 345.0]),
                )
                .build()?;

            let mut cache = TransformResolutionCache::default();
            let entity_db = static_test_setup_store(
                &mut cache,
                prior_static_chunk,
                final_static_chunk,
                regular_chunk,
                *flavor,
            )?;

            // Check that the transform cache has the expected transforms.
            apply_store_subscriber_events(&mut cache, &entity_db);

            let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
            let transforms = transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "my_entity",
                )))
                .unwrap();

            assert_eq!(
                transforms.latest_at_transform(
                    &entity_db,
                    &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                ),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                })
            );
            assert_eq!(
                transforms.latest_at_transform(
                    &entity_db,
                    &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                ),
                transforms
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(*timeline.name(), 0)),
            );
            assert_eq!(
                transforms
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(*timeline.name(), 1)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    // Due to atomic-latest-at, the translation is no longer visible despite being on the static chunk.
                    transform: DAffine3::from_scale(glam::dvec3(123.0, 234.0, 345.0)),
                })
            );

            // Timelines that the cache has never seen should still have the static transform.
            let transforms_per_timeline = cache.transforms_for_timeline(TimelineName::new("other"));
            let transforms = transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "my_entity",
                )))
                .unwrap();
            assert_eq!(
                transforms.latest_at_transform(
                    &entity_db,
                    &LatestAtQuery::new(TimelineName::new("other"), 123)
                ),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                })
            );
        }

        Ok(())
    }

    #[test]
    fn test_static_pose_transforms() -> Result<(), Box<dyn std::error::Error>> {
        for flavor in &ALL_STATIC_TEST_FLAVOURS {
            // Log a few tree transforms at different times.
            let timeline = Timeline::new_sequence("t");
            let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    TimePoint::default(),
                    &InstancePoses3D::new().with_translations([[321.0, 234.0, 345.0]]),
                )
                .build()?;
            let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    TimePoint::default(),
                    &InstancePoses3D::new().with_translations([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]),
                )
                .build()?;
            let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    [(timeline, 1)],
                    // Add a splatted scale.
                    &InstancePoses3D::new().with_scales([[10.0, 20.0, 30.0]]),
                )
                .build()?;

            let mut cache = TransformResolutionCache::default();
            let entity_db = static_test_setup_store(
                &mut cache,
                prior_static_chunk,
                final_static_chunk,
                regular_chunk,
                *flavor,
            )?;

            // Check that the transform cache has the expected transforms.
            apply_store_subscriber_events(&mut cache, &entity_db);

            let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
            let transforms = transforms_per_timeline
                .pose_transforms(EntityPath::from("my_entity").hash())
                .unwrap();

            assert_eq!(
                transforms.latest_at_instance_poses(
                    &entity_db,
                    &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                ),
                vec![
                    DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                    DAffine3::from_translation(glam::dvec3(4.0, 5.0, 6.0)),
                ],
            );
            assert_eq!(
                transforms.latest_at_instance_poses(
                    &entity_db,
                    &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                ),
                transforms
                    .latest_at_instance_poses(&entity_db, &LatestAtQuery::new(*timeline.name(), 0)),
            );
            assert_eq!(
                transforms
                    .latest_at_instance_poses(&entity_db, &LatestAtQuery::new(*timeline.name(), 1)),
                // Due to atomic-latest-at, the translation is no longer visible despite being on the static chunk.
                vec![DAffine3::from_scale(glam::dvec3(10.0, 20.0, 30.0)),]
            );

            // Timelines that the cache has never seen should still have the static poses.
            let transforms_per_timeline = cache.transforms_for_timeline(TimelineName::new("other"));
            let transforms = transforms_per_timeline
                .pose_transforms(EntityPath::from("my_entity").hash())
                .unwrap();
            assert_eq!(
                transforms.latest_at_instance_poses(
                    &entity_db,
                    &LatestAtQuery::new(TimelineName::new("other"), 123)
                ),
                vec![
                    DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                    DAffine3::from_translation(glam::dvec3(4.0, 5.0, 6.0)),
                ]
            );
        }

        Ok(())
    }

    #[test]
    fn test_static_pinhole_projection() -> Result<(), Box<dyn std::error::Error>> {
        for flavor in &ALL_STATIC_TEST_FLAVOURS {
            let image_from_camera_prior = PinholeProjection::from_focal_length_and_principal_point(
                [123.0, 123.0],
                [123.0, 123.0],
            );
            let image_from_camera_final =
                PinholeProjection::from_focal_length_and_principal_point([1.0, 2.0], [1.0, 2.0]);

            // Static pinhole, non-static view coordinates.
            let timeline = Timeline::new_sequence("t");
            let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    TimePoint::default(),
                    &Pinhole::new(image_from_camera_prior).with_resolution([1.0, 1.0]),
                )
                .build()?;
            let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(
                    TimePoint::default(),
                    &Pinhole::new(image_from_camera_final).with_resolution([2.0, 2.0]),
                )
                .build()?;
            let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row([(timeline, 1)], &archetypes::ViewCoordinates::BLU())
                .build()?;

            let mut cache = TransformResolutionCache::default();
            let entity_db = static_test_setup_store(
                &mut cache,
                prior_static_chunk,
                final_static_chunk,
                regular_chunk,
                *flavor,
            )?;

            // Check that the transform cache has the expected transforms.
            apply_store_subscriber_events(&mut cache, &entity_db);

            let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
            let transforms = transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "my_entity",
                )))
                .unwrap();

            assert_eq!(
                transforms.latest_at_pinhole(
                    &entity_db,
                    &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                ),
                Some(ResolvedPinholeProjection {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    image_from_camera: image_from_camera_final,
                    resolution: Some([2.0, 2.0].into()),
                    view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
                })
            );
            assert_eq!(
                transforms.latest_at_pinhole(
                    &entity_db,
                    &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                ),
                transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(*timeline.name(), 0))
            );
            assert_eq!(
                transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(*timeline.name(), 1)),
                Some(ResolvedPinholeProjection {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    image_from_camera: image_from_camera_final,
                    resolution: Some([2.0, 2.0].into()),
                    view_coordinates: components::ViewCoordinates::BLU,
                })
            );

            // Timelines that the cache has never seen should still have the static pinhole.
            let transforms_per_timeline = cache.transforms_for_timeline(TimelineName::new("other"));
            let transforms = transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "my_entity",
                )))
                .unwrap();
            assert_eq!(
                transforms.latest_at_pinhole(
                    &entity_db,
                    &LatestAtQuery::new(TimelineName::new("other"), 123)
                ),
                Some(ResolvedPinholeProjection {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    image_from_camera: image_from_camera_final,
                    resolution: Some([2.0, 2.0].into()),
                    view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
                })
            );
        }

        Ok(())
    }

    #[test]
    fn test_static_view_coordinates_projection() -> Result<(), Box<dyn std::error::Error>> {
        for flavor in &ALL_STATIC_TEST_FLAVOURS {
            let image_from_camera =
                PinholeProjection::from_focal_length_and_principal_point([1.0, 2.0], [1.0, 2.0]);

            // Static view coordinates, non-static pinhole.
            let timeline = Timeline::new_sequence("t");
            let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(TimePoint::default(), &archetypes::ViewCoordinates::BRU())
                .build()?;
            let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row(TimePoint::default(), &archetypes::ViewCoordinates::BLU())
                .build()?;
            let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype_auto_row([(timeline, 1)], &Pinhole::new(image_from_camera))
                .build()?;

            let mut cache = TransformResolutionCache::default();
            let entity_db = static_test_setup_store(
                &mut cache,
                prior_static_chunk,
                final_static_chunk,
                regular_chunk,
                *flavor,
            )?;

            // Check that the transform cache has the expected transforms.
            apply_store_subscriber_events(&mut cache, &entity_db);
            let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
            let transforms = transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "my_entity",
                )))
                .unwrap();

            // There's view coordinates, but that doesn't show up.
            assert_eq!(
                transforms.latest_at_pinhole(
                    &entity_db,
                    &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                ),
                None
            );
            assert_eq!(
                transforms.latest_at_pinhole(
                    &entity_db,
                    &LatestAtQuery::new(*timeline.name(), TimeInt::MIN)
                ),
                transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(*timeline.name(), 0)),
            );
            // Once we get a pinhole camera, the view coordinates should be there.
            assert_eq!(
                transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(*timeline.name(), 1)),
                Some(ResolvedPinholeProjection {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    image_from_camera,
                    resolution: None,
                    view_coordinates: components::ViewCoordinates::BLU,
                })
            );
        }

        Ok(())
    }

    #[test]
    fn test_tree_transforms() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &Transform3D::from_translation([1.0, 2.0, 3.0]),
            )
            .with_archetype_auto_row([(timeline, 3)], &Transform3D::from_scale([1.0, 2.0, 3.0]))
            .with_archetype_auto_row(
                [(timeline, 4)],
                &Transform3D::from_rotation(glam::Quat::from_rotation_x(1.0)),
            )
            .with_archetype_auto_row([(timeline, 5)], &Transform3D::clear_fields())
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Check that the transform cache has the expected transforms.
        apply_store_subscriber_events(&mut cache, &entity_db);
        let timeline_name = *timeline.name();
        let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();

        for (t, expected) in [
            (0, None),
            (
                1,
                Some(DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0))),
            ),
            (
                2,
                Some(DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0))),
            ),
            (3, Some(DAffine3::from_scale(glam::dvec3(1.0, 2.0, 3.0)))),
            (
                4,
                // Note: We must use the same conversion path as the actual implementation:
                // glam::Quat (f32) -> Quaternion (f32) -> glam::DQuat (f64)
                // This involves casting f32 components to f64 and renormalizing, which produces
                // slightly different values than directly computing in f64.
                Some(DAffine3::from_quat(
                    convert::quaternion_to_dquat(re_sdk_types::datatypes::Quaternion::from(
                        glam::Quat::from_rotation_x(1.0),
                    ))
                    .unwrap(),
                )),
            ),
            (5, Some(DAffine3::IDENTITY)), // Empty transform is treated as connected with identity.
            (123, Some(DAffine3::IDENTITY)), // Empty transform is treated as connected with identity.
        ] {
            assert_eq!(
                transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                expected.map(|transform| ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform,
                }),
                "at time {t}"
            );
        }

        Ok(())
    }

    #[test]
    fn test_pose_transforms_instance_poses() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &InstancePoses3D::new().with_translations([
                    [1.0, 2.0, 3.0],
                    [4.0, 5.0, 6.0],
                    [7.0, 8.0, 9.0],
                ]),
            )
            .with_archetype_auto_row(
                [(timeline, 3)],
                // Less instances, and a splatted scale.
                &InstancePoses3D::new()
                    .with_translations([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]])
                    .with_scales([[2.0, 3.0, 4.0]]),
            )
            .with_archetype_auto_row([(timeline, 4)], &InstancePoses3D::clear_fields())
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Check that the transform cache has the expected transforms.
        apply_store_subscriber_events(&mut cache, &entity_db);
        let timeline = *timeline.name();
        let transforms_per_timeline = cache.transforms_for_timeline(timeline);
        let transforms = transforms_per_timeline
            .pose_transforms(EntityPath::from("my_entity").hash())
            .unwrap();

        for (t, poses) in [
            (0, Vec::new()),
            (
                1,
                vec![
                    DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                    DAffine3::from_translation(glam::dvec3(4.0, 5.0, 6.0)),
                    DAffine3::from_translation(glam::dvec3(7.0, 8.0, 9.0)),
                ],
            ),
            (
                2,
                vec![
                    DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                    DAffine3::from_translation(glam::dvec3(4.0, 5.0, 6.0)),
                    DAffine3::from_translation(glam::dvec3(7.0, 8.0, 9.0)),
                ],
            ),
            (
                3,
                vec![
                    DAffine3::from_scale_rotation_translation(
                        glam::dvec3(2.0, 3.0, 4.0),
                        glam::DQuat::IDENTITY,
                        glam::dvec3(1.0, 2.0, 3.0),
                    ),
                    DAffine3::from_scale_rotation_translation(
                        glam::dvec3(2.0, 3.0, 4.0),
                        glam::DQuat::IDENTITY,
                        glam::dvec3(4.0, 5.0, 6.0),
                    ),
                ],
            ),
            (4, Vec::new()),
            (123, Vec::new()),
        ] {
            assert_eq!(
                transforms.latest_at_instance_poses(&entity_db, &LatestAtQuery::new(timeline, t)),
                poses,
                "Unexpected result at time {t}"
            );
        }

        Ok(())
    }

    #[test]
    fn test_pinhole_projections() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        let image_from_camera =
            PinholeProjection::from_focal_length_and_principal_point([1.0, 2.0], [1.0, 2.0]);

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row([(timeline, 1)], &Pinhole::new(image_from_camera))
            .with_archetype_auto_row([(timeline, 3)], &archetypes::ViewCoordinates::BLU())
            // Clear out the pinhole projection (this should yield nothing then for the remaining view coordinates.)
            .with_archetype_auto_row([(timeline, 4)], &Pinhole::clear_fields())
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Check that the transform cache has the expected transforms.
        apply_store_subscriber_events(&mut cache, &entity_db);
        let timeline = *timeline.name();
        let transforms_per_timeline = cache.transforms_for_timeline(timeline);
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();

        for (t, pinhole_view_coordinates) in [
            (0, None),
            (1, Some(Pinhole::DEFAULT_CAMERA_XYZ)),
            (2, Some(Pinhole::DEFAULT_CAMERA_XYZ)),
            (3, Some(components::ViewCoordinates::BLU)),
            (4, None), // View coordinates alone doesn't give us a pinhole projection from the transform cache.
            (123, None),
        ] {
            assert_eq!(
                transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(timeline, t)),
                pinhole_view_coordinates.map(|view_coordinates| ResolvedPinholeProjection {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    image_from_camera,
                    resolution: None,
                    view_coordinates,
                }),
                "Unexpected result at time {t}"
            );
        }

        Ok(())
    }

    #[test]
    fn test_out_of_order_updates() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &Transform3D::from_translation([1.0, 2.0, 3.0]),
            )
            .with_archetype_auto_row(
                [(timeline, 3)],
                // Note that this clears anything that could be inserted at time 2 due to atomic-query semantics.
                &Transform3D::from_translation([2.0, 3.0, 4.0]),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Check that the transform cache has the expected transforms.
        apply_store_subscriber_events(&mut cache, &entity_db);
        let timeline = *timeline.name();

        {
            let transforms_per_timeline = cache.transforms_for_timeline(timeline);
            let transforms = transforms_per_timeline
                .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                    "my_entity",
                )))
                .unwrap();

            // Check that the transform cache has the expected transforms.
            for (t, transform) in [
                (1, DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0))),
                (2, DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0))),
                (3, DAffine3::from_translation(glam::dvec3(2.0, 3.0, 4.0))),
            ] {
                assert_eq!(
                    transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline, t)),
                    Some(ParentFromChildTransform {
                        parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                        transform,
                    }),
                    "Unexpected result at time {t}",
                );
            }
        }

        // Add a transform between the two.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 2)],
                &Transform3D::from_scale([-1.0, -2.0, -3.0]),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Check that the transform cache has the expected changed transforms.
        apply_store_subscriber_events(&mut cache, &entity_db);
        let timeline = *timeline.name();
        let transforms_per_timeline = cache.transforms_for_timeline(timeline);
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();

        // Check that the transform cache has the expected transforms.
        for (t, transform) in [
            (1, DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0))),
            (2, DAffine3::from_scale(glam::dvec3(-1.0, -2.0, -3.0))),
            (3, DAffine3::from_translation(glam::dvec3(2.0, 3.0, 4.0))),
        ] {
            assert_eq!(
                transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline, t)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform,
                }),
                "Unexpected result at time {t}",
            );
        }

        Ok(())
    }

    #[test]
    fn test_clear_non_recursive() -> Result<(), Box<dyn std::error::Error>> {
        for (clear_in_separate_chunk, first_clear_then_data) in
            [(false, false), (true, false), (true, true)]
        {
            println!("clear_in_separate_chunk: {clear_in_separate_chunk}");
            println!("first_clear_then_data: {first_clear_then_data}");

            let mut entity_db = new_entity_db_with_subscriber_registered();
            let mut cache = TransformResolutionCache::default();

            let timeline = Timeline::new_sequence("t");
            let timeline_name = *timeline.name();

            let path = EntityPath::from("ent");
            let data_chunk = Chunk::builder(path.clone())
                .with_archetype_auto_row(
                    [(timeline, 1)],
                    &Transform3D::from_translation([1.0, 2.0, 3.0]),
                )
                .with_archetype_auto_row(
                    [(timeline, 3)],
                    &Transform3D::from_translation([3.0, 4.0, 5.0]),
                )
                .build()?;
            let clear_chunk = Chunk::builder(path.clone())
                .with_archetype_auto_row([(timeline, 2)], &archetypes::Clear::new(false))
                .build()?;

            if clear_in_separate_chunk && !first_clear_then_data {
                entity_db.add_chunk(&Arc::new(data_chunk))?;

                // If we're putting the clear in a separate chunk, we can try warming the cache and see whether we get the right transforms.
                {
                    apply_store_subscriber_events(&mut cache, &entity_db);
                    let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
                    let transforms = transforms_per_timeline
                        .frame_transforms(TransformFrameIdHash::from_entity_path(&path))
                        .unwrap();
                    assert_eq!(
                        transforms
                            .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
                        Some(ParentFromChildTransform {
                            parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                            transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                        })
                    );
                    assert_eq!(
                        transforms
                            .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 3)),
                        Some(ParentFromChildTransform {
                            parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                            transform: DAffine3::from_translation(glam::dvec3(3.0, 4.0, 5.0)),
                        })
                    );
                }

                // Now add a separate chunk with a clear.
                entity_db.add_chunk(&Arc::new(clear_chunk))?;
            } else if clear_in_separate_chunk && first_clear_then_data {
                // First add clear chunk.
                entity_db.add_chunk(&Arc::new(clear_chunk))?;

                // Warm the cache with this situation.
                apply_store_subscriber_events(&mut cache, &entity_db);
                let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
                assert_eq!(
                    transforms_per_timeline
                        .frame_transforms(TransformFrameIdHash::from_entity_path(&path)),
                    None
                );

                // And only now add the data chunk.
                entity_db.add_chunk(&Arc::new(data_chunk))?;
            } else {
                let chunk = data_chunk.concatenated(&clear_chunk)?;
                entity_db.add_chunk(&Arc::new(chunk))?;
            }

            // Check transforms AFTER we apply the clear.
            {
                apply_store_subscriber_events(&mut cache, &entity_db);
                let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
                let transforms = transforms_per_timeline
                    .frame_transforms(TransformFrameIdHash::from_entity_path(&path))
                    .unwrap();

                assert_eq!(
                    transforms
                        .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
                    Some(ParentFromChildTransform {
                        parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                        transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                    })
                );
                assert_eq!(
                    transforms
                        .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 2)),
                    None
                );
                assert_eq!(
                    transforms
                        .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 3)),
                    Some(ParentFromChildTransform {
                        parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                        transform: DAffine3::from_translation(glam::dvec3(3.0, 4.0, 5.0)),
                    })
                );
            }
        }

        Ok(())
    }

    #[test]
    fn test_clear_recursive() -> Result<(), Box<dyn std::error::Error>> {
        for (clear_in_separate_chunk, update_after_each_chunk) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            println!(
                "clear_in_separate_chunk: {clear_in_separate_chunk}, apply_after_each_chunk: {update_after_each_chunk}",
            );

            let mut entity_db = new_entity_db_with_subscriber_registered();
            let mut cache = TransformResolutionCache::default();

            let timeline = Timeline::new_sequence("t");

            let mut parent_chunk = Chunk::builder(EntityPath::from("parent"))
                .with_archetype_auto_row(
                    [(timeline, 1)],
                    &Transform3D::from_translation([1.0, 2.0, 3.0]),
                );
            if !clear_in_separate_chunk {
                parent_chunk = parent_chunk
                    .with_archetype_auto_row([(timeline, 2)], &archetypes::Clear::new(true));
            }
            entity_db.add_chunk(&Arc::new(parent_chunk.build()?))?;
            if update_after_each_chunk {
                apply_store_subscriber_events(&mut cache, &entity_db);
            }

            let child_chunk = Chunk::builder(EntityPath::from("parent/child"))
                .with_archetype_auto_row(
                    [(timeline, 1)],
                    &Transform3D::from_translation([1.0, 2.0, 3.0]),
                );
            entity_db.add_chunk(&Arc::new(child_chunk.build()?))?;
            if update_after_each_chunk {
                apply_store_subscriber_events(&mut cache, &entity_db);
            }

            if clear_in_separate_chunk {
                let chunk = Chunk::builder(EntityPath::from("parent"))
                    .with_archetype_auto_row([(timeline, 2)], &archetypes::Clear::new(true))
                    .build()?;
                entity_db.add_chunk(&Arc::new(chunk))?;
                if update_after_each_chunk {
                    apply_store_subscriber_events(&mut cache, &entity_db);
                }
            }

            let timeline = *timeline.name();
            apply_store_subscriber_events(&mut cache, &entity_db);
            let transforms_per_timeline = cache.transforms_for_timeline(timeline);

            for path in [EntityPath::from("parent"), EntityPath::from("parent/child")] {
                let transform = transforms_per_timeline
                    .frame_transforms(TransformFrameIdHash::from_entity_path(&path))
                    .unwrap();

                println!("checking for correct transforms for path: {path:?}");

                assert_eq!(
                    transform.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline, 1)),
                    Some(ParentFromChildTransform {
                        parent: TransformFrameIdHash::from_entity_path(&path.parent().unwrap()),
                        transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                    })
                );
                assert_eq!(
                    transform.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline, 2)),
                    None
                );
            }
        }

        Ok(())
    }

    #[derive(Debug, Clone, Copy)]
    enum ChildParentFrameChangesOverTimeTestMode {
        SingleChunk,
        MultipleChunksInOrder,
        MultipleChunksReverseOrder,
    }

    fn test_single_child_and_parent_over_time(
        mode: ChildParentFrameChangesOverTimeTestMode,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        let timeline = Timeline::new_sequence("t");
        let timeline_name = *timeline.name();

        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &Transform3D::from_translation([1.0, 0.0, 0.0]),
            )
            .with_archetype_auto_row(
                [(timeline, 2)],
                &Transform3D::new()
                    .with_translation([2.0, 0.0, 0.0])
                    .with_child_frame("frame0"), // Uses implicit entity-path derived parent frame.
            )
            .with_archetype_auto_row(
                [(timeline, 3)],
                &Transform3D::new()
                    .with_translation([3.0, 0.0, 0.0])
                    .with_child_frame("frame0")
                    .with_parent_frame("frame1"),
            )
            .with_archetype_auto_row(
                [(timeline, 4)],
                &Transform3D::new()
                    .with_translation([4.0, 0.0, 0.0])
                    .with_child_frame("frame2")
                    .with_parent_frame("frame3"),
            )
            .build()?;

        match mode {
            ChildParentFrameChangesOverTimeTestMode::SingleChunk => {
                entity_db.add_chunk(&Arc::new(chunk))?;
                apply_store_subscriber_events(&mut cache, &entity_db);
            }
            ChildParentFrameChangesOverTimeTestMode::MultipleChunksInOrder => {
                for row_idx in 0..chunk.num_rows() {
                    entity_db.add_chunk(&Arc::new(
                        chunk.row_sliced_shallow(row_idx, 1).with_id(ChunkId::new()),
                    ))?;
                    apply_store_subscriber_events(&mut cache, &entity_db);
                }
            }
            ChildParentFrameChangesOverTimeTestMode::MultipleChunksReverseOrder => {
                for row_idx in (0..chunk.num_rows()).rev() {
                    entity_db.add_chunk(&Arc::new(
                        chunk.row_sliced_shallow(row_idx, 1).with_id(ChunkId::new()),
                    ))?;
                    apply_store_subscriber_events(&mut cache, &entity_db);
                }
            }
        }

        let timeline_transforms = cache.transforms_for_timeline(*timeline.name());

        // State of the implicit frame over time.
        let transforms_implicit_frame = timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_entity_path(&EntityPath::from(
                "my_entity",
            )))
            .unwrap();
        // Nothing we add over time affects the implicit frame whose relationship is set at frame 1
        for t in [1, 2, 3, 4, 5] {
            assert_eq!(
                transforms_implicit_frame
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(glam::dvec3(1.0, 0.0, 0.0)),
                }),
                "querying at t=={t}"
            );
        }

        // State of frame0 over time.
        let transforms_frame0 = timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_str("frame0"))
            .unwrap();
        for (t, expected_translation_and_parent) in [
            (4, Some((3.0, TransformFrameIdHash::from_str("frame1")))),
            (3, Some((3.0, TransformFrameIdHash::from_str("frame1")))),
            (
                2,
                Some((2.0, TransformFrameIdHash::entity_path_hierarchy_root())),
            ),
            (1, None),
            (0, None),
        ] {
            assert_eq!(
                transforms_frame0
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                expected_translation_and_parent.map(|(x, parent)| ParentFromChildTransform {
                    parent,
                    transform: DAffine3::from_translation(glam::dvec3(x, 0.0, 0.0)),
                }),
                "querying at t=={t}"
            );
        }

        // frame1 is never a child, only a parent.
        assert!(
            timeline_transforms
                .frame_transforms(TransformFrameIdHash::from_str("custom_frame1"))
                .is_none(),
        );

        // State of frame2 over time.
        let transforms_frame2 = timeline_transforms
            .frame_transforms(TransformFrameIdHash::from_str("frame2"))
            .unwrap();
        for t in [1, 2, 3] {
            assert_eq!(
                transforms_frame2
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                None
            );
        }
        for t in [4, 5] {
            assert_eq!(
                transforms_frame2
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::from_str("frame3"),
                    transform: DAffine3::from_translation(glam::dvec3(4.0, 0.0, 0.0)),
                }),
                "querying at t=={t}"
            );
        }

        // frame3 is never a child, only a parent.
        assert!(
            timeline_transforms
                .frame_transforms(TransformFrameIdHash::from_str("custom_frame3"))
                .is_none()
        );

        Ok(())
    }

    #[test]
    fn test_single_child_and_parent_over_time_single_chunk()
    -> Result<(), Box<dyn std::error::Error>> {
        test_single_child_and_parent_over_time(ChildParentFrameChangesOverTimeTestMode::SingleChunk)
    }

    #[test]
    fn test_single_child_and_parent_over_time_multiple_chunks_in_order()
    -> Result<(), Box<dyn std::error::Error>> {
        test_single_child_and_parent_over_time(
            ChildParentFrameChangesOverTimeTestMode::MultipleChunksInOrder,
        )
    }

    #[test]
    fn test_single_child_and_parent_over_time_multiple_chunks_reverse_order()
    -> Result<(), Box<dyn std::error::Error>> {
        test_single_child_and_parent_over_time(
            ChildParentFrameChangesOverTimeTestMode::MultipleChunksReverseOrder,
        )
    }

    #[test]
    fn test_static_child_frames() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        let timeline = Timeline::new_sequence("t");
        let timeline_name = *timeline.name();

        let temporal_entity_path = EntityPath::from("my_entity");
        let static_entity_path = EntityPath::from("my_static_entity");

        entity_db.add_chunk(&Arc::new(
            Chunk::builder(static_entity_path.clone())
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &Transform3D::new()
                        .with_translation([1.0, 0.0, 0.0])
                        .with_child_frame("frame0"),
                )
                .build()?,
        ))?;
        entity_db.add_chunk(&Arc::new(
            Chunk::builder(temporal_entity_path)
                .with_archetype_auto_row(
                    [(timeline, 1)],
                    &Transform3D::new()
                        .with_translation([2.0, 0.0, 0.0])
                        .with_child_frame("frame1"),
                )
                .build()?,
        ))?;
        apply_store_subscriber_events(&mut cache, &entity_db);

        {
            let timeline_transforms = cache.transforms_for_timeline(*timeline.name());

            // Check frame0 only ever sees the static transform.
            let transforms_frame0 = timeline_transforms
                .frame_transforms(TransformFrameIdHash::from_str("frame0"))
                .unwrap();
            assert_eq!(
                transforms_frame0
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(glam::dvec3(1.0, 0.0, 0.0)),
                })
            );

            // Check frame1 only ever sees the temporal transform.
            let transforms_frame1 = timeline_transforms
                .frame_transforms(TransformFrameIdHash::from_str("frame1"))
                .unwrap();
            assert_eq!(
                transforms_frame1
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0)),
                None
            );
            assert_eq!(
                transforms_frame1
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(glam::dvec3(2.0, 0.0, 0.0)),
                })
            );
        }

        // Now we change the static chunk to also talk about a new frame2.
        // (Note we're not allowed to also mention frame1 since it is already used by our non-temporal entity)
        // Before, there was a translation there but due to atomic latest-at we won't see that.
        entity_db.add_chunk(&Arc::new(
            Chunk::builder(static_entity_path)
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &Transform3D::new()
                        .with_child_frame("frame2")
                        .with_scale(2.0),
                )
                .build()?,
        ))?;
        apply_store_subscriber_events(&mut cache, &entity_db);

        {
            let timeline_transforms = cache.transforms_for_timeline(*timeline.name());

            // Information about frame0 is still there, just like it would be when adding additional temporal rows at the same time.
            let transforms_frame0 = timeline_transforms
                .frame_transforms(TransformFrameIdHash::from_str("frame0"))
                .unwrap();
            assert_eq!(
                transforms_frame0
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(glam::dvec3(1.0, 0.0, 0.0)),
                })
            );

            // But there's also a new frame2.
            let transforms_frame2 = timeline_transforms
                .frame_transforms(TransformFrameIdHash::from_str("frame2"))
                .unwrap();
            assert_eq!(
                transforms_frame2
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_scale(glam::DVec3::splat(2.0)),
                })
            );
        }

        Ok(())
    }

    #[test]
    fn test_different_associated_paths_for_static_and_temporal()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        let timeline = Timeline::new_sequence("t");
        let timeline_name = *timeline.name();

        let static_entity_path = EntityPath::from("static_entity");
        let temporal_entity_path = EntityPath::from("temporal_entity");
        let child_frame = TransformFrameIdHash::from_str("child_frame");

        let static_chunk = Chunk::builder(static_entity_path.clone())
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &Transform3D::new()
                    .with_translation([1.0, 2.0, 3.0])
                    .with_child_frame("child_frame")
                    .with_parent_frame("parent_frame"),
            )
            .build()?;
        let temporal_chunk = Chunk::builder(temporal_entity_path.clone())
            .with_archetype_auto_row(
                [(timeline, 1)],
                &Transform3D::new()
                    .with_translation([4.0, 5.0, 6.0])
                    .with_child_frame("child_frame")
                    .with_parent_frame("parent_frame"),
            )
            .build()?;

        #[derive(Debug)]
        enum Scenario {
            StaticAndTemporalAtOnce,
            StaticFirstThenTemporal,
            TemporalFirstThenStatic,
        }

        for scenario in [
            Scenario::StaticAndTemporalAtOnce,
            Scenario::StaticFirstThenTemporal,
            Scenario::TemporalFirstThenStatic,
        ] {
            match scenario {
                Scenario::StaticAndTemporalAtOnce => {
                    entity_db.add_chunk(&Arc::new(static_chunk.clone()))?;
                    entity_db.add_chunk(&Arc::new(temporal_chunk.clone()))?;
                }
                Scenario::StaticFirstThenTemporal => {
                    entity_db.add_chunk(&Arc::new(static_chunk.clone()))?;
                }
                Scenario::TemporalFirstThenStatic => {
                    entity_db.add_chunk(&Arc::new(temporal_chunk.clone()))?;
                }
            }
            apply_store_subscriber_events(&mut cache, &entity_db);

            // Warm cache.
            {
                let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
                let transforms = transforms_per_timeline
                    .frame_transforms(child_frame)
                    .unwrap();
                transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0));
                transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1));
            }

            // Add extra chunk.
            match scenario {
                Scenario::StaticAndTemporalAtOnce => {
                    // Already added both.
                }
                Scenario::StaticFirstThenTemporal => {
                    entity_db.add_chunk(&Arc::new(temporal_chunk.clone()))?;
                }
                Scenario::TemporalFirstThenStatic => {
                    entity_db.add_chunk(&Arc::new(static_chunk.clone()))?;
                }
            }
            apply_store_subscriber_events(&mut cache, &entity_db);

            // Both static and temporal data should be accessible
            let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
            let transforms = transforms_per_timeline
                .frame_transforms(child_frame)
                .unwrap();

            // At time 0, should see static data
            assert_eq!(
                transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 0)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::from_str("parent_frame"),
                    transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                }),
                "Unexpected transform at time 0 (scenario: {scenario:?})",
            );
            // At time 1, should see temporal data (overriding static due to atomic-latest-at)
            assert_eq!(
                transforms.latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, 1)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::from_str("parent_frame"),
                    transform: DAffine3::from_translation(glam::dvec3(4.0, 5.0, 6.0)),
                }),
                "Unexpected transform at time 1 (scenario: {scenario:?})",
            );

            // Verify associated entity paths are correctly tracked
            assert_eq!(
                transforms.associated_entity_path(TimeInt::STATIC),
                &static_entity_path,
                "Unexpected path for static data (scenario: {scenario:?})",
            );
            assert_eq!(
                transforms.associated_entity_path(TimeInt::new_temporal(1)),
                &temporal_entity_path,
                "Unexpected path for temporal data (scenario: {scenario:?})",
            );

            // Test on a different timeline that never saw the temporal data
            let other_timeline = TimelineName::new("other");
            let transforms_per_timeline = cache.transforms_for_timeline(other_timeline);
            let transforms = transforms_per_timeline
                .frame_transforms(child_frame)
                .unwrap();
            assert_eq!(
                transforms
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(other_timeline, 100)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::from_str("parent_frame"),
                    transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                }),
                "Unexpected transform on other timeline (scenario: {scenario:?})",
            );
        }

        Ok(())
    }

    fn test_error_on_changing_associated_path(
        time: TimeInt,
    ) -> Result<(), Box<dyn std::error::Error>> {
        re_log::setup_logging();
        let (logger, log_rx) = re_log::ChannelLogger::new(re_log::LevelFilter::Error);
        re_log::add_boxed_logger(Box::new(logger)).expect("Failed to add logger");

        let mut entity_db = EntityDb::new(StoreInfo::testing().store_id);
        let mut cache = TransformResolutionCache::default();

        let time_point = if time.is_static() {
            TimePoint::STATIC
        } else {
            [(Timeline::new_sequence("t"), time)].into()
        };

        // First, create temporal transform
        let temporal_chunk1 = Chunk::builder(EntityPath::from("entity_a"))
            .with_archetype_auto_row(
                time_point.clone(),
                &Transform3D::from_translation([1.0, 0.0, 0.0]).with_child_frame("my_frame"),
            )
            .build()?;
        cache.process_store_events(entity_db.add_chunk(&Arc::new(temporal_chunk1))?.iter());

        assert!(log_rx.try_recv().is_err());

        // Try to associate the same frame with a different temporal entity - should log error
        let temporal_chunk2 = Chunk::builder(EntityPath::from("entity_b"))
            .with_archetype_auto_row(
                time_point,
                &Transform3D::from_translation([2.0, 0.0, 0.0]).with_child_frame("my_frame"),
            )
            .build()?;
        cache.process_store_events(entity_db.add_chunk(&Arc::new(temporal_chunk2))?.iter());

        let error = log_rx.try_recv().unwrap();
        assert!(log_rx.try_recv().is_err()); // Exactly one error.

        assert_eq!(error.level, re_log::Level::Error);
        assert!(
            error.msg.contains("entity_a"),
            "Expected to mention previous entity, but msg was {}",
            error.msg
        );
        assert!(
            error.msg.contains("entity_b"),
            "Expected to mention new entity, but msg was {}",
            error.msg
        );
        assert!(
            error.msg.contains("my_frame"),
            "Expected to mention target, but msg was {}",
            error.msg
        );

        Ok(())
    }

    #[test]
    fn test_error_on_changing_associated_path_static() -> Result<(), Box<dyn std::error::Error>> {
        test_error_on_changing_associated_path(TimeInt::STATIC)
    }

    #[test]
    fn test_error_on_changing_associated_path_temporal() -> Result<(), Box<dyn std::error::Error>> {
        test_error_on_changing_associated_path(TimeInt::new_temporal(0))
    }

    #[test]
    fn test_pinhole_with_explicit_frames() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        let timeline = Timeline::new_sequence("t");
        let timeline_name = *timeline.name();

        let image_from_camera =
            PinholeProjection::from_focal_length_and_principal_point([1.0, 2.0], [1.0, 2.0]);

        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            // Add pinhole with explicit child and parent frames
            .with_archetype_auto_row(
                [(timeline, 0)],
                &Pinhole::new(image_from_camera)
                    .with_child_frame("child_frame")
                    .with_parent_frame("parent_frame"),
            )
            // Add a 3D transform on top.
            .with_archetype_auto_row(
                [(timeline, 1)],
                &Transform3D::from_translation([1.0, 2.0, 3.0])
                    .with_child_frame("child_frame")
                    .with_parent_frame("parent_frame"),
            )
            // Add a 3D transform to a different child frame.
            .with_archetype_auto_row(
                [(timeline, 2)],
                &Transform3D::from_translation([3.0, 4.0, 5.0])
                    .with_child_frame("other_frame")
                    .with_parent_frame("parent_frame"),
            )
            // Add a pinhole to that same relation, this time with an explicit resolution.
            .with_archetype_auto_row(
                [(timeline, 3)],
                &Pinhole::new(image_from_camera)
                    .with_resolution([1.0, 2.0])
                    .with_child_frame("other_frame")
                    .with_parent_frame("parent_frame"),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        apply_store_subscriber_events(&mut cache, &entity_db);

        let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);

        // Check transforms going out from child_frame
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_str("child_frame"))
            .unwrap();
        for t in [0, 1, 2, 3] {
            // Pinhole from child_frame->X exists at all times unchanged.
            assert_eq!(
                transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                Some(ResolvedPinholeProjection {
                    parent: TransformFrameIdHash::from_str("parent_frame"),
                    image_from_camera,
                    resolution: None,
                    view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
                }),
                "Unexpected pinhole for child_frame at time t={t}"
            );

            // After time 1 we have a transform on top
            if t == 0 {
                assert_eq!(
                    transforms
                        .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                    None,
                    "Unexpected transform for child_frame at time t={t}"
                );
            } else {
                assert_eq!(
                    transforms
                        .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                    Some(ParentFromChildTransform {
                        parent: TransformFrameIdHash::from_str("parent_frame"),
                        transform: DAffine3::from_translation(glam::dvec3(1.0, 2.0, 3.0)),
                    }),
                    "Unexpected transform for child_frame at time t={t}"
                );
            }
        }

        // Check transforms going out from other_frame
        let transforms = transforms_per_timeline
            .frame_transforms(TransformFrameIdHash::from_str("other_frame"))
            .unwrap();
        for t in [0, 1, 2, 3] {
            // Pinhole from other_frame->X exists only at time t==3
            if t < 3 {
                assert_eq!(
                    transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                    None,
                    "Unexpected pinhole for other_frame at time t={t}"
                );
            } else {
                assert_eq!(
                    transforms.latest_at_pinhole(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                    Some(ResolvedPinholeProjection {
                        parent: TransformFrameIdHash::from_str("parent_frame"),
                        image_from_camera,
                        resolution: Some([1.0, 2.0].into()),
                        view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
                    }),
                    "Unexpected pinhole for other_frame at time t={t}"
                );
            }

            // After time 2 we have a transform.
            if t < 2 {
                assert_eq!(
                    transforms
                        .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                    None,
                    "Unexpected transform for other_frame at time t={t}"
                );
            } else {
                assert_eq!(
                    transforms
                        .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, t)),
                    Some(ParentFromChildTransform {
                        parent: TransformFrameIdHash::from_str("parent_frame"),
                        transform: DAffine3::from_translation(glam::dvec3(3.0, 4.0, 5.0)),
                    }),
                    "Unexpected transform for other_frame at time t={t}"
                );
            }
        }

        Ok(())
    }

    // TODO(andreas): We're missing tests for more corner cases involving child frames and (recursive) clears.

    #[test]
    fn test_gc() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity0"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &Transform3D::from_translation([1.0, 2.0, 3.0]),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Apply some updates to the transform before GC pass.
        apply_store_subscriber_events(&mut cache, &entity_db);
        let num_bytes_before_gc = cache.total_size_bytes();

        let chunk = Chunk::builder(EntityPath::from("my_entity1"))
            .with_archetype_auto_row(
                [(timeline, 2)],
                &Transform3D::from_translation([4.0, 5.0, 6.0]),
            )
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Don't apply updates for this chunk.
        let _store_events = entity_db.gc(&GarbageCollectionOptions::gc_everything());
        apply_store_subscriber_events(&mut cache, &entity_db);
        let num_bytes_after_gc = cache.total_size_bytes();
        assert!(
            num_bytes_after_gc < num_bytes_before_gc,
            "Expected cache size to decrease after GC (before/after: {num_bytes_before_gc} bytes)"
        );

        assert_eq!(
            cache
                .transforms_for_timeline(*timeline.name())
                .per_child_frame_transforms,
            cache.static_timeline.per_child_frame_transforms
        );

        Ok(())
    }

    // Tests GCing a recursive clear.
    #[test]
    fn test_gc_recursive_clear() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = new_entity_db_with_subscriber_registered();
        let mut cache = TransformResolutionCache::default();

        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_recursive_clear"))
            .with_archetype_auto_row([(timeline, 1)], &archetypes::Clear::new(true))
            .build()?;
        entity_db.add_chunk(&Arc::new(chunk))?;

        // Apply some updates to the transform before GC pass.
        apply_store_subscriber_events(&mut cache, &entity_db);

        assert!(
            cache
                .transforms_for_timeline(*timeline.name())
                .recursive_clears
                .contains_key(&EntityPath::from("my_recursive_clear")),
        );

        // Don't apply updates for this chunk.
        let _store_events = entity_db.gc(&GarbageCollectionOptions::gc_everything());
        apply_store_subscriber_events(&mut cache, &entity_db);

        assert!(
            cache
                .transforms_for_timeline(*timeline.name())
                .recursive_clears
                .is_empty(),
        );

        Ok(())
    }

    #[test]
    fn test_cache_invalidation() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = EntityDb::new(StoreInfo::testing().store_id);
        let mut cache = TransformResolutionCache::default();

        let timeline = Timeline::new_sequence("t");
        let timeline_name = *timeline.name();
        let frame = TransformFrameIdHash::from_entity_path(&EntityPath::from("my_entity"));

        // Initial chunk with various events, some of which don't do anything about transforms.
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &Transform3D::from_translation([1.0, 0.0, 0.0]),
            )
            .with_archetype_auto_row([(timeline, 2)], &MyPoints::new([MyPoint::new(0.0, 0.0)]))
            .with_archetype_auto_row(
                [(timeline, 3)],
                &Transform3D::from_translation([2.0, 0.0, 0.0]),
            )
            .build()?;
        cache.process_store_events(entity_db.add_chunk(&Arc::new(chunk))?.iter());

        // Query all transforms, warming the cache.
        let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
        let transforms = transforms_per_timeline.frame_transforms(frame).unwrap();
        for (time, expected_translation) in [
            (1, glam::dvec3(1.0, 0.0, 0.0)),
            (2, glam::dvec3(1.0, 0.0, 0.0)),
            (3, glam::dvec3(2.0, 0.0, 0.0)),
        ] {
            assert_eq!(
                transforms
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, time)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(expected_translation),
                }),
                "querying at time {time}"
            );
        }

        // New chunk overriding some of the times and adding new ones.
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 1)],
                &Transform3D::from_translation([3.0, 0.0, 0.0]),
            )
            .with_archetype_auto_row(
                [(timeline, 2)],
                &Transform3D::from_translation([4.0, 0.0, 0.0]),
            )
            .with_archetype_auto_row(
                [(timeline, 5)],
                &Transform3D::from_translation([5.0, 0.0, 0.0]),
            )
            .build()?;
        cache.process_store_events(entity_db.add_chunk(&Arc::new(chunk))?.iter());

        // Query again, ensuring we get new transforms.
        let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
        let transforms = transforms_per_timeline.frame_transforms(frame).unwrap();
        for (time, expected_translation) in [
            (1, glam::dvec3(3.0, 0.0, 0.0)),
            (2, glam::dvec3(4.0, 0.0, 0.0)),
            (3, glam::dvec3(2.0, 0.0, 0.0)),
            (4, glam::dvec3(2.0, 0.0, 0.0)),
            (5, glam::dvec3(5.0, 0.0, 0.0)),
        ] {
            assert_eq!(
                transforms
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, time)),
                Some(ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(expected_translation),
                }),
                "querying at time {time}"
            );
        }

        // Add a clear chunk.
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row([(timeline, 3)], &archetypes::Clear::new(false))
            .build()?;
        cache.process_store_events(entity_db.add_chunk(&Arc::new(chunk))?.iter());

        // Query again, ensure the transform is cleared in the right places.
        let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
        let transforms = transforms_per_timeline.frame_transforms(frame).unwrap();
        for (time, expected_translation) in [
            (1, Some(glam::dvec3(3.0, 0.0, 0.0))),
            (2, Some(glam::dvec3(4.0, 0.0, 0.0))),
            (3, None),
            (4, None),
            (5, Some(glam::dvec3(5.0, 0.0, 0.0))),
        ] {
            assert_eq!(
                transforms
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, time)),
                expected_translation.map(|translation| ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(translation),
                }),
                "querying at time {time}"
            );
        }

        // Add a chunk that tries to restore the transform _at_ the clear.
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype_auto_row(
                [(timeline, 3)],
                &Transform3D::from_translation([6.0, 0.0, 0.0]),
            )
            .build()?;
        cache.process_store_events(entity_db.add_chunk(&Arc::new(chunk))?.iter());

        // Query again, ensure that the clear "wins" (no change to before)
        let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
        let transforms = transforms_per_timeline.frame_transforms(frame).unwrap();
        for (time, expected_translation) in [
            (1, Some(glam::dvec3(3.0, 0.0, 0.0))),
            (2, Some(glam::dvec3(4.0, 0.0, 0.0))),
            (3, None),
            (4, None),
            (5, Some(glam::dvec3(5.0, 0.0, 0.0))),
        ] {
            assert_eq!(
                transforms
                    .latest_at_transform(&entity_db, &LatestAtQuery::new(timeline_name, time)),
                expected_translation.map(|translation| ParentFromChildTransform {
                    parent: TransformFrameIdHash::entity_path_hierarchy_root(),
                    transform: DAffine3::from_translation(translation),
                }),
                "querying at time {time}"
            );
        }

        Ok(())
    }
}
