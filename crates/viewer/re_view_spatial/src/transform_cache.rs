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
use re_log_types::{EntityPath, EntityPathHash, StoreId, TimeInt, Timeline};
use re_types::{
    archetypes::{self},
    components::{self},
    Archetype as _, Component, ComponentName,
};

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
pub struct TransformCacheStoreSubscriber {
    /// All components of [`archetypes::Transform3D`]
    transform_components: IntSet<ComponentName>,

    /// All components of [`archetypes::InstancePoses3D`]
    pose_components: IntSet<ComponentName>,

    /// All components related to pinholes (i.e. [`components::PinholeProjection`] and [`components::ViewCoordinates`]).
    pinhole_components: IntSet<ComponentName>,

    per_timeline: HashMap<Timeline, CachedTransformsPerTimeline>,
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
    }
}

/// Points in time that have changed for a given entity,
/// i.e. the cache is invalid for these times.
#[derive(Debug)]
struct InvalidatedTransforms {
    entity_path: EntityPath,
    times: Vec<TimeInt>,
    aspects: TransformAspect,
}

#[derive(Default)]
pub struct CachedTransformsPerTimeline {
    /// Updates that should be applied to the cache.
    /// I.e. times & entities at which the cache is invalid right now.
    invalidated_transforms: Vec<InvalidatedTransforms>,

    per_entity: IntMap<EntityPathHash, PerTimelinePerEntityTransforms>,
}

type PoseTransformMap = BTreeMap<TimeInt, Vec<Affine3A>>;

/// Maps from time to pinhole projection.
///
/// Unlike with tree & pose transforms, there's identity value that we can insert upon clears.
/// (clears here meaning that the user first logs a pinhole and then later either logs a clear or an empty pinhole array)
/// Therefore, we instead store those events as `None` values to ensure that everything after a clear
/// is properly marked as having no pinhole projection.
type PinholeProjectionMap = BTreeMap<TimeInt, Option<ResolvedPinholeProjection>>;

pub struct PerTimelinePerEntityTransforms {
    timeline: Timeline,

    tree_transforms: BTreeMap<TimeInt, Affine3A>,

    // Pose transforms and pinhole projections are typically more rare, which is why we store them as optional boxes.
    pose_transforms: Option<Box<PoseTransformMap>>,
    pinhole_projections: Option<Box<PinholeProjectionMap>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPinholeProjection {
    pub image_from_camera: components::PinholeProjection,

    /// View coordinates at this pinhole camera.
    ///
    /// This is needed to orient 2D in 3D and 3D in 2D the right way around
    /// (answering questions like which axis is distance to viewer increasing).
    /// If no view coordinates were logged, this is set to [`Self::DEFAULT_VIEW_COORDINATES`].
    pub view_coordinates: components::ViewCoordinates,
}

impl CachedTransformsPerTimeline {
    #[inline]
    pub fn entity_transforms(
        &self,
        entity_path: EntityPathHash,
    ) -> Option<&PerTimelinePerEntityTransforms> {
        self.per_entity.get(&entity_path)
    }
}

impl PerTimelinePerEntityTransforms {
    #[inline]
    pub fn latest_at_tree_transform(&self, query: &LatestAtQuery) -> Affine3A {
        debug_assert_eq!(query.timeline(), self.timeline);
        self.tree_transforms
            .range(..query.at().inc())
            .next_back()
            .map(|(_time, transform)| *transform)
            .unwrap_or(Affine3A::IDENTITY)
    }

    #[inline]
    pub fn latest_at_instance_poses(&self, query: &LatestAtQuery) -> &[Affine3A] {
        debug_assert_eq!(query.timeline(), self.timeline);
        self.pose_transforms
            .as_ref()
            .and_then(|pose_transforms| pose_transforms.range(..query.at().inc()).next_back())
            .map(|(_time, pose_transforms)| pose_transforms.as_slice())
            .unwrap_or(&[])
    }

    #[inline]
    pub fn latest_at_pinhole(&self, query: &LatestAtQuery) -> Option<&ResolvedPinholeProjection> {
        debug_assert_eq!(query.timeline(), self.timeline);
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
    pub fn transforms_per_timeline(
        &self,
        timeline: Timeline,
    ) -> Option<&CachedTransformsPerTimeline> {
        self.per_timeline.get(&timeline)
    }

    /// Makes sure the transform cache is up to date with the latest data.
    ///
    /// This needs to be called once per frame prior to any transform propagation.
    /// (which is done by [`crate::contexts::TransformTreeContext`])
    pub fn apply_all_updates(&mut self, entity_db: &EntityDb) {
        re_tracing::profile_function!();

        for (timeline, per_timeline) in &mut self.per_timeline {
            for invalidated_transform in per_timeline.invalidated_transforms.drain(..) {
                let entity_path = &invalidated_transform.entity_path;
                let entity_entry = per_timeline
                    .per_entity
                    .entry(entity_path.hash())
                    .or_insert_with(|| PerTimelinePerEntityTransforms {
                        timeline: *timeline,
                        tree_transforms: Default::default(),
                        pose_transforms: Default::default(),
                        pinhole_projections: Default::default(),
                    });

                for time in invalidated_transform.times {
                    let query = LatestAtQuery::new(*timeline, time);

                    if invalidated_transform
                        .aspects
                        .contains(TransformAspect::Tree)
                    {
                        let transform = query_and_resolve_tree_transform_at_entity(
                            entity_path,
                            entity_db,
                            &query,
                        )
                        .unwrap_or(Affine3A::IDENTITY);
                        // If there's *no* transform, we have to put identity in, otherwise we'd miss clears!
                        entity_entry.tree_transforms.insert(time, transform);
                    }
                    if invalidated_transform
                        .aspects
                        .contains(TransformAspect::Pose)
                    {
                        let poses = query_and_resolve_instance_poses_at_entity(
                            entity_path,
                            entity_db,
                            &query,
                        );
                        // *do* also insert empty ones, otherwise it's not possible to clear previous state.
                        entity_entry
                            .pose_transforms
                            .get_or_insert_with(Box::default)
                            .insert(time, poses);
                    }
                    if invalidated_transform
                        .aspects
                        .contains(TransformAspect::PinholeOrViewCoordinates)
                    {
                        let pinhole_projection = query_and_resolve_pinhole_projection_at_entity(
                            entity_path,
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

    fn add_chunk(&mut self, event: &re_chunk_store::ChunkStoreEvent, aspects: TransformAspect) {
        let entity_path = event.chunk.entity_path();

        for (timeline, time_column) in event.diff.chunk.timelines() {
            let per_timeline = self.per_timeline.entry(*timeline).or_default();

            // All of these require complex latest-at queries that would require a lot more context,
            // are fairly expensive, and may depend on other components that may come in at the same time.
            // (we could inject that here, but it's not entirely straight forward).
            // So instead, we note down that the caches is invalidated for the given entity & times.

            // This invalidates any time _after_ the first event in this chunk.
            // (e.g. if a rotation is added prior to translations later on,
            // then the resulting transforms at those translations changes as well for latest-at queries)
            let mut invalidated_times = Vec::new();
            let Some(min_time) = time_column.times().min() else {
                continue;
            };
            if let Some(entity_entry) = per_timeline.per_entity.get_mut(&entity_path.hash()) {
                if aspects.contains(TransformAspect::Tree) {
                    let invalidated_tree_transforms =
                        entity_entry.tree_transforms.split_off(&min_time);
                    invalidated_times.extend(invalidated_tree_transforms.into_keys());
                }
                if aspects.contains(TransformAspect::Pose) {
                    if let Some(pose_transforms) = &mut entity_entry.pose_transforms {
                        let invalidated_pose_transforms = pose_transforms.split_off(&min_time);
                        invalidated_times.extend(invalidated_pose_transforms.into_keys());
                    }
                }
                if aspects.contains(TransformAspect::PinholeOrViewCoordinates) {
                    if let Some(pinhole_projections) = &mut entity_entry.pinhole_projections {
                        let invalidated_pinhole_projections =
                            pinhole_projections.split_off(&min_time);
                        invalidated_times.extend(invalidated_pinhole_projections.into_keys());
                    }
                }
            }

            per_timeline
                .invalidated_transforms
                .push(InvalidatedTransforms {
                    entity_path: entity_path.clone(),
                    times: time_column
                        .times()
                        .chain(invalidated_times.into_iter())
                        .collect(),
                    aspects,
                });
        }
    }

    fn remove_chunk(&mut self, event: &re_chunk_store::ChunkStoreEvent, aspects: TransformAspect) {
        let entity_path = event.chunk.entity_path();

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
            if let Some(per_entity) = per_timeline.per_entity.get_mut(&entity_path.hash()) {
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
                        .map_or(true, |pose_transforms| pose_transforms.is_empty())
                    && per_entity
                        .pinhole_projections
                        .as_ref()
                        .map_or(true, |pinhole_projections| pinhole_projections.is_empty())
                {
                    per_timeline.per_entity.remove(&entity_path.hash());
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
            }
            if aspects.is_empty() {
                continue;
            }

            if event.kind == re_chunk_store::ChunkStoreDiffKind::Deletion {
                self.remove_chunk(event, aspects);
            } else {
                self.add_chunk(event, aspects);
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
    let components = archetypes::Transform3D::all_components();
    let component_names = components.iter().map(|descr| descr.component_name);
    let result = entity_db.latest_at(query, entity_path, component_names);
    if result.components.is_empty() {
        return None;
    }

    let mut transform = Affine3A::IDENTITY;

    // The order of the components here is important, and checked by `debug_assert_transform_field_order`
    if let Some(translation) = result.component_instance::<components::Translation3D>(0) {
        transform = Affine3A::from(translation);
    }
    if let Some(axis_angle) = result.component_instance::<components::RotationAxisAngle>(0) {
        if let Ok(axis_angle) = Affine3A::try_from(axis_angle) {
            transform *= axis_angle;
        } else {
            return Some(Affine3A::ZERO);
        }
    }
    if let Some(quaternion) = result.component_instance::<components::RotationQuat>(0) {
        if let Ok(quaternion) = Affine3A::try_from(quaternion) {
            transform *= quaternion;
        } else {
            return Some(Affine3A::ZERO);
        }
    }
    if let Some(scale) = result.component_instance::<components::Scale3D>(0) {
        if scale.x() == 0.0 && scale.y() == 0.0 && scale.z() == 0.0 {
            return Some(Affine3A::ZERO);
        }
        transform *= Affine3A::from(scale);
    }
    if let Some(mat3x3) = result.component_instance::<components::TransformMat3x3>(0) {
        let affine_transform = Affine3A::from(mat3x3);
        if affine_transform.matrix3.determinant() == 0.0 {
            return Some(Affine3A::ZERO);
        }
        transform *= affine_transform;
    }

    if result.component_instance::<components::TransformRelation>(0)
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

/// Queries all components that are part of pose transforms, returning the transform from child to parent.
///
/// If any of the components yields an invalid transform, returns a `glam::Affine3A::ZERO` for that instance.
/// (this effectively ignores the instance for most visualizations!)
// TODO(#3849): There's no way to discover invalid transforms right now (they can be intentional but often aren't).
fn query_and_resolve_instance_poses_at_entity(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> Vec<Affine3A> {
    // TODO(andreas): Filter out styling components.
    let components = archetypes::InstancePoses3D::all_components();
    let component_names = components.iter().map(|descr| descr.component_name);
    let result = entity_db.latest_at(query, entity_path, component_names);

    let max_num_instances = result
        .components
        .iter()
        .map(|(name, row)| row.num_instances(name))
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
        .component_batch::<components::PoseTranslation3D>()
        .unwrap_or_default();
    let batch_rotation_quat = result
        .component_batch::<components::PoseRotationQuat>()
        .unwrap_or_default();
    let batch_rotation_axis_angle = result
        .component_batch::<components::PoseRotationAxisAngle>()
        .unwrap_or_default();
    let batch_scale = result
        .component_batch::<components::PoseScale3D>()
        .unwrap_or_default();
    let batch_mat3x3 = result
        .component_batch::<components::PoseTransformMat3x3>()
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
        .latest_at_component::<components::PinholeProjection>(entity_path, query)
        .map(|(_index, image_from_camera)| ResolvedPinholeProjection {
            image_from_camera,
            view_coordinates: entity_db
                .latest_at_component::<components::ViewCoordinates>(entity_path, query)
                .map_or(archetypes::Pinhole::DEFAULT_CAMERA_XYZ, |(_index, res)| res),
        })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk_store::{
        external::re_chunk::ChunkBuilder, ChunkId, GarbageCollectionOptions, RowId,
    };
    use re_types::{archetypes, Loggable, SerializedComponentBatch};

    use super::*;

    fn ensure_subscriber_registered(entity_db: &EntityDb) {
        TransformCacheStoreSubscriber::access(&entity_db.store_id(), |_| {
            // Make sure the subscriber is registered.
        });
    }

    #[test]
    fn test_transforms_per_timeline_access() {
        let mut entity_db = EntityDb::new(StoreId::random(re_log_types::StoreKind::Recording));
        ensure_subscriber_registered(&entity_db);

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk0 = ChunkBuilder::new(ChunkId::new(), EntityPath::from("with_transform"))
            .with_archetype(
                RowId::new(),
                [(timeline, 1)],
                &archetypes::Transform3D::from_translation([1.0, 2.0, 3.0]),
            )
            .build()
            .unwrap();
        let chunk1 = ChunkBuilder::new(ChunkId::new(), EntityPath::from("without_transform"))
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
            let transforms_per_timeline = cache.transforms_per_timeline(timeline).unwrap();
            assert!(transforms_per_timeline
                .entity_transforms(EntityPath::from("without_transform").hash())
                .is_none());
            assert!(transforms_per_timeline
                .entity_transforms(EntityPath::from("rando").hash())
                .is_none());
            let transforms = transforms_per_timeline
                .entity_transforms(EntityPath::from("with_transform").hash())
                .unwrap();
            assert_eq!(transforms.timeline, timeline);
            assert_eq!(transforms.tree_transforms.len(), 1);
            assert_eq!(transforms.pose_transforms, None);
            assert_eq!(transforms.pinhole_projections, None);
        });
    }

    #[test]
    fn test_tree_transforms() {
        let mut entity_db = EntityDb::new(StoreId::random(re_log_types::StoreKind::Recording));
        ensure_subscriber_registered(&entity_db);

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = ChunkBuilder::new(ChunkId::new(), EntityPath::from("my_entity"))
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
            cache.apply_all_updates(&entity_db);
            let transforms_per_timeline = cache.transforms_per_timeline(timeline).unwrap();
            let transforms = transforms_per_timeline
                .entity_transforms(EntityPath::from("my_entity").hash())
                .unwrap();

            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 0)),
                glam::Affine3A::IDENTITY
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 1)),
                glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0))
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 2)),
                glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0))
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 3)),
                glam::Affine3A::from_scale_rotation_translation(
                    glam::Vec3::new(1.0, 2.0, 3.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(1.0, 2.0, 3.0),
                )
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 4)),
                glam::Affine3A::from_quat(glam::Quat::from_rotation_x(1.0))
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 5)),
                glam::Affine3A::IDENTITY
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 123)),
                glam::Affine3A::IDENTITY
            );
        });
    }

    #[test]
    fn test_pose_transforms() {
        let mut entity_db = EntityDb::new(StoreId::random(re_log_types::StoreKind::Recording));
        ensure_subscriber_registered(&entity_db);

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = ChunkBuilder::new(ChunkId::new(), EntityPath::from("my_entity"))
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
            cache.apply_all_updates(&entity_db);
            let transforms_per_timeline = cache.transforms_per_timeline(timeline).unwrap();
            let transforms = transforms_per_timeline
                .entity_transforms(EntityPath::from("my_entity").hash())
                .unwrap();

            assert_eq!(
                transforms.latest_at_instance_poses(&LatestAtQuery::new(timeline, 0)),
                &[]
            );
            assert_eq!(
                transforms.latest_at_instance_poses(&LatestAtQuery::new(timeline, 1)),
                &[
                    glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                    glam::Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
                    glam::Affine3A::from_translation(glam::Vec3::new(7.0, 8.0, 9.0)),
                ]
            );
            assert_eq!(
                transforms.latest_at_instance_poses(&LatestAtQuery::new(timeline, 2)),
                &[
                    glam::Affine3A::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                    glam::Affine3A::from_translation(glam::Vec3::new(4.0, 5.0, 6.0)),
                    glam::Affine3A::from_translation(glam::Vec3::new(7.0, 8.0, 9.0)),
                ]
            );
            assert_eq!(
                transforms.latest_at_instance_poses(&LatestAtQuery::new(timeline, 3)),
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
                transforms.latest_at_instance_poses(&LatestAtQuery::new(timeline, 4)),
                &[]
            );
            assert_eq!(
                transforms.latest_at_instance_poses(&LatestAtQuery::new(timeline, 123)),
                &[]
            );
        });
    }

    #[test]
    fn test_pinhole_projections() {
        let mut entity_db = EntityDb::new(StoreId::random(re_log_types::StoreKind::Recording));
        ensure_subscriber_registered(&entity_db);

        let image_from_camera =
            components::PinholeProjection::from_focal_length_and_principal_point(
                [1.0, 2.0],
                [1.0, 2.0],
            );

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = ChunkBuilder::new(ChunkId::new(), EntityPath::from("my_entity"))
            .with_archetype(
                RowId::new(),
                [(timeline, 1)],
                &archetypes::Pinhole::new(image_from_camera),
            )
            .with_archetype(
                RowId::new(),
                [(timeline, 3)],
                &archetypes::ViewCoordinates::BLU,
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
            cache.apply_all_updates(&entity_db);
            let transforms_per_timeline = cache.transforms_per_timeline(timeline).unwrap();
            let transforms = transforms_per_timeline
                .entity_transforms(EntityPath::from("my_entity").hash())
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
        let mut entity_db = EntityDb::new(StoreId::random(re_log_types::StoreKind::Recording));
        ensure_subscriber_registered(&entity_db);

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        let chunk = ChunkBuilder::new(ChunkId::new(), EntityPath::from("my_entity"))
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
            cache.apply_all_updates(&entity_db);
            let transforms_per_timeline = cache.transforms_per_timeline(timeline).unwrap();
            let transforms = transforms_per_timeline
                .entity_transforms(EntityPath::from("my_entity").hash())
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
        let chunk = ChunkBuilder::new(ChunkId::new(), EntityPath::from("my_entity"))
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
            cache.apply_all_updates(&entity_db);
            let transforms_per_timeline = cache.transforms_per_timeline(timeline).unwrap();
            let transforms = transforms_per_timeline
                .entity_transforms(EntityPath::from("my_entity").hash())
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
    fn test_gc() {
        let mut entity_db = EntityDb::new(StoreId::random(re_log_types::StoreKind::Recording));
        ensure_subscriber_registered(&entity_db);

        let timeline = Timeline::new_sequence("t");
        let chunk = ChunkBuilder::new(ChunkId::new(), EntityPath::from("my_entity0"))
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

        let chunk = ChunkBuilder::new(ChunkId::new(), EntityPath::from("my_entity1"))
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
            assert!(cache.transforms_per_timeline(timeline).is_none());
        });
    }
}
