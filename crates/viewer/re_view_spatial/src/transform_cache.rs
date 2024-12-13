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
    Archetype as _, ComponentName,
};

/// Store subscriber that resolves all transform components at a given entity to an affine transform.
pub struct TransformCacheStoreSubscriber {
    /// The components of interest.
    transform_components: IntSet<ComponentName>,
    pose_components: IntSet<ComponentName>,
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
            per_timeline: Default::default(),
        }
    }
}

pub struct CachedTransformsPerTimeline {
    per_entity: IntMap<EntityPathHash, PerTimelinePerEntityTransforms>,
}

pub struct PerTimelinePerEntityTransforms {
    timeline: Timeline,
    entity_path: EntityPath,
    tree_transforms: BTreeMap<TimeInt, ResolvedTreeTransform>,
    pose_transforms: BTreeMap<TimeInt, ResolvedInstancePoses>,
}

#[derive(Default, Clone)]
pub enum ResolvedTreeTransform {
    /// There is a tree transform, and we have a cached value.
    Cached(glam::Affine3A),

    /// There is a tree transform, but we don't have anything cached.
    Uncached,

    /// There is no tree transform.
    #[default]
    None,
}

#[derive(Default, Clone)]
pub enum ResolvedInstancePoses {
    /// There are instance poses, and we have a cached value.
    Cached(Vec<glam::Affine3A>),

    /// There are instance poses, but we don't have anything cached.
    Uncached,

    /// There are no instance poses.
    #[default]
    None,
}

impl CachedTransformsPerTimeline {
    #[inline]
    pub fn entity_transforms(
        &mut self,
        entity_path: &EntityPath,
    ) -> Option<&mut PerTimelinePerEntityTransforms> {
        self.per_entity.get_mut(&entity_path.hash())
    }
}

impl PerTimelinePerEntityTransforms {
    pub fn latest_at_tree_transform(
        &mut self, // TODO: make this immutable
        entity_db: &EntityDb,
        query: &LatestAtQuery,
    ) -> Option<glam::Affine3A> {
        debug_assert!(query.timeline() == self.timeline);

        let tree_transform = self
            .tree_transforms
            .range_mut(..query.at())
            .next_back()
            .map(|(_time, transform)| transform)?;

        match tree_transform {
            ResolvedTreeTransform::Cached(transform) => Some(*transform),
            ResolvedTreeTransform::Uncached => {
                let transform =
                    query_and_resolve_tree_transform_at_entity(&self.entity_path, entity_db, query);
                if let Some(transform) = transform {
                    *tree_transform = ResolvedTreeTransform::Cached(transform);
                    Some(transform)
                } else {
                    *tree_transform = ResolvedTreeTransform::None;
                    None
                }
            }
            ResolvedTreeTransform::None => None,
        }
    }

    pub fn latest_at_instance_poses(
        &mut self, // TODO: make this immutable
        entity_db: &EntityDb,
        query: &LatestAtQuery,
        // TODO(andreas): A Cow or reference would be nice here instead of cloning a Vec. At least this is somewhat rare right now?
    ) -> Vec<glam::Affine3A> {
        debug_assert!(query.timeline() == self.timeline);

        let Some(pose_transforms) = self
            .pose_transforms
            .range_mut(..query.at())
            .next_back()
            .map(|(_time, transform)| transform)
        else {
            return Vec::new();
        };

        match pose_transforms {
            ResolvedInstancePoses::Cached(poses) => poses.clone(),
            ResolvedInstancePoses::Uncached => {
                let poses =
                    query_and_resolve_instance_poses_at_entity(&self.entity_path, entity_db, query);
                if !poses.is_empty() {
                    *pose_transforms = ResolvedInstancePoses::Cached(poses.clone());
                    poses
                } else {
                    *pose_transforms = ResolvedInstancePoses::None;
                    Vec::new()
                }
            }
            ResolvedInstancePoses::None => Vec::new(),
        }
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
    // TODO: no mut plz
    #[inline]
    pub fn access<T>(store_id: &StoreId, f: impl FnMut(&mut Self) -> T) -> Option<T> {
        ChunkStore::with_per_store_subscriber_mut(Self::subscription_handle(), store_id, f)
    }

    /// Accesses the transform component tracking data for a given timeline.
    ///
    /// Returns `None` if the timeline doesn't have any transforms at all.
    #[inline]
    pub fn transforms_per_timeline(
        &mut self,
        timeline: Timeline,
    ) -> Option<&mut CachedTransformsPerTimeline> {
        self.per_timeline.get_mut(&timeline)
    }
}

impl PerStoreChunkSubscriber for TransformCacheStoreSubscriber {
    fn name() -> String {
        "rerun.TransformResolverStoreSubscriber".to_owned()
    }

    fn on_events<'a>(&mut self, events: impl Iterator<Item = &'a re_chunk_store::ChunkStoreEvent>) {
        re_tracing::profile_function!();

        for event in events {
            // TODO:???
            // if event.compacted.is_some() {
            //     // Compactions don't change the data.
            //     continue;
            // }
            if event.kind == re_chunk_store::ChunkStoreDiffKind::Deletion {
                // Not participating in GC for now.
                continue;
            }

            let has_tree_transforms = event
                .chunk
                .component_names()
                .any(|component_name| self.transform_components.contains(&component_name));
            let has_instance_poses = event
                .chunk
                .component_names()
                .any(|component_name| self.pose_components.contains(&component_name));

            if !has_instance_poses && !has_tree_transforms {
                continue;
            }

            let entity_path = event.chunk.entity_path();
            let entity_path_hash = entity_path.hash();

            for (timeline, time_column) in event.diff.chunk.timelines() {
                // Components may only show up on some of the timelines.
                // But being overly conservative here is doesn't hurt us much and makes this a lot easier.
                let per_timeline = self.per_timeline.entry(*timeline).or_insert_with(|| {
                    CachedTransformsPerTimeline {
                        per_entity: Default::default(),
                    }
                });

                let per_entity = per_timeline
                    .per_entity
                    .entry(entity_path_hash)
                    .or_insert_with(|| PerTimelinePerEntityTransforms {
                        entity_path: entity_path.clone(),
                        timeline: *timeline,
                        tree_transforms: Default::default(),
                        pose_transforms: Default::default(),
                    });

                if has_tree_transforms {
                    // TODO: invalidate things forward in time.
                    for time in time_column.times() {
                        per_entity
                            .tree_transforms
                            .insert(time, ResolvedTreeTransform::Uncached);
                    }
                }
                if has_instance_poses {
                    // TODO: invalidate things forward in time.
                    for time in time_column.times() {
                        per_entity
                            .pose_transforms
                            .insert(time, ResolvedInstancePoses::Uncached);
                    }
                }
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
