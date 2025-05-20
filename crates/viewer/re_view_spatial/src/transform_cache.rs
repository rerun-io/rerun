use std::collections::BTreeMap;

use ahash::{HashMap, HashSet};
use glam::Affine3A;
use itertools::Either;
use nohash_hasher::{IntMap, IntSet};

use once_cell::sync::OnceCell;
use re_chunk_store::{
    ChunkStore, ChunkStoreSubscriberHandle, LatestAtQuery, PerStoreChunkSubscriber,
};
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, EntityPathHash, StoreId, TimeInt, TimelineName};
use re_types::{
    Archetype as _, ArchetypeName, Component as _, ComponentDescriptor, ComponentName,
    archetypes::{self, InstancePoses3D},
    components::{self},
};
use vec1::smallvec_v1::SmallVec1;

/// Store subscriber that resolves all transform components at a given entity to an affine transform.
///
/// It only handles resulting transforms individually to each entity, not how these transforms propagate in the tree.
/// For transform tree propagation see [`crate::contexts::TransformTreeContext`].
///
/// There are different kinds of transforms handled here:
/// * [`archetypes::Transform3D`]
///   Tree transforms that should propagate in the tree (via [`crate::contexts::TransformTreeContext`]).
/// * [`archetypes::InstancePoses3D`]
///   Instance poses that should be applied to the tree transforms (via [`crate::contexts::TransformTreeContext`]) but not propagate.
/// * [`components::PinholeProjection`] and [`components::ViewCoordinates`]
///   Pinhole projections & associated view coordinates used for visualizing cameras in 3D and embedding 2D in 3D
///
/// Most of what this construct does internally is to keep track at which points in time these sets of components
/// change such that a latest-at query for them may (!) yield any new results.
/// It then (in [`TransformCacheStoreSubscriber::apply_all_updates`]) performs those queries and derives transforms from them.
pub struct TransformCacheStoreSubscriber {
    /// All components of [`archetypes::Transform3D`]
    transform_components: IntSet<ComponentName>,

    /// All components of [`archetypes::InstancePoses3D`]
    pose_components: IntSet<ComponentName>,

    /// All components related to pinholes (i.e. [`components::PinholeProjection`] and [`components::ViewCoordinates`]).
    pinhole_components: IntSet<ComponentName>,

    per_timeline: HashMap<TimelineName, CachedTransformsForTimeline>,
    static_timeline: CachedTransformsForTimeline,
}

impl Default for TransformCacheStoreSubscriber {
    #[inline]
    fn default() -> Self {
        use re_types::Archetype as _;

        Self {
            transform_components: archetypes::Transform3D::all_components()
                .iter()
                .map(|descr| descr.component_name)
                .collect(),
            pose_components: archetypes::InstancePoses3D::all_components()
                .iter()
                .map(|descr| descr.component_name)
                .collect(),
            pinhole_components: [
                components::PinholeProjection::name(),
                components::ViewCoordinates::name(),
            ]
            .into_iter()
            .collect(),

            per_timeline: Default::default(),
            static_timeline: CachedTransformsForTimeline {
                invalidated_transforms: Default::default(),
                per_entity: Default::default(),
                recursive_clears: Default::default(), // Unused for static timeline.
            },
        }
    }
}

bitflags::bitflags! {
    /// Flags for the different kinds of independent transforms that the transform cache handles.
    #[derive(Debug, Clone, Copy)]
    pub struct TransformAspect: u8 {
        /// The entity has a tree transform, i.e. any non-style component of [`archetypes::Transform3D`].
        const Tree = 1 << 0;

        /// The entity has instance poses, i.e. any non-style component of [`archetypes::InstancePoses3D`].
        const Pose = 1 << 1;

        /// The entity has a pinhole projection or view coordinates, i.e. either [`components::PinholeProjection`] or [`components::ViewCoordinates`].
        const PinholeOrViewCoordinates = 1 << 2;

        /// The entity has a clear component.
        const Clear = 1 << 3;
    }
}

/// Points in time that have changed for a given entity,
/// i.e. the cache is invalid for these times.
#[derive(Debug, Clone)]
struct InvalidatedTransforms {
    entity_path: EntityPath,
    times: Vec<TimeInt>,
    aspects: TransformAspect,
}

/// Cached transforms for a single timeline.
///
/// Includes any static transforms that may apply globally.
/// Therefore, this can't be trivially constructed.
pub struct CachedTransformsForTimeline {
    /// Updates that should be applied to the cache.
    /// I.e. times & entities at which the cache is invalid right now.
    invalidated_transforms: Vec<InvalidatedTransforms>,

    per_entity: IntMap<EntityPath, TransformsForEntity>,

    // We need to keep track of all recursive clears that ever happened and when.
    // Otherwise, new incoming entities may not correctly change their transform at the time of clear.
    recursive_clears: IntMap<EntityPathHash, Vec<TimeInt>>,
}

impl CachedTransformsForTimeline {
    fn new(timeline: &TimelineName, static_transforms: &Self) -> Self {
        Self {
            // It's crucial to take over any invalidated transform events from the static timeline,
            // otherwise we'd miss any pending static transforms that should ALSO be added to this timeline.
            invalidated_transforms: static_transforms.invalidated_transforms.clone(),
            per_entity: static_transforms
                .per_entity
                .iter()
                .map(|(entity_path, static_transforms)| {
                    (
                        entity_path.clone(),
                        TransformsForEntity::new_for_new_empty_timeline(
                            *timeline,
                            static_transforms,
                        ),
                    )
                })
                .collect(),
            recursive_clears: IntMap::default(),
        }
    }

    fn add_recursive_clears(&mut self, entity_path: &EntityPath, times: Vec<TimeInt>) {
        // Insert clears for all entities down the known tree.
        // (clears _at_ the entity stored & processed separately by invalidating the entity's transforms)
        for (entity, transforms) in &mut self.per_entity {
            if entity.is_descendant_of(entity_path) {
                transforms.add_clears(&times);
            }
        }

        // Store for future reference.
        self.recursive_clears
            .entry(entity_path.hash())
            .or_default()
            .extend(times);
    }
}

/// Maps from archetype to resolved pose transform.
///
/// If there's a concrete archetype in here, the mapped values are the full resolved pose transform.
/// Otherwise, refer to [`Self::instance_poses_archetype`].
///
/// `TransformCache` doesn't do tree propgation, however (!!!) there's a mini-tree in here that we already fully apply:
/// `InstancePose3D` are applied on top of concrete archetype poses.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PoseTransformArchetypeMap {
    /// Iff there's a concrete archetype in here, the mapped values are the full resolved pose transform.
    /// Otherwise, refer to [`Self::instance_poses_archetype`].
    // TODO(andreas): use some kind of small map? Vec of tuples might already be more appropriate?
    pub instance_from_archetype_poses_per_archetype:
        IntMap<ArchetypeName, SmallVec1<[Affine3A; 1]>>,

    /// Resolved transforms for the instance poses archetype if any.
    pub instance_from_overall_poses: Vec<Affine3A>,
}

impl PoseTransformArchetypeMap {
    #[cfg(test)]
    #[inline]
    fn get(&self, archetype: ArchetypeName) -> &[Affine3A] {
        self.instance_from_archetype_poses_per_archetype
            .get(&archetype)
            .map_or(&self.instance_from_overall_poses, |v| v.as_slice())
    }

    /// Returns `true` if there are no transforms for any archetype.
    #[inline]
    fn is_empty(&self) -> bool {
        self.instance_from_archetype_poses_per_archetype.is_empty()
            && self.instance_from_overall_poses.is_empty()
    }
}

type PoseTransformTimeMap = BTreeMap<TimeInt, PoseTransformArchetypeMap>;

/// Maps from time to pinhole projection.
///
/// Unlike with tree & pose transforms, there's no identity value that we can insert upon clears.
/// (clears here meaning that the user first logs a pinhole and then later either logs a clear or an empty pinhole array)
/// Therefore, we instead store those events as `None` values to ensure that everything after a clear
/// is properly marked as having no pinhole projection.
type PinholeProjectionMap = BTreeMap<TimeInt, Option<ResolvedPinholeProjection>>;

/// Cached transforms for a single entity.
///
/// Incorporates any static transforms that may apply to this entity.
#[derive(Clone, Debug, PartialEq)]
pub struct TransformsForEntity {
    // Is None if this is about the "static timeline".
    #[cfg(debug_assertions)]
    timeline: Option<TimelineName>,

    tree_transforms: BTreeMap<TimeInt, Affine3A>,

    // Pose transforms and pinhole projections are typically more rare, which is why we store them as optional boxes.
    pose_transforms: Option<Box<PoseTransformTimeMap>>,
    pinhole_projections: Option<Box<PinholeProjectionMap>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPinholeProjection {
    pub image_from_camera: components::PinholeProjection,

    /// View coordinates at this pinhole camera.
    ///
    /// This is needed to orient 2D in 3D and 3D in 2D the right way around
    /// (answering questions like which axis is distance to viewer increasing).
    /// If no view coordinates were logged, this is set to [`archetypes::Pinhole::DEFAULT_CAMERA_XYZ`].
    pub view_coordinates: components::ViewCoordinates,
}

impl CachedTransformsForTimeline {
    #[inline]
    pub fn entity_transforms(&self, entity_path: &EntityPath) -> Option<&TransformsForEntity> {
        self.per_entity.get(entity_path)
    }
}

impl TransformsForEntity {
    fn new(
        entity_path: &EntityPath,
        _timeline: TimelineName,
        recursive_clears: &IntMap<EntityPathHash, Vec<TimeInt>>,
        static_timeline: &CachedTransformsForTimeline,
    ) -> Self {
        let mut tree_transforms = BTreeMap::new();
        let mut pose_transforms = None;
        let mut pinhole_projections = None;

        if let Some(static_transforms) = static_timeline.per_entity.get(entity_path) {
            tree_transforms = static_transforms.tree_transforms.clone();
            pose_transforms = static_transforms.pose_transforms.clone();
            pinhole_projections = static_transforms.pinhole_projections.clone();
        }

        let mut result = Self {
            #[cfg(debug_assertions)]
            timeline: Some(_timeline),
            pose_transforms,
            tree_transforms,
            pinhole_projections,
        };

        // Gather all times at which this entity is being cleared by one of its parent's recursive clears.
        let mut all_clear_times: Vec<TimeInt> = Vec::new();
        let mut current_entity = entity_path.clone();
        while let Some(parent_entity_path) = current_entity.parent() {
            if let Some(clear_times) = recursive_clears.get(&parent_entity_path.hash()) {
                all_clear_times.extend(clear_times.iter());
            }
            current_entity = parent_entity_path;
        }
        result.add_clears(&all_clear_times);

        result
    }

    fn new_for_new_empty_timeline(_timeline: TimelineName, static_timeline_entry: &Self) -> Self {
        Self {
            #[cfg(debug_assertions)]
            timeline: Some(_timeline),
            pose_transforms: static_timeline_entry.pose_transforms.clone(),
            tree_transforms: static_timeline_entry.tree_transforms.clone(),
            pinhole_projections: static_timeline_entry.pinhole_projections.clone(),
        }
    }

    fn new_static() -> Self {
        Self {
            #[cfg(debug_assertions)]
            timeline: None,
            tree_transforms: BTreeMap::new(),
            pose_transforms: None,
            pinhole_projections: None,
        }
    }

    pub fn add_clears(&mut self, times: &[TimeInt]) {
        if times.is_empty() {
            return;
        }

        self.tree_transforms
            .extend(times.iter().map(|time| (*time, Affine3A::IDENTITY)));
        self.pose_transforms
            .get_or_insert(Default::default())
            .extend(
                times
                    .iter()
                    .map(|time| (*time, PoseTransformArchetypeMap::default())),
            );
        self.pinhole_projections
            .get_or_insert(Default::default())
            .extend(times.iter().map(|time| (*time, None)));
    }

    #[inline]
    pub fn latest_at_tree_transform(&self, query: &LatestAtQuery) -> Affine3A {
        #[cfg(debug_assertions)] // `self.timeline` is only present with `debug_assertions` enabled.
        debug_assert!(Some(query.timeline()) == self.timeline || self.timeline.is_none());

        self.tree_transforms
            .range(..query.at().inc())
            .next_back()
            .map(|(_time, transform)| *transform)
            .unwrap_or(Affine3A::IDENTITY)
    }

    #[cfg(test)]
    #[inline]
    pub fn latest_at_instance_poses(
        &self,
        query: &LatestAtQuery,
        archetype: ArchetypeName,
    ) -> &[Affine3A] {
        #[cfg(debug_assertions)] // `self.timeline` is only present with `debug_assertions` enabled.
        debug_assert!(Some(query.timeline()) == self.timeline || self.timeline.is_none());

        self.pose_transforms
            .as_ref()
            .and_then(|pose_transforms| pose_transforms.range(..query.at().inc()).next_back())
            .map(|(_time, pose_transforms)| pose_transforms.get(archetype))
            .unwrap_or(&[])
    }

    #[inline]
    pub fn latest_at_instance_poses_all(
        &self,
        query: &LatestAtQuery,
    ) -> Option<&PoseTransformArchetypeMap> {
        #[cfg(debug_assertions)] // `self.timeline` is only present with `debug_assertions` enabled.
        debug_assert!(Some(query.timeline()) == self.timeline || self.timeline.is_none());

        self.pose_transforms
            .as_ref()
            .and_then(|pose_transforms| pose_transforms.range(..query.at().inc()).next_back())
            .map(|(_time, pose_transforms)| pose_transforms)
    }

    #[inline]
    pub fn latest_at_pinhole(&self, query: &LatestAtQuery) -> Option<&ResolvedPinholeProjection> {
        #[cfg(debug_assertions)] // `self.timeline` is only present with `debug_assertions` enabled.
        debug_assert!(Some(query.timeline()) == self.timeline || self.timeline.is_none());

        self.pinhole_projections
            .as_ref()
            .and_then(|pinhole_projections| {
                pinhole_projections.range(..query.at().inc()).next_back()
            })
            .and_then(|(_time, projection)| projection.as_ref())
    }
}

impl TransformCacheStoreSubscriber {
    /// Accesses the global store subscriber.
    ///
    /// Lazily registers the subscriber if it hasn't been registered yet.
    pub fn subscription_handle() -> ChunkStoreSubscriberHandle {
        static SUBSCRIPTION: OnceCell<ChunkStoreSubscriberHandle> = OnceCell::new();
        *SUBSCRIPTION.get_or_init(ChunkStore::register_per_store_subscriber::<Self>)
    }

    /// Accesses the transform component tracking data for a given store.
    #[inline]
    pub fn access<T>(store_id: &StoreId, f: impl FnMut(&Self) -> T) -> Option<T> {
        ChunkStore::with_per_store_subscriber(Self::subscription_handle(), store_id, f)
    }

    /// Accesses the transform component tracking data for a given store exclusively.
    #[inline]
    pub fn access_mut<T>(store_id: &StoreId, f: impl FnMut(&mut Self) -> T) -> Option<T> {
        ChunkStore::with_per_store_subscriber_mut(Self::subscription_handle(), store_id, f)
    }

    /// Accesses the transform component tracking data for a given timeline.
    ///
    /// Returns `None` if the timeline doesn't have any transforms at all.
    #[inline]
    pub fn transforms_for_timeline(&self, timeline: TimelineName) -> &CachedTransformsForTimeline {
        self.per_timeline
            .get(&timeline)
            .unwrap_or(&self.static_timeline)
    }

    /// Makes sure the transform cache is up to date with the latest data.
    ///
    /// This needs to be called once per frame prior to any transform propagation.
    /// (which is done by [`crate::contexts::TransformTreeContext`])
    // TODO(andreas): easy optimization: apply only updates for a single timeline at a time.
    pub fn apply_all_updates(&mut self, entity_db: &EntityDb) {
        re_tracing::profile_function!();

        // Update static transforms.
        for invalidated_transform in self.static_timeline.invalidated_transforms.drain(..) {
            let InvalidatedTransforms {
                entity_path,
                aspects,
                ..
            } = invalidated_transform;

            let static_transforms = self
                .static_timeline
                .per_entity
                .entry(entity_path.clone())
                // There have never been any static or non-static transforms for this entity, therefore there's nothing to pass
                // into that entity for static transforms.
                .or_insert_with(TransformsForEntity::new_static);

            // Technically this doesn't query static components but rather just what's at the beginning of this arbitrary timeline,
            // but it's the most convenient way to the data we want.
            let query = LatestAtQuery::new(
                TimelineName::new(
                    "placeholder timeline (only actually interested in static components)",
                ),
                TimeInt::MIN,
            );

            if aspects.contains(TransformAspect::Tree) {
                if let Some(transform) =
                    query_and_resolve_tree_transform_at_entity(&entity_path, entity_db, &query)
                {
                    static_transforms
                        .tree_transforms
                        .insert(TimeInt::STATIC, transform);
                }
            }
            if aspects.contains(TransformAspect::Pose) {
                let poses =
                    query_and_resolve_instance_poses_at_entity(&entity_path, entity_db, &query);
                if !poses.is_empty() {
                    static_transforms.pose_transforms =
                        Some(Box::new(BTreeMap::from([(TimeInt::STATIC, poses)])));
                }
            }
            if aspects.contains(TransformAspect::PinholeOrViewCoordinates) {
                let pinhole_projection =
                    query_and_resolve_pinhole_projection_at_entity(&entity_path, entity_db, &query);
                if let Some(pinhole_projection) = pinhole_projection {
                    static_transforms.pinhole_projections = Some(Box::new(BTreeMap::from([(
                        TimeInt::STATIC,
                        Some(pinhole_projection),
                    )])));
                }
            }
        }

        // Update dynamic transforms.
        for (timeline, per_timeline) in &mut self.per_timeline {
            for invalidated_transform in per_timeline.invalidated_transforms.drain(..) {
                let InvalidatedTransforms {
                    entity_path,
                    aspects,
                    times,
                } = invalidated_transform;

                let entity_entry = per_timeline
                    .per_entity
                    .entry(entity_path.clone())
                    .or_insert_with(|| {
                        TransformsForEntity::new(
                            &entity_path,
                            *timeline,
                            &per_timeline.recursive_clears,
                            &self.static_timeline,
                        )
                    });

                for time in times {
                    let query = LatestAtQuery::new(*timeline, time);
                    if aspects.intersects(TransformAspect::Tree | TransformAspect::Clear) {
                        let transform = query_and_resolve_tree_transform_at_entity(
                            &entity_path,
                            entity_db,
                            &query,
                        )
                        .unwrap_or(Affine3A::IDENTITY);
                        // If there's *no* transform, we have to put identity in, otherwise we'd miss clears!
                        entity_entry.tree_transforms.insert(time, transform);
                    }
                    if aspects.intersects(TransformAspect::Pose | TransformAspect::Clear) {
                        let poses = query_and_resolve_instance_poses_at_entity(
                            &entity_path,
                            entity_db,
                            &query,
                        );
                        // *do* also insert empty ones, otherwise it's not possible to clear previous state.
                        entity_entry
                            .pose_transforms
                            .get_or_insert_with(Box::default)
                            .insert(time, poses);
                    }
                    if aspects.intersects(
                        TransformAspect::PinholeOrViewCoordinates | TransformAspect::Clear,
                    ) {
                        let pinhole_projection = query_and_resolve_pinhole_projection_at_entity(
                            &entity_path,
                            entity_db,
                            &query,
                        );
                        // `None` values need to be inserted as well to clear out previous state.
                        // See also doc string on `PinholeProjectionMap`.
                        entity_entry
                            .pinhole_projections
                            .get_or_insert_with(Box::default)
                            .insert(time, pinhole_projection);
                    }
                }
            }
        }
    }

    fn add_temporal_chunk(
        &mut self,
        event: &re_chunk_store::ChunkStoreEvent,
        aspects: TransformAspect,
    ) {
        re_tracing::profile_function!();

        let chunk = &event.diff.chunk;
        debug_assert!(!chunk.is_static());

        let entity_path = chunk.entity_path();

        for (timeline, time_column) in chunk.timelines() {
            let per_timeline = self.per_timeline.entry(*timeline).or_insert_with(|| {
                CachedTransformsForTimeline::new(timeline, &self.static_timeline)
            });

            // All of these require complex latest-at queries that would require a lot more context,
            // are fairly expensive, and may depend on other components that may come in at the same time.
            // (we could inject that here, but it's not entirely straight forward).
            // So instead, we note down that the caches are invalidated for the given entity & times.

            // This invalidates any time _after_ the first event in this chunk.
            // (e.g. if a rotation is added prior to translations later on,
            // then the resulting transforms at those translations change as well for latest-at queries)

            let mut invalidated_times = Vec::new();

            // Min time is conservative - technically we want to check this for each component individually,
            // but using the same for all is fine as it rarely matters.
            // (it may produce some false positive transform updates)
            let Some(min_time) = time_column.times().min() else {
                continue;
            };
            if let Some(entity_entry) = per_timeline.per_entity.get_mut(entity_path) {
                if aspects.intersects(TransformAspect::Tree | TransformAspect::Clear) {
                    let invalidated_tree_transforms =
                        entity_entry.tree_transforms.split_off(&min_time);
                    invalidated_times.extend(invalidated_tree_transforms.into_keys());
                }
                if aspects.intersects(TransformAspect::Pose | TransformAspect::Clear) {
                    if let Some(pose_transforms) = &mut entity_entry.pose_transforms {
                        let invalidated_pose_transforms = pose_transforms.split_off(&min_time);
                        invalidated_times.extend(invalidated_pose_transforms.into_keys());
                    }
                }
                if aspects
                    .intersects(TransformAspect::PinholeOrViewCoordinates | TransformAspect::Clear)
                {
                    if let Some(pinhole_projections) = &mut entity_entry.pinhole_projections {
                        let invalidated_pinhole_projections =
                            pinhole_projections.split_off(&min_time);
                        invalidated_times.extend(invalidated_pinhole_projections.into_keys());
                    }
                }
            }

            if aspects.contains(TransformAspect::Clear) {
                re_tracing::profile_scope!("check for recursive clears");

                let descr = re_types::archetypes::Clear::descriptor_is_recursive();

                let recursively_cleared_times = chunk
                    .iter_component_indices(timeline, &descr)
                    .zip(chunk.iter_slices::<bool>(descr.clone()))
                    .filter_map(|((time, _row_id), bool_slice)| {
                        bool_slice
                            .values()
                            .first()
                            .and_then(|is_recursive| (*is_recursive != 0).then_some(time))
                    })
                    .collect::<Vec<_>>();

                if !recursively_cleared_times.is_empty() {
                    per_timeline.add_recursive_clears(entity_path, recursively_cleared_times);
                }
            }

            let times = time_column
                .times()
                .chain(invalidated_times.into_iter())
                .collect();

            per_timeline
                .invalidated_transforms
                .push(InvalidatedTransforms {
                    entity_path: entity_path.clone(),
                    times,
                    aspects,
                });
        }
    }

    fn add_static_chunk(
        &mut self,
        event: &re_chunk_store::ChunkStoreEvent,
        aspects: TransformAspect,
    ) {
        re_tracing::profile_function!();

        debug_assert!(event.diff.chunk.is_static());

        let entity_path = event.chunk.entity_path();

        self.static_timeline
            .invalidated_transforms
            .push(InvalidatedTransforms {
                entity_path: entity_path.clone(),
                times: vec![TimeInt::STATIC],
                aspects,
            });

        // Adding a static transform invalidates ALL times for this entity on ALL timelines, since the resulting transforms at all times may be different now.
        // Furthermore, since we want to incorporate the static transforms into all timelines, we have to add this event to all timelines.
        for (timeline, per_timeline_transforms) in &mut self.per_timeline {
            let entity_transforms = per_timeline_transforms
                .per_entity
                .entry(entity_path.clone())
                .or_insert_with(|| {
                    // Need to add an entry now if there wasn't one before.
                    // Also note that the static transforms we use to construct this might touch on aspects that aren't invalidated, so it's still important to pass that in.
                    TransformsForEntity::new(
                        entity_path,
                        *timeline,
                        &per_timeline_transforms.recursive_clears,
                        &self.static_timeline,
                    )
                });
            if aspects.contains(TransformAspect::Tree) {
                per_timeline_transforms
                    .invalidated_transforms
                    .push(InvalidatedTransforms {
                        entity_path: entity_path.clone(),
                        times: std::iter::once(TimeInt::STATIC)
                            .chain(entity_transforms.tree_transforms.keys().copied())
                            .collect(),
                        aspects,
                    });
            }
            if aspects.contains(TransformAspect::Pose) {
                let mut times = vec![TimeInt::STATIC];
                if let Some(pose_transforms) = &entity_transforms.pose_transforms {
                    times.extend(pose_transforms.keys().copied());
                }

                per_timeline_transforms
                    .invalidated_transforms
                    .push(InvalidatedTransforms {
                        entity_path: entity_path.clone(),
                        times,
                        aspects,
                    });
            }
            if aspects.contains(TransformAspect::PinholeOrViewCoordinates) {
                let mut times = vec![TimeInt::STATIC];
                if let Some(pinhole_projections) = &entity_transforms.pinhole_projections {
                    times.extend(pinhole_projections.keys().copied());
                }

                per_timeline_transforms
                    .invalidated_transforms
                    .push(InvalidatedTransforms {
                        entity_path: entity_path.clone(),
                        times,
                        aspects,
                    });
            }
            // Don't care about clears here, they don't have any effect for keeping track of changes when logged static.
        }
    }

    fn remove_chunk(&mut self, event: &re_chunk_store::ChunkStoreEvent, aspects: TransformAspect) {
        re_tracing::profile_function!();

        let entity_path = event.chunk.entity_path();

        // Note that we ignore static timelines for removal.
        for (timeline, time_column) in event.diff.chunk.timelines() {
            let Some(per_timeline) = self.per_timeline.get_mut(timeline) else {
                continue;
            };

            // Remove incoming data.
            for invalidated_transform in per_timeline
                .invalidated_transforms
                .iter_mut()
                .filter(|invalidated_transform| &invalidated_transform.entity_path == entity_path)
            {
                let times = time_column.times().collect::<HashSet<_>>();
                invalidated_transform
                    .times
                    .retain(|time| !times.contains(time));
            }
            per_timeline
                .invalidated_transforms
                .retain(|invalidated_transform| !invalidated_transform.times.is_empty());

            // Remove existing data.
            if let Some(per_entity) = per_timeline.per_entity.get_mut(entity_path) {
                for time in time_column.times() {
                    if aspects.contains(TransformAspect::Tree) {
                        per_entity.tree_transforms.remove(&time);
                    }
                    if aspects.contains(TransformAspect::Pose) {
                        if let Some(pose_transforms) = &mut per_entity.pose_transforms {
                            pose_transforms.remove(&time);
                        }
                    }
                    if aspects.contains(TransformAspect::PinholeOrViewCoordinates) {
                        if let Some(pinhole_projections) = &mut per_entity.pinhole_projections {
                            pinhole_projections.remove(&time);
                        }
                    }
                }

                if per_entity.tree_transforms.is_empty()
                    && per_entity
                        .pose_transforms
                        .as_ref()
                        .is_none_or(|pose_transforms| pose_transforms.is_empty())
                    && per_entity
                        .pinhole_projections
                        .as_ref()
                        .is_none_or(|pinhole_projections| pinhole_projections.is_empty())
                {
                    per_timeline.per_entity.remove(entity_path);
                }
            }

            if per_timeline.per_entity.is_empty() && per_timeline.invalidated_transforms.is_empty()
            {
                self.per_timeline.remove(timeline);
            }
        }
    }
}

impl PerStoreChunkSubscriber for TransformCacheStoreSubscriber {
    fn name() -> String {
        "rerun.TransformCacheStoreSubscriber".to_owned()
    }

    fn on_events<'a>(&mut self, events: impl Iterator<Item = &'a re_chunk_store::ChunkStoreEvent>) {
        re_tracing::profile_function!();

        for event in events {
            // The components we are interested in may only show up on some of the timelines
            // within this chunk, so strictly speaking the affected "aspects" we compute here are conservative.
            // But that's fairly rare, so a few false positive entries here are fine.
            let mut aspects = TransformAspect::empty();
            for component_name in event.chunk.component_names() {
                if self.transform_components.contains(&component_name) {
                    aspects |= TransformAspect::Tree;
                }
                if self.pose_components.contains(&component_name) {
                    aspects |= TransformAspect::Pose;
                }
                if self.pinhole_components.contains(&component_name) {
                    aspects |= TransformAspect::PinholeOrViewCoordinates;
                }
                if component_name == re_types::components::ClearIsRecursive::name() {
                    aspects |= TransformAspect::Clear;
                }
            }
            if aspects.is_empty() {
                continue;
            }

            if event.kind == re_chunk_store::ChunkStoreDiffKind::Deletion {
                self.remove_chunk(event, aspects);
            } else if event.diff.chunk.is_static() {
                self.add_static_chunk(event, aspects);
            } else {
                self.add_temporal_chunk(event, aspects);
            }
        }
    }
}

/// Queries all components that are part of pose transforms, returning the transform from child to parent.
///
/// If any of the components yields an invalid transform, returns a `glam::Affine3A::ZERO`.
/// (this effectively disconnects a subtree from the transform hierarchy!)
// TODO(#3849): There's no way to discover invalid transforms right now (they can be intentional but often aren't).
fn query_and_resolve_tree_transform_at_entity(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> Option<Affine3A> {
    // TODO(andreas): Filter out styling components.
    let results = entity_db.latest_at(
        query,
        entity_path,
        archetypes::Transform3D::all_components().iter(),
    );
    if results.components.is_empty() {
        return None;
    }

    let mut transform = Affine3A::IDENTITY;

    // It's an error if there's more than one component. Warn in that case.
    let mono_log_level = re_log::Level::Warn;

    // The order of the components here is important, and checked by `debug_assert_transform_field_order`
    if let Some(translation) =
        results.component_mono_with_log_level_by_name::<components::Translation3D>(mono_log_level)
    {
        transform = Affine3A::from(translation);
    }
    if let Some(axis_angle) = results
        .component_mono_with_log_level_by_name::<components::RotationAxisAngle>(mono_log_level)
    {
        if let Ok(axis_angle) = Affine3A::try_from(axis_angle) {
            transform *= axis_angle;
        } else {
            return Some(Affine3A::ZERO);
        }
    }
    if let Some(quaternion) =
        results.component_mono_with_log_level_by_name::<components::RotationQuat>(mono_log_level)
    {
        if let Ok(quaternion) = Affine3A::try_from(quaternion) {
            transform *= quaternion;
        } else {
            return Some(Affine3A::ZERO);
        }
    }
    if let Some(scale) =
        results.component_mono_with_log_level_by_name::<components::Scale3D>(mono_log_level)
    {
        if scale.x() == 0.0 && scale.y() == 0.0 && scale.z() == 0.0 {
            return Some(Affine3A::ZERO);
        }
        transform *= Affine3A::from(scale);
    }
    if let Some(mat3x3) =
        results.component_mono_with_log_level_by_name::<components::TransformMat3x3>(mono_log_level)
    {
        let affine_transform = Affine3A::from(mat3x3);
        if affine_transform.matrix3.determinant() == 0.0 {
            return Some(Affine3A::ZERO);
        }
        transform *= affine_transform;
    }

    if results
        .component_mono_with_log_level_by_name::<components::TransformRelation>(mono_log_level)
        == Some(components::TransformRelation::ChildFromParent)
    {
        let determinant = transform.matrix3.determinant();
        if determinant != 0.0 && determinant.is_finite() {
            transform = transform.inverse();
        } else {
            // All "regular invalid" transforms should have been caught.
            // So ending up here means something else went wrong?
            re_log::warn_once!(
                "Failed to express child-from-parent transform at {} since it wasn't invertible",
                entity_path,
            );
        }
    }

    Some(transform)
}

/// Lists all archetypes except [`archetypes::InstancePoses3D`] that have their own instance poses.
// TODO(andreas, jleibs): Model this out as a generic extension mechanism.
fn archetypes_with_instance_pose_transforms_and_translation_descriptor()
-> [(ArchetypeName, ComponentDescriptor); 3] {
    [
        (
            archetypes::Boxes3D::name(),
            archetypes::Boxes3D::descriptor_centers(),
        ),
        (
            archetypes::Ellipsoids3D::name(),
            archetypes::Ellipsoids3D::descriptor_centers(),
        ),
        (
            archetypes::Capsules3D::name(),
            archetypes::Capsules3D::descriptor_translations(),
        ),
    ]
}

/// Queries all components that are part of pose transforms, returning the transform from child to parent.
///
/// If any of the components yields an invalid transform, returns a `glam::Affine3A::ZERO` for that instance.
/// (this effectively ignores the instance for most visualizations!)
// TODO(#3849): There's no way to discover invalid transforms right now (they can be intentional but often aren't).
fn query_and_resolve_instance_poses_at_entity(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> PoseTransformArchetypeMap {
    let instance_from_overall_poses = query_and_resolve_instance_from_pose_for_archetype_name(
        entity_path,
        entity_db,
        query,
        archetypes::InstancePoses3D::name(),
        &archetypes::InstancePoses3D::descriptor_translations(),
    );

    // Some archetypes support their own instance poses.
    // TODO(andreas): can we quickly determine whether this is necessary for any given archetype?
    // TODO(andreas): Should we make all of this a single large query?
    let mut instance_from_archetype_poses_per_archetype = IntMap::default();
    for (archetype_name, descriptor_translations) in
        archetypes_with_instance_pose_transforms_and_translation_descriptor()
    {
        if let Ok(mut instance_from_archetype_poses) =
            SmallVec1::try_from_vec(query_and_resolve_instance_from_pose_for_archetype_name(
                entity_path,
                entity_db,
                query,
                archetype_name,
                &descriptor_translations,
            ))
        {
            // "zip" up with the overall poses.
            let length = instance_from_archetype_poses
                .len()
                .max(instance_from_overall_poses.len());
            instance_from_archetype_poses
                .resize(length, *instance_from_archetype_poses.last()) // Components repeat.
                .expect("Overall number of poses can't be zero.");

            for (instance_from_archetype_pose, instance_from_overall_pose) in
                instance_from_archetype_poses
                    .iter_mut()
                    .zip(instance_from_overall_poses.iter())
            {
                let overall_pose_archetype_pose = *instance_from_archetype_pose;
                *instance_from_archetype_pose =
                    (*instance_from_overall_pose) * overall_pose_archetype_pose;
            }

            instance_from_archetype_poses_per_archetype
                .insert(archetype_name, instance_from_archetype_poses);
        }
    }

    PoseTransformArchetypeMap {
        instance_from_archetype_poses_per_archetype,
        instance_from_overall_poses,
    }
}

/// Queries pose transforms for a specific archetype.
///
/// Note that the archetype field name for translation specifically may vary.
/// (this is technical debt, we should fix this)
fn query_and_resolve_instance_from_pose_for_archetype_name(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
    archetype_name: ArchetypeName,
    descriptor_translations: &ComponentDescriptor,
) -> Vec<Affine3A> {
    debug_assert_eq!(
        descriptor_translations.component_name,
        components::PoseTranslation3D::name()
    );
    debug_assert_eq!(descriptor_translations.archetype_name, Some(archetype_name));
    let descriptor_rotation_axis_angles =
        InstancePoses3D::descriptor_rotation_axis_angles().with_archetype_name(archetype_name);
    let descriptor_quaternions =
        InstancePoses3D::descriptor_quaternions().with_archetype_name(archetype_name);
    let descriptor_scales =
        InstancePoses3D::descriptor_scales().with_archetype_name(archetype_name);
    let descriptor_mat3x3 =
        InstancePoses3D::descriptor_mat3x3().with_archetype_name(archetype_name);

    let result = entity_db.latest_at(
        query,
        entity_path,
        [
            descriptor_translations,
            &descriptor_rotation_axis_angles,
            &descriptor_quaternions,
            &descriptor_scales,
            &descriptor_mat3x3,
        ],
    );

    let max_num_instances = result
        .components
        .iter()
        .map(|(component_descr, row)| row.num_instances(component_descr))
        .max()
        .unwrap_or(0) as usize;

    if max_num_instances == 0 {
        return Vec::new();
    }

    #[inline]
    pub fn clamped_or_nothing<T: Clone>(
        values: Vec<T>,
        clamped_len: usize,
    ) -> impl Iterator<Item = T> {
        let Some(last) = values.last() else {
            return Either::Left(std::iter::empty());
        };
        let last = last.clone();
        Either::Right(
            values
                .into_iter()
                .chain(std::iter::repeat(last))
                .take(clamped_len),
        )
    }

    let batch_translation = result
        .component_batch_by_name::<components::PoseTranslation3D>()
        .unwrap_or_default();
    let batch_rotation_quat = result
        .component_batch_by_name::<components::PoseRotationQuat>()
        .unwrap_or_default();
    let batch_rotation_axis_angle = result
        .component_batch_by_name::<components::PoseRotationAxisAngle>()
        .unwrap_or_default();
    let batch_scale = result
        .component_batch_by_name::<components::PoseScale3D>()
        .unwrap_or_default();
    let batch_mat3x3 = result
        .component_batch_by_name::<components::PoseTransformMat3x3>()
        .unwrap_or_default();

    if batch_translation.is_empty()
        && batch_rotation_quat.is_empty()
        && batch_rotation_axis_angle.is_empty()
        && batch_scale.is_empty()
        && batch_mat3x3.is_empty()
    {
        return Vec::new();
    }
    let mut iter_translation = clamped_or_nothing(batch_translation, max_num_instances);
    let mut iter_rotation_quat = clamped_or_nothing(batch_rotation_quat, max_num_instances);
    let mut iter_rotation_axis_angle =
        clamped_or_nothing(batch_rotation_axis_angle, max_num_instances);
    let mut iter_scale = clamped_or_nothing(batch_scale, max_num_instances);
    let mut iter_mat3x3 = clamped_or_nothing(batch_mat3x3, max_num_instances);

    (0..max_num_instances)
        .map(|_| {
            // We apply these in a specific order - see `debug_assert_transform_field_order`
            let mut transform = Affine3A::IDENTITY;
            if let Some(translation) = iter_translation.next() {
                transform = Affine3A::from(translation);
            }
            if let Some(rotation_quat) = iter_rotation_quat.next() {
                if let Ok(rotation_quat) = Affine3A::try_from(rotation_quat) {
                    transform *= rotation_quat;
                } else {
                    transform = Affine3A::ZERO;
                }
            }
            if let Some(rotation_axis_angle) = iter_rotation_axis_angle.next() {
                if let Ok(axis_angle) = Affine3A::try_from(rotation_axis_angle) {
                    transform *= axis_angle;
                } else {
                    transform = Affine3A::ZERO;
                }
            }
            if let Some(scale) = iter_scale.next() {
                transform *= Affine3A::from(scale);
            }
            if let Some(mat3x3) = iter_mat3x3.next() {
                transform *= Affine3A::from(mat3x3);
            }
            transform
        })
        .collect()
}

fn query_and_resolve_pinhole_projection_at_entity(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> Option<ResolvedPinholeProjection> {
    entity_db
        .latest_at_component::<components::PinholeProjection>(
            entity_path,
            query,
            &archetypes::Pinhole::descriptor_image_from_camera(),
        )
        .map(|(_index, image_from_camera)| ResolvedPinholeProjection {
            image_from_camera,
            view_coordinates: {
                query_view_coordinates(entity_path, entity_db, query)
                    .unwrap_or(archetypes::Pinhole::DEFAULT_CAMERA_XYZ)
            },
        })
}

/// Queries view coordinates from either the [`archetypes::Pinhole`] or [`archetypes::ViewCoordinates`] archetype.
///
/// Gives precedence to the `Pinhole` archetype.
// TODO(#9917): This is confusing and should be cleaned up.
pub fn query_view_coordinates(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> Option<components::ViewCoordinates> {
    entity_db
        .latest_at_component::<components::ViewCoordinates>(
            entity_path,
            query,
            &archetypes::Pinhole::descriptor_camera_xyz(),
        )
        .or_else(|| {
            entity_db.latest_at_component::<components::ViewCoordinates>(
                entity_path,
                query,
                &archetypes::ViewCoordinates::descriptor_xyz(),
            )
        })
        .map(|(_index, view_coordinates)| view_coordinates)
}

/// Queries view coordinates from either the [`archetypes::Pinhole`] or [`archetypes::ViewCoordinates`] archetype
/// at the closest ancestor of the given entity path.
///
/// Gives precedence to the `Pinhole` archetype.
// TODO(#9917): This is confusing and should be cleaned up.
pub fn query_view_coordinates_at_closest_ancestor(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> Option<components::ViewCoordinates> {
    entity_db
        .latest_at_component_at_closest_ancestor::<components::ViewCoordinates>(
            entity_path,
            query,
            &archetypes::Pinhole::descriptor_camera_xyz(),
        )
        .or_else(|| {
            entity_db.latest_at_component_at_closest_ancestor::<components::ViewCoordinates>(
                entity_path,
                query,
                &archetypes::ViewCoordinates::descriptor_xyz(),
            )
        })
        .map(|(_path, _index, view_coordinates)| view_coordinates)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk_store::{Chunk, GarbageCollectionOptions, RowId};
    use re_log_types::{TimePoint, Timeline};
    use re_types::{Loggable as _, SerializedComponentBatch, archetypes, datatypes};

    use super::*;

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

    fn apply_all_updates(entity_db: &EntityDb) {
        TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
            cache.apply_all_updates(entity_db);
        });
    }

    fn static_test_setup_store(
        prior_static_chunk: Chunk,
        final_static_chunk: Chunk,
        regular_chunk: Chunk,
        flavor: StaticTestFlavor,
    ) -> EntityDb {
        // Print the flavor to its shown on test failure.
        println!("{flavor:?}");

        let mut entity_db = new_entity_db_with_subscriber_registered();

        match flavor {
            StaticTestFlavor::StaticThenRegular { update_inbetween } => {
                entity_db.add_chunk(&Arc::new(final_static_chunk)).unwrap();
                if update_inbetween {
                    apply_all_updates(&entity_db);
                }
                entity_db.add_chunk(&Arc::new(regular_chunk)).unwrap();
            }

            StaticTestFlavor::RegularThenStatic { update_inbetween } => {
                entity_db.add_chunk(&Arc::new(regular_chunk)).unwrap();
                if update_inbetween {
                    apply_all_updates(&entity_db);
                }
                entity_db.add_chunk(&Arc::new(final_static_chunk)).unwrap();
            }

            StaticTestFlavor::PriorStaticThenRegularThenStatic { update_inbetween } => {
                entity_db.add_chunk(&Arc::new(prior_static_chunk)).unwrap();
                entity_db.add_chunk(&Arc::new(regular_chunk)).unwrap();
                if update_inbetween {
                    apply_all_updates(&entity_db);
                }
                entity_db.add_chunk(&Arc::new(final_static_chunk)).unwrap();
            }
        }

        entity_db
    }

    fn new_entity_db_with_subscriber_registered() -> EntityDb {
        let entity_db = EntityDb::new(StoreId::random(re_log_types::StoreKind::Recording));
        TransformCacheStoreSubscriber::access(&entity_db.store_id(), |_| {
            // Make sure the subscriber is registered.
        });
        entity_db
    }

    #[test]
    fn test_transforms_per_timeline_access() {
        let mut entity_db = new_entity_db_with_subscriber_registered();

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk0 = Chunk::builder(EntityPath::from("with_transform"))
            .with_archetype(
                RowId::new(),
                [(timeline, 1)],
                &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0]),
            )
            .build()
            .unwrap();
        let chunk1 = Chunk::builder(EntityPath::from("without_transform"))
            .with_archetype(
                RowId::new(),
                [(timeline, 1)],
                // Anything that doesn't have components the transform cache is interested in.
                &archetypes::Points3D::new([[1.0, 2.0, 3.0]]),
            )
            .build()
            .unwrap();
        entity_db.add_chunk(&Arc::new(chunk0)).unwrap();
        entity_db.add_chunk(&Arc::new(chunk1)).unwrap();

        TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
            cache.apply_all_updates(&entity_db);
            let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
            assert!(
                transforms_per_timeline
                    .entity_transforms(&EntityPath::from("without_transform"))
                    .is_none()
            );
            assert!(
                transforms_per_timeline
                    .entity_transforms(&EntityPath::from("rando"))
                    .is_none()
            );
            let transforms = transforms_per_timeline
                .entity_transforms(&EntityPath::from("with_transform"))
                .unwrap();
            assert_eq!(transforms.timeline, Some(*timeline.name()));
            assert_eq!(transforms.tree_transforms.len(), 1);
            assert_eq!(transforms.pose_transforms, None);
            assert_eq!(transforms.pinhole_projections, None);
        });
    }

    #[test]
    fn test_static_tree_transforms() {
        for flavor in &ALL_STATIC_TEST_FLAVOURS {
            // Log a few tree transforms at different times.
            let timeline = Timeline::new_sequence("t");
            let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    // Make sure only translation is logged (no null arrays for everything else).
                    &archetypes::Transform3D::update_fields()
                        .with_translation([123.0, 234.0, 345.0]),
                )
                .build()
                .unwrap();
            let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    // Make sure only translation is logged (no null arrays for everything else).
                    &archetypes::Transform3D::update_fields().with_translation([1.0, 2.0, 3.0]),
                )
                .build()
                .unwrap();
            let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype(
                    RowId::new(),
                    [(timeline, 1)],
                    &archetypes::Transform3D::update_fields().with_scale([123.0, 234.0, 345.0]),
                )
                .build()
                .unwrap();

            let entity_db = static_test_setup_store(
                prior_static_chunk,
                final_static_chunk,
                regular_chunk,
                *flavor,
            );

            // Check that the transform cache has the expected transforms.
            TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
                cache.apply_all_updates(&entity_db);
                let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
                let transforms = transforms_per_timeline
                    .entity_transforms(&EntityPath::from("my_entity"))
                    .unwrap();

                assert_eq!(
                    transforms.latest_at_tree_transform(&LatestAtQuery::new(
                        *timeline.name(),
                        TimeInt::MIN
                    )),
                    glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0))
                );
                assert_eq!(
                    transforms.latest_at_tree_transform(&LatestAtQuery::new(
                        *timeline.name(),
                        TimeInt::MIN
                    )),
                    transforms.latest_at_tree_transform(&LatestAtQuery::new(*timeline.name(), 0)),
                );
                assert_eq!(
                    transforms.latest_at_tree_transform(&LatestAtQuery::new(*timeline.name(), 1)),
                    glam::Affine3A::from_scale_rotation_translation(
                        glam::Vec3::new(123.0, 234.0, 345.0),
                        glam::Quat::IDENTITY,
                        glam::Vec3::new(1.0, 2.0, 3.0),
                    )
                );

                // Timelines that the cache has never seen should still have the static transform.
                let transforms_per_timeline =
                    cache.transforms_for_timeline(TimelineName::new("other"));
                let transforms = transforms_per_timeline
                    .entity_transforms(&EntityPath::from("my_entity"))
                    .unwrap();
                assert_eq!(
                    transforms.latest_at_tree_transform(&LatestAtQuery::new(
                        TimelineName::new("other"),
                        123
                    )),
                    glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0))
                );
            });
        }
    }

    #[test]
    fn test_static_pose_transforms() {
        for flavor in &ALL_STATIC_TEST_FLAVOURS {
            // Log a few tree transforms at different times.
            let timeline = Timeline::new_sequence("t");
            let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &archetypes::InstancePoses3D::new().with_translations([[321.0, 234.0, 345.0]]),
                )
                .build()
                .unwrap();
            let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &archetypes::InstancePoses3D::new()
                        .with_translations([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]),
                )
                .build()
                .unwrap();
            let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype(
                    RowId::new(),
                    [(timeline, 1)],
                    // Add a splatted scale.
                    &archetypes::InstancePoses3D::new().with_scales([[10.0, 20.0, 30.0]]),
                )
                .build()
                .unwrap();

            let entity_db = static_test_setup_store(
                prior_static_chunk,
                final_static_chunk,
                regular_chunk,
                *flavor,
            );

            // Check that the transform cache has the expected transforms.
            TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
                cache.apply_all_updates(&entity_db);
                let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
                let transforms = transforms_per_timeline
                    .entity_transforms(&EntityPath::from("my_entity"))
                    .unwrap();

                assert_eq!(
                    transforms.latest_at_instance_poses(
                        &LatestAtQuery::new(*timeline.name(), TimeInt::MIN,),
                        archetypes::InstancePoses3D::name()
                    ),
                    &[
                        glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                        glam::Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
                    ]
                );
                assert_eq!(
                    transforms.latest_at_instance_poses(
                        &LatestAtQuery::new(*timeline.name(), TimeInt::MIN,),
                        archetypes::InstancePoses3D::name()
                    ),
                    transforms.latest_at_instance_poses(
                        &LatestAtQuery::new(*timeline.name(), 0),
                        archetypes::InstancePoses3D::name(),
                    ),
                );
                assert_eq!(
                    transforms.latest_at_instance_poses(
                        &LatestAtQuery::new(*timeline.name(), 1),
                        archetypes::InstancePoses3D::name(),
                    ),
                    &[
                        glam::Affine3A::from_scale_rotation_translation(
                            glam::Vec3::new(10.0, 20.0, 30.0),
                            glam::Quat::IDENTITY,
                            glam::Vec3::new(1.0, 2.0, 3.0),
                        ),
                        glam::Affine3A::from_scale_rotation_translation(
                            glam::Vec3::new(10.0, 20.0, 30.0),
                            glam::Quat::IDENTITY,
                            glam::Vec3::new(4.0, 5.0, 6.0),
                        ),
                    ]
                );

                // Timelines that the cache has never seen should still have the static poses.
                let transforms_per_timeline =
                    cache.transforms_for_timeline(TimelineName::new("other"));
                let transforms = transforms_per_timeline
                    .entity_transforms(&EntityPath::from("my_entity"))
                    .unwrap();
                assert_eq!(
                    transforms.latest_at_instance_poses(
                        &LatestAtQuery::new(TimelineName::new("other"), 123),
                        archetypes::InstancePoses3D::name(),
                    ),
                    &[
                        glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                        glam::Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
                    ]
                );
            });
        }
    }

    #[test]
    fn test_static_pinhole_projection() {
        for flavor in &ALL_STATIC_TEST_FLAVOURS {
            let image_from_camera_prior =
                components::PinholeProjection::from_focal_length_and_principal_point(
                    [123.0, 123.0],
                    [123.0, 123.0],
                );
            let image_from_camera_final =
                components::PinholeProjection::from_focal_length_and_principal_point(
                    [1.0, 2.0],
                    [1.0, 2.0],
                );

            // Static pinhole, non-static view coordinates.
            let timeline = Timeline::new_sequence("t");
            let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &archetypes::Pinhole::new(image_from_camera_prior),
                )
                .build()
                .unwrap();
            let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &archetypes::Pinhole::new(image_from_camera_final),
                )
                .build()
                .unwrap();
            let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype(
                    RowId::new(),
                    [(timeline, 1)],
                    &archetypes::ViewCoordinates::BLU(),
                )
                .build()
                .unwrap();

            let entity_db = static_test_setup_store(
                prior_static_chunk,
                final_static_chunk,
                regular_chunk,
                *flavor,
            );

            // Check that the transform cache has the expected transforms.
            TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
                cache.apply_all_updates(&entity_db);
                let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
                let transforms = transforms_per_timeline
                    .entity_transforms(&EntityPath::from("my_entity"))
                    .unwrap();

                assert_eq!(
                    transforms
                        .latest_at_pinhole(&LatestAtQuery::new(*timeline.name(), TimeInt::MIN)),
                    Some(&ResolvedPinholeProjection {
                        image_from_camera: image_from_camera_final,
                        view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
                    })
                );
                assert_eq!(
                    transforms
                        .latest_at_pinhole(&LatestAtQuery::new(*timeline.name(), TimeInt::MIN)),
                    transforms.latest_at_pinhole(&LatestAtQuery::new(*timeline.name(), 0)),
                );
                assert_eq!(
                    transforms.latest_at_pinhole(&LatestAtQuery::new(*timeline.name(), 1)),
                    Some(&ResolvedPinholeProjection {
                        image_from_camera: image_from_camera_final,
                        view_coordinates: components::ViewCoordinates::BLU,
                    })
                );

                // Timelines that the cache has never seen should still have the static pinhole.
                let transforms_per_timeline =
                    cache.transforms_for_timeline(TimelineName::new("other"));
                let transforms = transforms_per_timeline
                    .entity_transforms(&EntityPath::from("my_entity"))
                    .unwrap();
                assert_eq!(
                    transforms.latest_at_pinhole(&LatestAtQuery::new(*timeline.name(), 123)),
                    Some(&ResolvedPinholeProjection {
                        image_from_camera: image_from_camera_final,
                        view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
                    })
                );
            });
        }
    }

    #[test]
    fn test_static_view_coordinates_projection() {
        for flavor in &ALL_STATIC_TEST_FLAVOURS {
            let image_from_camera =
                components::PinholeProjection::from_focal_length_and_principal_point(
                    [1.0, 2.0],
                    [1.0, 2.0],
                );

            // Static view coordinates, non-static pinhole.
            let timeline = Timeline::new_sequence("t");
            let prior_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &archetypes::ViewCoordinates::BRU(),
                )
                .build()
                .unwrap();
            let final_static_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &archetypes::ViewCoordinates::BLU(),
                )
                .build()
                .unwrap();
            let regular_chunk = Chunk::builder(EntityPath::from("my_entity"))
                .with_archetype(
                    RowId::new(),
                    [(timeline, 1)],
                    &archetypes::Pinhole::new(image_from_camera),
                )
                .build()
                .unwrap();

            let entity_db = static_test_setup_store(
                prior_static_chunk,
                final_static_chunk,
                regular_chunk,
                *flavor,
            );

            // Check that the transform cache has the expected transforms.
            TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
                cache.apply_all_updates(&entity_db);
                let transforms_per_timeline = cache.transforms_for_timeline(*timeline.name());
                let transforms = transforms_per_timeline
                    .entity_transforms(&EntityPath::from("my_entity"))
                    .unwrap();

                // There's view coordinates, but that doesn't show up.
                assert_eq!(
                    transforms
                        .latest_at_pinhole(&LatestAtQuery::new(*timeline.name(), TimeInt::MIN)),
                    None
                );
                assert_eq!(
                    transforms
                        .latest_at_pinhole(&LatestAtQuery::new(*timeline.name(), TimeInt::MIN)),
                    transforms.latest_at_pinhole(&LatestAtQuery::new(*timeline.name(), 0)),
                );
                // Once we get a pinhole camera, the view coordinates should be there.
                assert_eq!(
                    transforms.latest_at_pinhole(&LatestAtQuery::new(*timeline.name(), 1)),
                    Some(&ResolvedPinholeProjection {
                        image_from_camera,
                        view_coordinates: components::ViewCoordinates::BLU,
                    })
                );
            });
        }
    }

    #[test]
    fn test_tree_transforms() {
        let mut entity_db = new_entity_db_with_subscriber_registered();

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype(
                RowId::new(),
                [(timeline, 1)],
                &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0]),
            )
            .with_archetype(
                RowId::new(),
                [(timeline, 3)],
                &archetypes::Transform3D::update_fields().with_scale([1.0, 2.0, 3.0]),
            )
            .with_archetype(
                RowId::new(),
                [(timeline, 4)],
                &archetypes::Transform3D::from_rotation(glam::Quat::from_rotation_x(1.0)),
            )
            .with_archetype(
                RowId::new(),
                [(timeline, 5)],
                &archetypes::Transform3D::clear_fields(),
            )
            .build()
            .unwrap();
        entity_db.add_chunk(&Arc::new(chunk)).unwrap();

        // Check that the transform cache has the expected transforms.
        TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
            let timeline_name = *timeline.name();
            cache.apply_all_updates(&entity_db);
            let transforms_per_timeline = cache.transforms_for_timeline(timeline_name);
            let transforms = transforms_per_timeline
                .entity_transforms(&EntityPath::from("my_entity"))
                .unwrap();

            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline_name, 0)),
                glam::Affine3A::IDENTITY
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline_name, 1)),
                glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0))
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline_name, 2)),
                glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0))
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline_name, 3)),
                glam::Affine3A::from_scale_rotation_translation(
                    glam::Vec3::new(1.0, 2.0, 3.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(1.0, 2.0, 3.0),
                )
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline_name, 4)),
                glam::Affine3A::from_quat(glam::Quat::from_rotation_x(1.0))
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline_name, 5)),
                glam::Affine3A::IDENTITY
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline_name, 123)),
                glam::Affine3A::IDENTITY
            );
        });
    }

    #[test]
    fn test_pose_transforms_instance_poses_only() {
        let mut entity_db = new_entity_db_with_subscriber_registered();

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype(
                RowId::new(),
                [(timeline, 1)],
                &archetypes::InstancePoses3D::new().with_translations([
                    [1.0, 2.0, 3.0],
                    [4.0, 5.0, 6.0],
                    [7.0, 8.0, 9.0],
                ]),
            )
            .with_archetype(
                RowId::new(),
                [(timeline, 3)],
                // Less instances, and a splatted scale.
                &archetypes::InstancePoses3D::new()
                    .with_translations([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]])
                    .with_scales([[2.0, 3.0, 4.0]]),
            )
            .with_serialized_batches(
                RowId::new(),
                [(timeline, 4)],
                [
                    SerializedComponentBatch::new(
                        arrow::array::new_empty_array(&components::Translation3D::arrow_datatype()),
                        archetypes::InstancePoses3D::descriptor_translations(),
                    ),
                    SerializedComponentBatch::new(
                        arrow::array::new_empty_array(&components::Scale3D::arrow_datatype()),
                        archetypes::InstancePoses3D::descriptor_scales(),
                    ),
                ],
            )
            // TODO(#7245): Use this instead of the above
            // .with_archetype(
            //     RowId::new(),
            //     [(timeline, 4)],
            //     &archetypes::InstancePoses3D::clear_fields(),
            // )
            .build()
            .unwrap();
        entity_db.add_chunk(&Arc::new(chunk)).unwrap();

        // Check that the transform cache has the expected transforms.
        TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
            let timeline = *timeline.name();
            cache.apply_all_updates(&entity_db);
            let transforms_per_timeline = cache.transforms_for_timeline(timeline);
            let transforms = transforms_per_timeline
                .entity_transforms(&EntityPath::from("my_entity"))
                .unwrap();

            assert_eq!(
                transforms.latest_at_instance_poses(
                    &LatestAtQuery::new(timeline, 0),
                    archetypes::InstancePoses3D::name()
                ),
                &[]
            );
            assert_eq!(
                transforms.latest_at_instance_poses(
                    &LatestAtQuery::new(timeline, 1),
                    archetypes::InstancePoses3D::name()
                ),
                &[
                    glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                    glam::Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
                    glam::Affine3A::from_translation(glam::Vec3::new(7.0, 8.0, 9.0)),
                ]
            );
            assert_eq!(
                transforms.latest_at_instance_poses(
                    &LatestAtQuery::new(timeline, 2),
                    archetypes::InstancePoses3D::name()
                ),
                &[
                    glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                    glam::Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
                    glam::Affine3A::from_translation(glam::Vec3::new(7.0, 8.0, 9.0)),
                ]
            );
            assert_eq!(
                transforms.latest_at_instance_poses(
                    &LatestAtQuery::new(timeline, 3),
                    archetypes::InstancePoses3D::name()
                ),
                &[
                    glam::Affine3A::from_scale_rotation_translation(
                        glam::Vec3::new(2.0, 3.0, 4.0),
                        glam::Quat::IDENTITY,
                        glam::Vec3::new(1.0, 2.0, 3.0),
                    ),
                    glam::Affine3A::from_scale_rotation_translation(
                        glam::Vec3::new(2.0, 3.0, 4.0),
                        glam::Quat::IDENTITY,
                        glam::Vec3::new(4.0, 5.0, 6.0),
                    ),
                ]
            );
            assert_eq!(
                transforms.latest_at_instance_poses(
                    &LatestAtQuery::new(timeline, 4),
                    archetypes::InstancePoses3D::name()
                ),
                &[]
            );
            assert_eq!(
                transforms.latest_at_instance_poses(
                    &LatestAtQuery::new(timeline, 123),
                    archetypes::InstancePoses3D::name()
                ),
                &[]
            );
        });
    }

    #[test]
    fn test_mixing_instance_poses() {
        let mut entity_db = new_entity_db_with_subscriber_registered();

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype(
                RowId::new(),
                [(timeline, 1)],
                &archetypes::InstancePoses3D::new().with_translations([
                    [1.0, 2.0, 3.0],
                    [4.0, 5.0, 6.0],
                    [7.0, 8.0, 9.0],
                ]),
            )
            .with_archetype(
                RowId::new(),
                [(timeline, 2)],
                // Add some "base offset", but only for the first two items.
                &archetypes::Boxes3D::update_fields()
                    .with_centers([[10.0, 0.0, 0.0], [0.0, 100.0, 0.0]]),
            )
            .with_archetype(
                RowId::new(),
                [(timeline, 3)],
                // Rotate the box by 90 degrees around the Y axis.
                &archetypes::Boxes3D::update_fields().with_rotation_axis_angles([
                    datatypes::RotationAxisAngle::new(
                        glam::Vec3::new(0.0, 1.0, 0.0),
                        90.0_f32.to_radians(),
                    ),
                ]),
            )
            .build()
            .unwrap();
        entity_db.add_chunk(&Arc::new(chunk)).unwrap();

        // Check that the transform cache has the expected transforms.
        TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
            let timeline = *timeline.name();
            cache.apply_all_updates(&entity_db);
            let transforms_per_timeline = cache.transforms_for_timeline(timeline);
            let transforms = transforms_per_timeline
                .entity_transforms(&EntityPath::from("my_entity"))
                .unwrap();

            // Pose for instances poses and non-boxes are unchanged over time.
            for t in 1..=4 {
                for archetype in [
                    archetypes::InstancePoses3D::name(),
                    "made_up_archetype".into(),
                ] {
                    assert_eq!(
                        transforms
                            .latest_at_instance_poses(&LatestAtQuery::new(timeline, t), archetype),
                        &[
                            glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                            glam::Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
                            glam::Affine3A::from_translation(glam::Vec3::new(7.0, 8.0, 9.0)),
                        ]
                    );
                }
            }

            // Poses for boxes change over time.
            // T1
            assert_eq!(
                transforms.latest_at_instance_poses(
                    &LatestAtQuery::new(timeline, 1),
                    archetypes::Boxes3D::name()
                ),
                // All from `InstancePoses3D`
                &[
                    glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                    glam::Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
                    glam::Affine3A::from_translation(glam::Vec3::new(7.0, 8.0, 9.0)),
                ]
            );

            // T2
            assert_eq!(
                transforms.latest_at_instance_poses(
                    &LatestAtQuery::new(timeline, 2),
                    archetypes::Boxes3D::name()
                ),
                // All from `InstancePoses3D` combined with box centers.
                &[
                    glam::Affine3A::from_translation(glam::Vec3::new(11.0, 2.0, 3.0)),
                    glam::Affine3A::from_translation(glam::Vec3::new(4.0, 105.0, 6.0)),
                    glam::Affine3A::from_translation(glam::Vec3::new(7.0, 108.0, 9.0)), // Affected by the last box center which is still splatted.
                ]
            );

            // T3.
            let query_result = transforms.latest_at_instance_poses(
                &LatestAtQuery::new(timeline, 3),
                archetypes::Boxes3D::name(),
            );

            // More readable sanity check on translations which aren't affected by the rotation.
            assert_eq!(
                query_result[0].translation,
                glam::Vec3A::new(11.0, 2.0, 3.0)
            );
            // Since rotation isn't 100% accurate, we need to check for equality with a small tolerance.
            let eps = 0.000001;
            // Rotation on the first box affects all insteances since it's splatted.
            let rotation = glam::Affine3A::from_axis_angle(
                glam::Vec3::new(0.0, 1.0, 0.0),
                90.0_f32.to_radians(),
            );
            let expected = glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)) * // Pose
                            glam::Affine3A::from_translation(glam::Vec3::new(10.0, 0.0, 0.0)) * rotation; // Box
            assert!(
                query_result[0].abs_diff_eq(expected, eps),
                "Expected: {:?}\nGot: {:?}",
                expected,
                query_result[0]
            );
            let expected = glam::Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)) * // Pose
                            (glam::Affine3A::from_translation(glam::Vec3::new(0.0, 100.0, 0.0)) * rotation); // Box
            assert!(
                query_result[1].abs_diff_eq(expected, eps),
                "Expected: {:?}\nGot: {:?}",
                expected,
                query_result[1]
            );
            let expected = glam::Affine3A::from_translation(glam::Vec3::new(7.0, 8.0, 9.0)) * // Pose
                            (glam::Affine3A::from_translation(glam::Vec3::new(0.0, 100.0, 0.0)) * rotation); // Box
            assert!(
                query_result[2].abs_diff_eq(expected, eps),
                "Expected: {:?}\nGot: {:?}",
                expected,
                query_result[2]
            );
        });
    }

    #[test]
    fn test_pinhole_projections() {
        let mut entity_db = new_entity_db_with_subscriber_registered();

        let image_from_camera =
            components::PinholeProjection::from_focal_length_and_principal_point(
                [1.0, 2.0],
                [1.0, 2.0],
            );

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype(
                RowId::new(),
                [(timeline, 1)],
                &archetypes::Pinhole::new(image_from_camera),
            )
            .with_archetype(
                RowId::new(),
                [(timeline, 3)],
                &archetypes::ViewCoordinates::BLU(),
            )
            // Clear out the pinhole projection (this should yield nothing then for the remaining view coordinates.)
            .with_serialized_batch(
                RowId::new(),
                [(timeline, 4)],
                SerializedComponentBatch::new(
                    arrow::array::new_empty_array(&components::PinholeProjection::arrow_datatype()),
                    archetypes::Pinhole::descriptor_image_from_camera(),
                ),
            )
            // TODO(#7245): Use this instead
            // .with_archetype(
            //     RowId::new(),
            //     [(timeline, 4)],
            //     &archetypes::Pinhole::clear_fields(),
            // )
            .build()
            .unwrap();
        entity_db.add_chunk(&Arc::new(chunk)).unwrap();

        // Check that the transform cache has the expected transforms.
        TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
            let timeline = *timeline.name();
            cache.apply_all_updates(&entity_db);
            let transforms_per_timeline = cache.transforms_for_timeline(timeline);
            let transforms = transforms_per_timeline
                .entity_transforms(&EntityPath::from("my_entity"))
                .unwrap();

            assert_eq!(
                transforms.latest_at_pinhole(&LatestAtQuery::new(timeline, 0)),
                None
            );
            assert_eq!(
                transforms.latest_at_pinhole(&LatestAtQuery::new(timeline, 1)),
                Some(&ResolvedPinholeProjection {
                    image_from_camera,
                    view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
                })
            );
            assert_eq!(
                transforms.latest_at_pinhole(&LatestAtQuery::new(timeline, 2)),
                Some(&ResolvedPinholeProjection {
                    image_from_camera,
                    view_coordinates: archetypes::Pinhole::DEFAULT_CAMERA_XYZ,
                })
            );
            assert_eq!(
                transforms.latest_at_pinhole(&LatestAtQuery::new(timeline, 3)),
                Some(&ResolvedPinholeProjection {
                    image_from_camera,
                    view_coordinates: components::ViewCoordinates::BLU,
                })
            );
            assert_eq!(
                transforms.latest_at_pinhole(&LatestAtQuery::new(timeline, 4)),
                None // View coordinates alone doesn't give us a pinhole projection from the transform cache.
            );
            assert_eq!(
                transforms.latest_at_pinhole(&LatestAtQuery::new(timeline, 123)),
                None
            );
        });
    }

    #[test]
    fn test_out_of_order_updates() {
        let mut entity_db = new_entity_db_with_subscriber_registered();

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype(
                RowId::new(),
                [(timeline, 1)],
                &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0]),
            )
            .with_archetype(
                RowId::new(),
                [(timeline, 3)],
                // Note that this doesn't clear anything that could be inserted at time 2.
                &archetypes::Transform3D::update_fields().with_translation([2.0, 3.0, 4.0]),
            )
            .build()
            .unwrap();
        entity_db.add_chunk(&Arc::new(chunk)).unwrap();

        // Check that the transform cache has the expected transforms.
        TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
            let timeline = *timeline.name();
            cache.apply_all_updates(&entity_db);
            let transforms_per_timeline = cache.transforms_for_timeline(timeline);
            let transforms = transforms_per_timeline
                .entity_transforms(&EntityPath::from("my_entity"))
                .unwrap();

            // Check that the transform cache has the expected transforms.
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 1)),
                glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0))
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 3)),
                glam::Affine3A::from_translation(glam::Vec3::new(2.0, 3.0, 4.0))
            );
        });

        // Add a transform between the two that invalidates the one at time stamp 3.
        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity"))
            .with_archetype(
                RowId::new(),
                [(timeline, 2)],
                &archetypes::Transform3D::update_fields().with_scale([-1.0, -2.0, -3.0]),
            )
            .build()
            .unwrap();
        entity_db.add_chunk(&Arc::new(chunk)).unwrap();

        // Check that the transform cache has the expected changed transforms.
        TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
            let timeline = *timeline.name();
            cache.apply_all_updates(&entity_db);
            let transforms_per_timeline = cache.transforms_for_timeline(timeline);
            let transforms = transforms_per_timeline
                .entity_transforms(&EntityPath::from("my_entity"))
                .unwrap();

            // Check that the transform cache has the expected transforms.
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 1)),
                glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0))
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 2)),
                glam::Affine3A::from_scale_rotation_translation(
                    glam::Vec3::new(-1.0, -2.0, -3.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(1.0, 2.0, 3.0),
                )
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 3)),
                glam::Affine3A::from_scale_rotation_translation(
                    glam::Vec3::new(-1.0, -2.0, -3.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(2.0, 3.0, 4.0),
                )
            );
        });
    }

    #[test]
    fn test_clear_non_recursive() {
        for clear_in_separate_chunk in [false, true] {
            println!("clear_in_separate_chunk: {clear_in_separate_chunk}");

            let mut entity_db = new_entity_db_with_subscriber_registered();

            let timeline = Timeline::new_sequence("t");
            let path = EntityPath::from("ent");
            let mut chunk = Chunk::builder(path.clone())
                .with_archetype(
                    RowId::new(),
                    [(timeline, 1)],
                    &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0]),
                )
                .with_archetype(
                    RowId::new(),
                    [(timeline, 3)],
                    &archetypes::Transform3D::from_translation([3.0, 4.0, 5.0]),
                );
            if !clear_in_separate_chunk {
                chunk = chunk.with_archetype(
                    RowId::new(),
                    [(timeline, 2)],
                    &archetypes::Clear::new(false),
                );
            };
            entity_db
                .add_chunk(&Arc::new(chunk.build().unwrap()))
                .unwrap();

            if clear_in_separate_chunk {
                let chunk = Chunk::builder(path.clone())
                    .with_archetype(
                        RowId::new(),
                        [(timeline, 2)],
                        &archetypes::Clear::new(false),
                    )
                    .build()
                    .unwrap();
                entity_db.add_chunk(&Arc::new(chunk)).unwrap();
            }

            TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
                let timeline = *timeline.name();
                cache.apply_all_updates(&entity_db);
                let transforms_per_timeline = cache.transforms_for_timeline(timeline);
                let transforms = transforms_per_timeline.entity_transforms(&path).unwrap();

                assert_eq!(
                    transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 1)),
                    glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0))
                );
                assert_eq!(
                    transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 2)),
                    glam::Affine3A::IDENTITY
                );
                assert_eq!(
                    transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 3)),
                    glam::Affine3A::from_translation(glam::Vec3::new(3.0, 4.0, 5.0))
                );
            });
        }
    }

    #[test]
    fn test_clear_recursive() {
        for (clear_in_separate_chunk, update_after_each_chunk) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            println!(
                "clear_in_separate_chunk: {clear_in_separate_chunk}, apply_after_each_chunk: {update_after_each_chunk}",
            );

            let mut entity_db = new_entity_db_with_subscriber_registered();

            let timeline = Timeline::new_sequence("t");

            let mut parent_chunk = Chunk::builder(EntityPath::from("parent")).with_archetype(
                RowId::new(),
                [(timeline, 1)],
                &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0]),
            );
            if !clear_in_separate_chunk {
                parent_chunk = parent_chunk.with_archetype(
                    RowId::new(),
                    [(timeline, 2)],
                    &archetypes::Clear::new(true),
                );
            };
            entity_db
                .add_chunk(&Arc::new(parent_chunk.build().unwrap()))
                .unwrap();
            if update_after_each_chunk {
                apply_all_updates(&entity_db);
            }

            let child_chunk = Chunk::builder(EntityPath::from("parent/child")).with_archetype(
                RowId::new(),
                [(timeline, 1)],
                &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0]),
            );
            entity_db
                .add_chunk(&Arc::new(child_chunk.build().unwrap()))
                .unwrap();
            if update_after_each_chunk {
                apply_all_updates(&entity_db);
            }

            if clear_in_separate_chunk {
                let chunk = Chunk::builder(EntityPath::from("parent"))
                    .with_archetype(RowId::new(), [(timeline, 2)], &archetypes::Clear::new(true))
                    .build()
                    .unwrap();
                entity_db.add_chunk(&Arc::new(chunk)).unwrap();
                if update_after_each_chunk {
                    apply_all_updates(&entity_db);
                }
            }

            TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
                let timeline = *timeline.name();
                cache.apply_all_updates(&entity_db);
                let transforms_per_timeline = cache.transforms_for_timeline(timeline);

                for path in [EntityPath::from("parent"), EntityPath::from("parent/child")] {
                    let transform = transforms_per_timeline.entity_transforms(&path).unwrap();

                    println!("checking for correct transforms for path: {path:?}");

                    assert_eq!(
                        transform.latest_at_tree_transform(&LatestAtQuery::new(timeline, 1)),
                        glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0))
                    );
                    assert_eq!(
                        transform.latest_at_tree_transform(&LatestAtQuery::new(timeline, 2)),
                        glam::Affine3A::IDENTITY
                    );
                }
            });
        }
    }

    #[test]
    fn test_gc() {
        let mut entity_db = new_entity_db_with_subscriber_registered();

        let timeline = Timeline::new_sequence("t");
        let chunk = Chunk::builder(EntityPath::from("my_entity0"))
            .with_archetype(
                RowId::new(),
                [(timeline, 1)],
                &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0]),
            )
            .build()
            .unwrap();
        entity_db.add_chunk(&Arc::new(chunk)).unwrap();

        // Apply some updates to the transform before GC pass.
        TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
            cache.apply_all_updates(&entity_db);
        });

        let chunk = Chunk::builder(EntityPath::from("my_entity1"))
            .with_archetype(
                RowId::new(),
                [(timeline, 2)],
                &archetypes::Transform3D::from_translation([4.0, 5.0, 6.0]),
            )
            .build()
            .unwrap();
        entity_db.add_chunk(&Arc::new(chunk)).unwrap();

        // Don't apply updates for this chunk.

        entity_db.gc(&GarbageCollectionOptions::gc_everything());

        TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
            assert!(
                cache.transforms_for_timeline(*timeline.name()).per_entity
                    == cache.static_timeline.per_entity
            );
        });
    }
}
