use ahash::HashMap;
use once_cell::sync::OnceCell;

use nohash_hasher::{IntMap, IntSet};
use re_chunk_store::{
    ChunkStore, ChunkStoreDiffKind, ChunkStoreEvent, ChunkStoreSubscriber,
    ChunkStoreSubscriberHandle,
};
use re_log_types::{EntityPath, EntityPathHash, StoreId};
use re_types::ComponentName;

// ---

/// Keeps track of which entities have had any `Transform3D`-related data on any timeline at any
/// point in time.
///
/// This is used to optimize queries in the `TransformContext`, so that we don't unnecessarily pay
/// for the fixed overhead of all the query layers when we know for a fact that there won't be any
/// data there.
/// This is a huge performance improvement in practice, especially in recordings with many entities.
#[derive(Default)]
pub struct TransformComponentTracker {
    /// Which entities have had any `Transform3D` component at any point in time, and which
    /// components they actually make use of.
    transform3d_entities: IntMap<EntityPathHash, IntSet<ComponentName>>,

    /// Which entities have had any `InstancePoses3D` components at any point in time, and
    /// which components they actually make use of.
    pose3d_entities: IntMap<EntityPathHash, IntSet<ComponentName>>,
}

impl TransformComponentTracker {
    /// Accesses the spatial topology for a given store.
    #[inline]
    pub fn access<T>(store_id: &StoreId, f: impl FnOnce(&Self) -> T) -> Option<T> {
        ChunkStore::with_subscriber_once(
            TransformComponentTrackerStoreSubscriber::subscription_handle(),
            move |susbcriber: &TransformComponentTrackerStoreSubscriber| {
                susbcriber.per_store.get(store_id).map(f)
            },
        )
        .flatten()
    }

    #[inline]
    pub fn transform3d_components(
        &self,
        entity_path: &EntityPath,
    ) -> Option<&IntSet<ComponentName>> {
        self.transform3d_entities.get(&entity_path.hash())
    }

    #[inline]
    pub fn pose3d_components(&self, entity_path: &EntityPath) -> Option<&IntSet<ComponentName>> {
        self.pose3d_entities.get(&entity_path.hash())
    }
}

// ---

pub struct TransformComponentTrackerStoreSubscriber {
    /// The components of interest.
    transform_components: IntSet<ComponentName>,
    pose_components: IntSet<ComponentName>,

    per_store: HashMap<StoreId, TransformComponentTracker>,
}

impl Default for TransformComponentTrackerStoreSubscriber {
    #[inline]
    fn default() -> Self {
        use re_types::Archetype as _;
        Self {
            transform_components: re_types::archetypes::Transform3D::all_components()
                .iter()
                .copied()
                .collect(),
            pose_components: re_types::archetypes::InstancePoses3D::all_components()
                .iter()
                .copied()
                .collect(),
            per_store: Default::default(),
        }
    }
}

impl TransformComponentTrackerStoreSubscriber {
    /// Accesses the global store subscriber.
    ///
    /// Lazily registers the subscriber if it hasn't been registered yet.
    pub fn subscription_handle() -> ChunkStoreSubscriberHandle {
        static SUBSCRIPTION: OnceCell<ChunkStoreSubscriberHandle> = OnceCell::new();
        *SUBSCRIPTION.get_or_init(|| ChunkStore::register_subscriber(Box::<Self>::default()))
    }
}

impl ChunkStoreSubscriber for TransformComponentTrackerStoreSubscriber {
    #[inline]
    fn name(&self) -> String {
        "rerun.store_subscriber.TransformComponentTracker".into()
    }

    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        for event in events
            .iter()
            // This is only additive, don't care about removals.
            .filter(|e| e.kind == ChunkStoreDiffKind::Addition)
        {
            let transform_component_tracker =
                self.per_store.entry(event.store_id.clone()).or_default();

            let entity_path_hash = event.chunk.entity_path().hash();

            for component_name in event.chunk.component_names() {
                if self.transform_components.contains(&component_name)
                    && event
                        .chunk
                        .components()
                        .get(&component_name)
                        .map_or(false, |list_array| {
                            list_array.offsets().lengths().any(|len| len > 0)
                        })
                {
                    transform_component_tracker
                        .transform3d_entities
                        .entry(entity_path_hash)
                        .or_default()
                        .insert(component_name);
                }
                if self.pose_components.contains(&component_name)
                    && event
                        .chunk
                        .components()
                        .get(&component_name)
                        .map_or(false, |list_array| {
                            list_array.offsets().lengths().any(|len| len > 0)
                        })
                {
                    transform_component_tracker
                        .pose3d_entities
                        .entry(entity_path_hash)
                        .or_default()
                        .insert(component_name);
                }
            }
        }
    }
}
