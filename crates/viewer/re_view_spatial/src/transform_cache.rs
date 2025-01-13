use std::collections::BTreeMap;

use ahash::HashMap;
use itertools::Either;
use nohash_hasher::{IntMap, IntSet};

use once_cell::sync::OnceCell;
use re_chunk_store::{
    ChunkStore, ChunkStoreSubscriberHandle, LatestAtQuery, PerStoreChunkSubscriber,
};
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, EntityPathHash, StoreId, TimeInt, Timeline};
use re_types::{
    components::{self},
    Archetype as _, Component, ComponentName,
};

/// Store subscriber that resolves all transform components at a given entity to an affine transform.
pub struct TransformCacheStoreSubscriber {
    transform_components: IntSet<ComponentName>,
    pose_components: IntSet<ComponentName>,
    pinhole_components: IntSet<ComponentName>,

    per_timeline: HashMap<Timeline, CachedTransformsPerTimeline>,
}

impl Default for TransformCacheStoreSubscriber {
    #[inline]
    fn default() -> Self {
        use re_types::Archetype as _;

        Self {
            transform_components: re_types::archetypes::Transform3D::all_components()
                .iter()
                .map(|descr| descr.component_name)
                .collect(),
            pose_components: re_types::archetypes::InstancePoses3D::all_components()
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
        const Tree = 1 << 0;
        const Pose = 1 << 1;
        const PinholeOrViewCoordinates = 1 << 2;
    }
}

/// Points in time that have changed for a given entity,
/// i.e. the cache is invalid for these times.
#[derive(Debug)]
struct QueuedTransformUpdates {
    entity_path: EntityPath,
    times: Vec<TimeInt>,
    aspects: TransformAspect,
}

pub struct CachedTransformsPerTimeline {
    /// Updates that should be applied to the cache.
    /// I.e. times & entities at which the cache is invalid right now.
    queued_updates: Vec<QueuedTransformUpdates>,

    per_entity: IntMap<EntityPathHash, PerTimelinePerEntityTransforms>,
}

pub struct PerTimelinePerEntityTransforms {
    timeline: Timeline,

    tree_transforms: BTreeMap<TimeInt, glam::Affine3A>,
    pose_transforms: BTreeMap<TimeInt, Vec<glam::Affine3A>>,
    // Note that pinhole projections are fairly rare - it's worth considering storing them separately so we don't have this around for every entity.
    // The flipside of that is of course that we'd have to do more lookups (unless we come up with a way to linearly iterate them)
    pinhole_projections: BTreeMap<TimeInt, ResolvedPinholeProjection>,
}

#[derive(Clone)]
pub struct ResolvedPinholeProjection {
    pub image_from_camera: components::PinholeProjection,
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
    pub fn latest_at_tree_transform(&self, query: &LatestAtQuery) -> Option<&glam::Affine3A> {
        debug_assert!(query.timeline() == self.timeline);
        self.tree_transforms
            .range(..query.at().inc())
            .next_back()
            .map(|(_time, transform)| transform)
    }

    #[inline]
    pub fn latest_at_instance_poses(&self, query: &LatestAtQuery) -> Option<&Vec<glam::Affine3A>> {
        debug_assert!(query.timeline() == self.timeline);
        self.pose_transforms
            .range(..query.at().inc())
            .next_back()
            .map(|(_time, transform)| transform)
    }

    #[inline]
    pub fn latest_at_pinhole(&self, query: &LatestAtQuery) -> Option<&ResolvedPinholeProjection> {
        debug_assert!(query.timeline() == self.timeline);
        self.pinhole_projections
            .range(..query.at().inc())
            .next_back()
            .map(|(_time, transform)| transform)
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
    /// This needs to be called once per frame.
    pub fn apply_all_updates(&mut self, entity_db: &EntityDb) {
        re_tracing::profile_function!();

        for (timeline, per_timeline) in &mut self.per_timeline {
            for queued_update in per_timeline.queued_updates.drain(..) {
                let entity_path = &queued_update.entity_path;
                let entity_entry = per_timeline
                    .per_entity
                    .entry(entity_path.hash())
                    .or_insert_with(|| PerTimelinePerEntityTransforms {
                        timeline: *timeline,
                        tree_transforms: Default::default(),
                        pose_transforms: Default::default(),
                        pinhole_projections: Default::default(),
                    });

                for time in queued_update.times {
                    let query = LatestAtQuery::new(*timeline, time);

                    if queued_update.aspects.contains(TransformAspect::Tree) {
                        if let Some(transform) = query_and_resolve_tree_transform_at_entity(
                            entity_path,
                            entity_db,
                            &query,
                        ) {
                            entity_entry.tree_transforms.insert(time, transform);
                        }
                    }
                    if queued_update.aspects.contains(TransformAspect::Pose) {
                        let transforms = query_and_resolve_instance_poses_at_entity(
                            entity_path,
                            entity_db,
                            &query,
                        );
                        if !transforms.is_empty() {
                            entity_entry.pose_transforms.insert(time, transforms);
                        }
                    }
                    if queued_update
                        .aspects
                        .contains(TransformAspect::PinholeOrViewCoordinates)
                    {
                        if let Some(resolved_pinhole_projection) =
                            query_and_resolve_pinhole_projection_at_entity(
                                entity_path,
                                entity_db,
                                &query,
                            )
                        {
                            entity_entry
                                .pinhole_projections
                                .insert(time, resolved_pinhole_projection);
                        }
                    }
                }
            }
        }
    }
}

impl PerStoreChunkSubscriber for TransformCacheStoreSubscriber {
    fn name() -> String {
        "rerun.TransformResolverStoreSubscriber".to_owned()
    }

    fn on_events<'a>(&mut self, events: impl Iterator<Item = &'a re_chunk_store::ChunkStoreEvent>) {
        re_tracing::profile_function!();

        for event in events {
            if event.compacted.is_some() {
                // Compactions don't change the data.
                continue;
            }
            if event.kind == re_chunk_store::ChunkStoreDiffKind::Deletion {
                // Not participating in GC for now.
                continue;
            }

            let mut aspects = TransformAspect::empty();
            for component_name in event.chunk.component_names() {
                if self.transform_components.contains(&component_name) {
                    aspects.set(TransformAspect::Tree, true);
                }
                if self.pose_components.contains(&component_name) {
                    aspects.set(TransformAspect::Pose, true);
                }
                if self.pinhole_components.contains(&component_name) {
                    aspects.set(TransformAspect::PinholeOrViewCoordinates, true);
                }
            }
            if aspects.is_empty() {
                continue;
            }

            let entity_path = event.chunk.entity_path();

            for (timeline, time_column) in event.diff.chunk.timelines() {
                // The components we are interested in may only show up on some of the timelines.
                // But that's fairly rare, so a few false positive entries here are fine.
                let per_timeline = self.per_timeline.entry(*timeline).or_insert_with(|| {
                    CachedTransformsPerTimeline {
                        queued_updates: Default::default(),
                        per_entity: Default::default(),
                    }
                });

                // All of these require complex latest-at queries that would require a lot more context,
                // are fairly expensive, and may depend on other components that may come in at the same time.
                // (we could inject that here, but it's not entirely straight forward).
                // So instead, we note down that the caches is invalidated for the given entity & times.

                // Any time _after_ the first event in this chunk is no longer valid now.
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
                        let invalidated_pose_transforms =
                            entity_entry.pose_transforms.split_off(&min_time);
                        invalidated_times.extend(invalidated_pose_transforms.into_keys());
                    }
                    if aspects.contains(TransformAspect::PinholeOrViewCoordinates) {
                        let invalidated_pinhole_projections =
                            entity_entry.pinhole_projections.split_off(&min_time);
                        invalidated_times.extend(invalidated_pinhole_projections.into_keys());
                    }
                }

                per_timeline.queued_updates.push(QueuedTransformUpdates {
                    entity_path: entity_path.clone(),
                    times: time_column
                        .times()
                        .chain(invalidated_times.into_iter())
                        .collect(),
                    aspects,
                });
            }
        }
    }
}

fn query_and_resolve_tree_transform_at_entity(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> Option<glam::Affine3A> {
    // TODO(andreas): Filter out the components we're actually interested in?
    let components = re_types::archetypes::Transform3D::all_components();
    let component_names = components.iter().map(|descr| descr.component_name);
    let result = entity_db.latest_at(query, entity_path, component_names);
    if result.components.is_empty() {
        return None;
    }

    let mut transform = glam::Affine3A::IDENTITY;

    // Order see `debug_assert_transform_field_order`
    if let Some(translation) = result.component_instance::<components::Translation3D>(0) {
        transform = glam::Affine3A::from(translation);
    }
    if let Some(axis_angle) = result.component_instance::<components::RotationAxisAngle>(0) {
        if let Ok(axis_angle) = glam::Affine3A::try_from(axis_angle) {
            transform *= axis_angle;
        } else {
            // Invalid transform.
            return None;
        }
    }
    if let Some(quaternion) = result.component_instance::<components::RotationQuat>(0) {
        if let Ok(quaternion) = glam::Affine3A::try_from(quaternion) {
            transform *= quaternion;
        } else {
            // Invalid transform.
            return None;
        }
    }
    if let Some(scale) = result.component_instance::<components::Scale3D>(0) {
        if scale.x() == 0.0 && scale.y() == 0.0 && scale.z() == 0.0 {
            // Invalid scale.
            return None;
        }
        transform *= glam::Affine3A::from(scale);
    }
    if let Some(mat3x3) = result.component_instance::<components::TransformMat3x3>(0) {
        let affine_transform = glam::Affine3A::from(mat3x3);
        if affine_transform.matrix3.determinant() == 0.0 {
            // Invalid transform.
            return None;
        }
        transform *= affine_transform;
    }

    if result.component_instance::<components::TransformRelation>(0) == Some(components::TransformRelation::ChildFromParent)
    // TODO(andreas): Should we warn?
        && transform.matrix3.determinant() != 0.0
    {
        transform = transform.inverse();
    }

    Some(transform)
}

fn query_and_resolve_instance_poses_at_entity(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> Vec<glam::Affine3A> {
    // TODO(andreas): Filter out the components we're actually interested in?
    let components = re_types::archetypes::InstancePoses3D::all_components();
    let component_names = components.iter().map(|descr| descr.component_name);
    let result = entity_db.latest_at(query, entity_path, component_names);

    let max_count = result
        .components
        .iter()
        .map(|(name, row)| row.num_instances(name))
        .max()
        .unwrap_or(0) as usize;

    if max_count == 0 {
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

    let mut iter_translation = clamped_or_nothing(
        result
            .component_batch::<components::PoseTranslation3D>()
            .unwrap_or_default(),
        max_count,
    );
    let mut iter_rotation_quat = clamped_or_nothing(
        result
            .component_batch::<components::PoseRotationQuat>()
            .unwrap_or_default(),
        max_count,
    );
    let mut iter_rotation_axis_angle = clamped_or_nothing(
        result
            .component_batch::<components::PoseRotationAxisAngle>()
            .unwrap_or_default(),
        max_count,
    );
    let mut iter_scale = clamped_or_nothing(
        result
            .component_batch::<components::PoseScale3D>()
            .unwrap_or_default(),
        max_count,
    );
    let mut iter_mat3x3 = clamped_or_nothing(
        result
            .component_batch::<components::PoseTransformMat3x3>()
            .unwrap_or_default(),
        max_count,
    );

    let mut transforms = Vec::with_capacity(max_count);
    for _ in 0..max_count {
        // Order see `debug_assert_transform_field_order`
        let mut transform = glam::Affine3A::IDENTITY;
        if let Some(translation) = iter_translation.next() {
            transform = glam::Affine3A::from(translation);
        }
        if let Some(rotation_quat) = iter_rotation_quat.next() {
            if let Ok(rotation_quat) = glam::Affine3A::try_from(rotation_quat) {
                transform *= rotation_quat;
            } else {
                transform = glam::Affine3A::ZERO;
            }
        }
        if let Some(rotation_axis_angle) = iter_rotation_axis_angle.next() {
            if let Ok(axis_angle) = glam::Affine3A::try_from(rotation_axis_angle) {
                transform *= axis_angle;
            } else {
                transform = glam::Affine3A::ZERO;
            }
        }
        if let Some(scale) = iter_scale.next() {
            transform *= glam::Affine3A::from(scale);
        }
        if let Some(mat3x3) = iter_mat3x3.next() {
            transform *= glam::Affine3A::from(mat3x3);
        }

        transforms.push(transform);
    }

    transforms
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
                .map_or(components::ViewCoordinates::RDF, |(_index, res)| res),
        })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk_store::{external::re_chunk::ChunkBuilder, ChunkId, RowId};
    use re_types::archetypes;

    use super::*;

    #[test]
    fn test_tree_transforms() {
        let mut entity_db = EntityDb::new(StoreId::random(re_log_types::StoreKind::Recording));
        TransformCacheStoreSubscriber::access(&entity_db.store_id(), |_| {
            // Make sure the subscriber is registered.
        });

        // Log a few tree transforms at different times.
        let timeline = Timeline::new_sequence("t");
        entity_db
            .add_chunk(&Arc::new(
                ChunkBuilder::new(ChunkId::new(), EntityPath::from("my_entity"))
                    .with_archetype(
                        RowId::new(),
                        [(timeline, 1)],
                        &archetypes::Transform3D::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
                    )
                    .with_component_batch(
                        RowId::new(),
                        [(timeline, 3)],
                        // Don't clear out existing translation.
                        &[components::Scale3D(glam::Vec3::new(1.0, 2.0, 3.0).into())],
                    )
                    .with_archetype(
                        RowId::new(),
                        [(timeline, 4)],
                        // Clears out previous translation & scale.
                        //&archetypes::Transform3D::from_rotation(glam::Quat::from_rotation_x(1.0)),
                        &archetypes::Transform3D::from_translation(glam::Vec3::new(
                            123.0, 2.0, 3.0,
                        )),
                    )
                    .build()
                    .unwrap(),
            ))
            .unwrap();

        // Check that the transform cache has the expected transforms.
        TransformCacheStoreSubscriber::access_mut(&entity_db.store_id(), |cache| {
            cache.apply_all_updates(&entity_db);
            let transforms_per_timeline = cache.transforms_per_timeline(timeline).unwrap();
            assert!(transforms_per_timeline
                .entity_transforms(EntityPath::from("not_my_entity").hash())
                .is_none());

            let transforms = transforms_per_timeline
                .entity_transforms(EntityPath::from("my_entity").hash())
                .unwrap();

            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 0)),
                None
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 1)),
                Some(&glam::Affine3A::from_translation(glam::Vec3::new(
                    1.0, 2.0, 3.0
                )))
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 2)),
                Some(&glam::Affine3A::from_translation(glam::Vec3::new(
                    1.0, 2.0, 3.0
                )))
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 3)),
                Some(&glam::Affine3A::from_scale_rotation_translation(
                    glam::Vec3::new(1.0, 2.0, 3.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(1.0, 2.0, 3.0),
                ))
            );
            assert_eq!(
                transforms.latest_at_tree_transform(&LatestAtQuery::new(timeline, 4)),
                //       Some(&glam::Affine3A::from_quat(glam::Quat::from_rotation_x(1.0)))
                Some(&glam::Affine3A::from_translation(glam::Vec3::new(
                    123.0, 2.0, 3.0
                )))
            );
        });
    }

    #[test]
    fn test_pose_transforms() {
        // TODO:
    }

    #[test]
    fn test_pinhole_projections() {
        // TODO:
    }

    #[test]
    fn test_invalidation() {
        // TODO:
    }
}
