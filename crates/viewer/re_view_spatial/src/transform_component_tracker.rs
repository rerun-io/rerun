use once_cell::sync::OnceCell;

use nohash_hasher::{IntMap, IntSet};
use re_chunk_store::{
    ChunkStore, ChunkStoreDiffKind, ChunkStoreEvent, ChunkStoreSubscriberHandle,
    PerStoreChunkSubscriber,
};
use re_log_types::{EntityPath, EntityPathHash, StoreId};
use re_types::{Component as _, ComponentName};

// ---

/// Set of components that an entity ever had over its known lifetime.
#[derive(Default, Clone)]
pub struct PotentialTransformComponentSet {
    /// All transform components ever present.
    pub transform3d: IntSet<ComponentName>,

    /// All pose transform components ever present.
    pub pose3d: IntSet<ComponentName>,

    /// Whether the entity ever had a pinhole camera.
    pub pinhole: bool,
}

/// Keeps track of which entities have had any `Transform3D`-related data on any timeline at any
/// point in time.
///
/// This is used to optimize queries in the `TransformContext`, so that we don't unnecessarily pay
/// for the fixed overhead of all the query layers when we know for a fact that there won't be any
/// data there.
/// This is a huge performance improvement in practice, especially in recordings with many entities.
pub struct TransformComponentTrackerStoreSubscriber {
    /// The components of interest.
    transform_components: IntSet<ComponentName>,
    pose_components: IntSet<ComponentName>,

    components_per_entity: IntMap<EntityPathHash, PotentialTransformComponentSet>,
}

impl Default for TransformComponentTrackerStoreSubscriber {
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
            components_per_entity: Default::default(),
        }
    }
}

impl TransformComponentTrackerStoreSubscriber {
    /// Accesses the global store subscriber.
    ///
    /// Lazily registers the subscriber if it hasn't been registered yet.
    pub fn subscription_handle() -> ChunkStoreSubscriberHandle {
        static SUBSCRIPTION: OnceCell<ChunkStoreSubscriberHandle> = OnceCell::new();
        *SUBSCRIPTION.get_or_init(ChunkStore::register_per_store_subscriber::<Self>)
    }

    /// Accesses the transform component tracking data for a given store.
    #[inline]
    pub fn access<T>(store_id: &StoreId, f: impl FnOnce(&Self) -> T) -> Option<T> {
        ChunkStore::with_per_store_subscriber_once(Self::subscription_handle(), store_id, f)
    }

    pub fn potential_transform_components(
        &self,
        entity_path: &EntityPath,
    ) -> Option<&PotentialTransformComponentSet> {
        self.components_per_entity.get(&entity_path.hash())
    }
}

impl PerStoreChunkSubscriber for TransformComponentTrackerStoreSubscriber {
    #[inline]
    fn name() -> String {
        "rerun.store_subscriber.TransformComponentTracker".into()
    }

    fn on_events<'a>(&mut self, events: impl Iterator<Item = &'a ChunkStoreEvent>) {
        re_tracing::profile_function!();

        for event in events
            // This is only additive, don't care about removals.
            .filter(|e| e.kind == ChunkStoreDiffKind::Addition)
        {
            let entity_path_hash = event.chunk.entity_path().hash();

            let contains_non_zero_component_array = |component_name| {
                event
                    .chunk
                    .components()
                    .get(&component_name)
                    .map_or(false, |per_desc| {
                        per_desc
                            .values()
                            .any(|list_array| list_array.offsets().lengths().any(|len| len > 0))
                    })
            };

            for component_name in event.chunk.component_names() {
                if self.transform_components.contains(&component_name)
                    && contains_non_zero_component_array(component_name)
                {
                    self.components_per_entity
                        .entry(entity_path_hash)
                        .or_default()
                        .transform3d
                        .insert(component_name);
                }
                if self.pose_components.contains(&component_name)
                    && contains_non_zero_component_array(component_name)
                {
                    self.components_per_entity
                        .entry(entity_path_hash)
                        .or_default()
                        .pose3d
                        .insert(component_name);
                }
                if component_name == re_types::components::PinholeProjection::name()
                    && contains_non_zero_component_array(component_name)
                {
                    self.components_per_entity
                        .entry(entity_path_hash)
                        .or_default()
                        .pinhole = true;
                }
            }
        }
    }
}
