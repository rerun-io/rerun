use std::collections::BTreeMap;

use ahash::HashMap;
use nohash_hasher::{IntMap, IntSet};

use once_cell::sync::OnceCell;
use re_chunk_store::{
    ChunkStore, ChunkStoreSubscriberHandle, LatestAtQuery, PerStoreChunkSubscriber,
};
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, EntityPathHash, StoreId, TimeInt, Timeline};
use re_types::{
    components::{self},
    ComponentName,
};

/// Store subscriber that resolves all transform components at a given entity to an affine transform.
pub struct TransformCacheStoreSubscriber {
    /// The components of interest.
    transform_components: IntSet<ComponentName>,
    pose_components: IntSet<ComponentName>,
    transforms_per_timeline: HashMap<Timeline, TransformsPerTimeline>,
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
            transforms_per_timeline: Default::default(),
        }
    }
}

#[derive(Default)]
struct TransformsPerTimeline {
    // Separate maps since we very often only have either for a given entity!
    tree_transforms_per_entity_per_time:
        IntMap<EntityPathHash, BTreeMap<TimeInt, ResolvedTreeTransform>>,
    pose_transforms_per_entity_per_time:
        IntMap<EntityPathHash, BTreeMap<TimeInt, ResolvedInstancePoses>>,
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
    Invalidated,

    /// There are no instance poses.
    #[default]
    None,
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

    pub fn latest_at_transforms(
        &mut self, // TODO: make this immutable
        entity_path: &EntityPath,
        entity_db: &EntityDb,
        query: &LatestAtQuery,
    ) -> glam::Affine3A {
        // TODO: also handle pose transforms
        // TODO: do this only once for a batch of entities.
        let Some(transforms_per_timeline) = self.transforms_per_timeline.get_mut(&query.timeline())
        else {
            return glam::Affine3A::IDENTITY;
        };

        let Some(tree_transform) = transforms_per_timeline
            .tree_transforms_per_entity_per_time
            .get_mut(&entity_path.hash())
            .and_then(|transforms_per_time| {
                transforms_per_time
                    .range_mut(..query.at())
                    .next_back()
                    .map(|(_time, transform)| transform)
            })
        else {
            return glam::Affine3A::IDENTITY;
        };

        match tree_transform {
            ResolvedTreeTransform::Cached(transform) => *transform,
            ResolvedTreeTransform::Uncached => {
                let transform = query_and_resolve_tree_transform_at_entity(
                    entity_path,
                    entity_db,
                    query,
                    self.transform_components.iter().copied(), //potential_transform_components.transform3d.iter().copied(),
                )
                .unwrap_or(glam::Affine3A::IDENTITY);
                *tree_transform = ResolvedTreeTransform::Cached(transform);
                transform
            }
            ResolvedTreeTransform::None => glam::Affine3A::IDENTITY,
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

            let entity_path_hash = event.chunk.entity_path().hash();

            for (timeline, time_column) in event.diff.chunk.timelines() {
                // Components may only show up on some of the timelines.
                // But being overly conservative here is doesn't hurt us much and makes this a lot easier.
                let transforms_per_entity =
                    self.transforms_per_timeline.entry(*timeline).or_default();

                if has_tree_transforms {
                    let tree_transforms_per_time = transforms_per_entity
                        .tree_transforms_per_entity_per_time
                        .entry(entity_path_hash)
                        .or_default();

                    // TODO: invalidate things forward in time.
                    for time in time_column.times() {
                        tree_transforms_per_time.insert(time, ResolvedTreeTransform::Uncached);
                    }
                }
                if has_instance_poses {
                    let instance_poses_per_time = transforms_per_entity
                        .pose_transforms_per_entity_per_time
                        .entry(entity_path_hash)
                        .or_default();

                    // TODO: invalidate things forward in time.
                    for time in time_column.times() {
                        instance_poses_per_time.insert(time, ResolvedInstancePoses::Invalidated);
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
    transform3d_components: impl Iterator<Item = re_types::ComponentName>,
) -> Option<glam::Affine3A> {
    let result = entity_db.latest_at(query, entity_path, transform3d_components);
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
