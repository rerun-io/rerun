use re_byte_size::{MemUsageNode, MemUsageTree, MemUsageTreeCapture};
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_log_types::StoreId;

use crate::{Cache, Memoizers};

/// Viewer-specific state associated with each store (recording or blueprint).
///
/// This bundles together all per-store caches and subscribers
/// that the viewer needs beyond the raw [`EntityDb`] data.
pub struct StoreCache {
    pub memoizers: Memoizers,
}

impl StoreCache {
    pub fn new(store_id: StoreId) -> Self {
        Self {
            memoizers: Memoizers::new(store_id),
        }
    }

    /// The store for which these caches are caching data.
    pub fn store_id(&self) -> &StoreId {
        self.memoizers.store_id()
    }

    /// Call once per frame to potentially flush the cache.
    pub fn begin_frame(&self) {
        let Self { memoizers } = self;
        memoizers.begin_frame();
    }

    /// Attempt to free up memory.
    ///
    /// This should attempt to purge everything
    /// that is not currently in use.
    ///
    /// Called BEFORE `begin_frame` (if at all).
    pub fn purge_memory(&mut self) {
        let Self { memoizers } = self;
        memoizers.purge_memory();
    }

    /// React to the chunk store's changelog, e.g. to invalidate unreachable data.
    pub fn on_store_events(&self, events: &[ChunkStoreEvent], entity_db: &EntityDb) {
        let Self { memoizers } = self;
        memoizers.on_store_events(events, entity_db);
    }

    /// How much memory we used after the last call to [`Self::purge_memory`].
    ///
    /// This is the lower bound on how much memory we need.
    ///
    /// Some caches just cannot shrink below a certain size,
    /// and we need to take that into account when budgeting for other things.
    pub fn memory_use_after_last_purge(&self) -> u64 {
        let Self { memoizers } = self;
        memoizers.memory_use_after_last_purge()
    }

    /// Returns a memory usage tree containing only GPU memory (VRAM) usage.
    pub fn vram_usage(&self) -> MemUsageTree {
        let Self { memoizers } = self;
        memoizers.vram_usage()
    }

    /// Accesses a memoization cache for reading and writing.
    ///
    /// Adds the cache lazily if it wasn't already there.
    pub fn memoizer<C: Cache + Default, R>(&self, f: impl FnOnce(&mut C) -> R) -> R {
        self.memoizers.entry::<C, R>(f)
    }
}

impl MemUsageTreeCapture for StoreCache {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        let Self { memoizers } = self;

        let mut node = MemUsageNode::new();
        node.add("memoizers", memoizers.capture_mem_usage_tree());
        node.into_tree()
    }
}
