use std::any::TypeId;
use std::sync::Arc;

use ahash::HashMap;
use re_byte_size::{MemUsageTree, MemUsageTreeCapture};
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_log_types::StoreId;
use re_mutex::Mutex;

/// A wrapper around a cache that allows for shared access with its own lock.
///
/// This reduces lock contention by having one lock per cache type
/// instead of a single lock for all caches.
struct SharedCache {
    name: &'static str,
    cache: Mutex<Box<dyn Cache>>,
}

impl SharedCache {
    fn new<C: Cache + Default>() -> Self {
        let cache = Box::<C>::default();
        Self {
            name: cache.name(),
            cache: Mutex::new(cache),
        }
    }

    fn lock(&self) -> re_mutex::MutexGuard<'_, Box<dyn Cache>> {
        self.cache.lock()
    }
}

/// Does memoization of different objects for the immediate mode UI.
pub struct Caches {
    /// Master map from cache type to the cache itself.
    ///
    /// The master mutex is only held briefly to look up or insert a cache.
    /// Each cache has its own mutex for actual access.
    caches: Mutex<HashMap<TypeId, Arc<SharedCache>>>,

    /// The store for which these caches are caching data.
    pub store_id: StoreId,

    /// How much memory we used after the last call to [`Self::purge_memory`].
    memory_use_after_last_purge: u64,
}

impl Caches {
    /// Creates a new instance of `Caches` associated with a specific store.
    pub fn new(store_id: StoreId) -> Self {
        Self {
            caches: Mutex::new(HashMap::default()),
            store_id,
            memory_use_after_last_purge: 0,
        }
    }

    /// Call once per frame to potentially flush the cache(s).
    pub fn begin_frame(&self) {
        re_tracing::profile_function!();

        #[expect(clippy::iter_over_hash_type)] // order doesn't matter here
        for cache in self.caches.lock().values() {
            re_tracing::profile_scope!(cache.name);
            cache.lock().begin_frame();
        }
    }

    /// How much memory we used after the last call to [`Self::purge_memory`].
    ///
    /// This is the lower bound on how much memory we need.
    ///
    /// Some caches just cannot shrink below a certain size,
    /// and we need to take that into account when budgeting for other things.
    pub fn memory_use_after_last_purge(&self) -> u64 {
        self.memory_use_after_last_purge
    }

    /// Returns a memory usage tree containing only GPU memory (VRAM) usage.
    pub fn vram_usage(&self) -> MemUsageTree {
        re_tracing::profile_function!();

        let mut node = re_byte_size::MemUsageNode::new();

        let mut cache_vram: Vec<_> = self
            .caches
            .lock()
            .values()
            .map(|cache| (cache.name, cache.lock().vram_usage()))
            .collect();

        cache_vram.sort_by_key(|(cache_name, _)| *cache_name);

        for (cache_name, vram_tree) in cache_vram {
            node.add(cache_name, vram_tree);
        }

        node.into_tree()
    }

    /// Attempt to free up memory.
    pub fn purge_memory(&mut self) {
        re_tracing::profile_function!();

        #[expect(clippy::iter_over_hash_type)] // order doesn't matter here
        for cache in self.caches.lock().values() {
            re_tracing::profile_scope!(cache.name);
            cache.lock().purge_memory();
        }

        self.memory_use_after_last_purge = self.capture_mem_usage_tree().size_bytes();
    }

    /// React to the chunk store's changelog, if needed.
    ///
    /// Useful to e.g. invalidate unreachable data.
    pub fn on_rrd_manifest(&self, entity_db: &EntityDb) {
        re_tracing::profile_function!();

        if self.store_id != *entity_db.store_id() {
            return;
        }

        #[expect(clippy::iter_over_hash_type)] // order doesn't matter here
        for cache in self.caches.lock().values() {
            re_tracing::profile_scope!(cache.name);
            cache.lock().on_rrd_manifest(entity_db);
        }
    }

    /// React to the chunk store's changelog, if needed.
    ///
    /// Useful to e.g. invalidate unreachable data.
    pub fn on_store_events(&self, events: &[ChunkStoreEvent], entity_db: &EntityDb) {
        re_tracing::profile_function!();

        let relevant_events = events
            .iter()
            .filter(|event| event.store_id == self.store_id)
            .collect::<Vec<_>>();
        if relevant_events.is_empty() {
            return;
        }

        #[expect(clippy::iter_over_hash_type)] // order doesn't matter here
        for cache in self.caches.lock().values() {
            re_tracing::profile_scope!(cache.name);
            cache.lock().on_store_events(&relevant_events, entity_db);
        }
    }

    /// Accesses a cache for reading and writing.
    ///
    /// Adds the cache lazily if it wasn't already there.
    pub fn entry<C: Cache + Default, R>(&self, f: impl FnOnce(&mut C) -> R) -> R {
        let shared_cache = {
            re_tracing::profile_wait!("master-cache-lock");
            // Only hold master lock briefly to get or create the cache entry
            let mut guard = self.caches.lock();
            guard
                .entry(TypeId::of::<C>())
                .or_insert_with(|| Arc::new(SharedCache::new::<C>()))
                .clone()
        };

        // Now lock only this specific cache
        let mut cache_guard = {
            re_tracing::profile_wait!("cache-lock", shared_cache.name);
            shared_cache.lock()
        };
        let cache = cache_guard.as_mut();
        f((cache as &mut dyn std::any::Any)
            .downcast_mut::<C>()
            .expect("Downcast failed, this indicates a bug in how `Caches` adds new cache types."))
    }
}

/// A cache for memoizing things in order to speed up immediate mode UI & other immediate mode style things.
///
/// See also egus's cache system, in [`egui::cache`] (<https://docs.rs/egui/latest/egui/cache/index.html>).
pub trait Cache: std::any::Any + Send + Sync + re_byte_size::MemUsageTreeCapture {
    fn name(&self) -> &'static str;

    /// Called once per frame to potentially flush the cache.
    fn begin_frame(&mut self) {}

    /// Attempt to free up memory.
    ///
    /// This should attempt to purge everything
    /// that is not currently in use.
    ///
    /// Called BEFORE `begin_frame` (if at all).
    fn purge_memory(&mut self);

    /// Returns a memory usage tree containing only GPU memory (VRAM) usage.
    ///
    /// This should report GPU memory usage with per-item breakdown where applicable.
    /// Defaults to an empty tree (0 bytes) for caches that don't use GPU memory.
    fn vram_usage(&self) -> MemUsageTree {
        MemUsageTree::Bytes(0)
    }

    /// React to the chunk store's changelog, if needed.
    ///
    /// Useful to e.g. invalidate unreachable data.
    /// Since caches are created per store, each cache consistently receives events only for the same store.
    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], entity_db: &EntityDb) {
        _ = events;
        _ = entity_db;
    }

    /// React to receiving an rrd manifest, if needed.
    ///
    /// Useful for creating data that may be based on the information we get in the rrd manifest.
    fn on_rrd_manifest(&mut self, entity_db: &EntityDb) {
        _ = entity_db;
    }
}

impl MemUsageTreeCapture for Caches {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        re_tracing::profile_function!();

        let mut node = re_byte_size::MemUsageNode::new();

        let mut cache_trees: Vec<_> = self
            .caches
            .lock()
            .values()
            .map(|cache| (cache.name, cache.lock().capture_mem_usage_tree()))
            .collect();
        cache_trees.sort_by_key(|(cache_name, _)| *cache_name);

        for (cache_name, tree) in cache_trees {
            node.add(cache_name, tree);
        }

        node.into_tree()
    }
}
