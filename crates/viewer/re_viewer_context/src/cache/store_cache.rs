use nohash_hasher::IntMap;
use re_byte_size::{MemUsageNode, MemUsageTree, MemUsageTreeCapture, SizeBytes as _};
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_log_types::StoreId;

use crate::view::visualizer_entity_subscriber::VisualizerEntitySubscriber;
use crate::{
    Cache, IndicatedEntities, Memoizers, PerVisualizerType, ViewClassRegistry,
    ViewSystemIdentifier, VisualizableEntities,
};

/// Viewer-specific state associated with each store (recording or blueprint).
///
/// This bundles together all per-store caches and subscribers
/// that the viewer needs beyond the raw [`EntityDb`] data.
pub struct StoreCache {
    pub memoizers: Memoizers,

    /// Per-visualizer entity subscribers that track which entities are visualizable.
    ///
    /// Store events are forwarded to these to keep
    /// per-entity visualizability data up to date.
    entity_subscribers: IntMap<ViewSystemIdentifier, VisualizerEntitySubscriber>,
}

impl StoreCache {
    /// Create a new cache without entity subscribers or bootstrapping.
    ///
    /// Useful as a placeholder/fallback or in tests.
    pub fn empty(view_class_registry: &ViewClassRegistry, store_id: StoreId) -> Self {
        Self {
            memoizers: Memoizers::new(store_id),
            entity_subscribers: view_class_registry.create_entity_subscribers(),
        }
    }

    pub fn new(view_class_registry: &ViewClassRegistry, entity_db: &EntityDb) -> Self {
        re_tracing::profile_function!();

        let mut entity_subscribers = view_class_registry.create_entity_subscribers();

        // Bootstrap all subscribers from existing store data
        // so they're up-to-date even without having received incremental events.
        #[expect(clippy::iter_over_hash_type)] // This is order-independent
        for subscriber in entity_subscribers.values_mut() {
            subscriber.bootstrap(entity_db);
        }

        Self {
            memoizers: Memoizers::new(entity_db.store_id().clone()),
            entity_subscribers,
        }
    }

    /// The store for which these caches are caching data.
    pub fn store_id(&self) -> &StoreId {
        self.memoizers.store_id()
    }

    /// Call once per frame to potentially flush the cache.
    pub fn begin_frame(&self) {
        let Self {
            memoizers,
            entity_subscribers: _,
        } = self;
        memoizers.begin_frame();
    }

    /// Attempt to free up memory.
    ///
    /// This should attempt to purge everything
    /// that is not currently in use.
    ///
    /// Called BEFORE `begin_frame` (if at all).
    pub fn purge_memory(&mut self) {
        let Self {
            memoizers,
            entity_subscribers: _,
        } = self;
        memoizers.purge_memory();
    }

    /// React to the chunk store's changelog, e.g. to invalidate unreachable data.
    pub fn on_store_events(&mut self, events: &[ChunkStoreEvent], entity_db: &EntityDb) {
        let Self {
            memoizers,
            entity_subscribers,
        } = self;
        memoizers.on_store_events(events, entity_db);

        #[expect(clippy::iter_over_hash_type)] // This is order-independent
        for subscriber in entity_subscribers.values_mut() {
            subscriber.on_events(events);
        }
    }

    /// How much memory we used after the last call to [`Self::purge_memory`].
    ///
    /// This is the lower bound on how much memory we need.
    ///
    /// Some caches just cannot shrink below a certain size,
    /// and we need to take that into account when budgeting for other things.
    pub fn memory_use_after_last_purge(&self) -> u64 {
        let Self {
            memoizers,
            entity_subscribers: _,
        } = self;
        memoizers.memory_use_after_last_purge()
    }

    /// Returns a memory usage tree containing only GPU memory (VRAM) usage.
    pub fn vram_usage(&self) -> MemUsageTree {
        let Self {
            memoizers,
            entity_subscribers: _,
        } = self;
        memoizers.vram_usage()
    }

    /// Accesses a memoization cache for reading and writing.
    ///
    /// Adds the cache lazily if it wasn't already there.
    pub fn memoizer<C: Cache + Default, R>(&self, f: impl FnOnce(&mut C) -> R) -> R {
        self.memoizers.entry::<C, R>(f)
    }

    /// For each visualizer, return the set of entities that may be visualizable with it.
    pub fn visualizable_entities_for_visualizer_systems(
        &self,
    ) -> PerVisualizerType<&VisualizableEntities> {
        re_tracing::profile_function!();

        PerVisualizerType(
            self.entity_subscribers
                .iter()
                .map(|(id, sub)| (*id, sub.visualizable_entities()))
                .collect(),
        )
    }

    /// For each visualizer, the set of entities that have at least one component with a matching archetype name.
    pub fn indicated_entities_per_visualizer(&self) -> PerVisualizerType<&IndicatedEntities> {
        re_tracing::profile_function!();

        PerVisualizerType(
            self.entity_subscribers
                .iter()
                .map(|(id, sub)| (*id, sub.indicated_entities()))
                .collect(),
        )
    }
}

impl MemUsageTreeCapture for StoreCache {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        re_tracing::profile_function!();

        let Self {
            memoizers,
            entity_subscribers,
        } = self;

        let mut node = MemUsageNode::new();
        node.add("memoizers", memoizers.capture_mem_usage_tree());
        node.add("entity_subscribers", entity_subscribers.total_size_bytes());
        node.into_tree()
    }
}
