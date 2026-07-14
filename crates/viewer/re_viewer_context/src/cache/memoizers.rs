use std::any::TypeId;
use std::sync::Arc;

use ahash::HashMap;
use re_byte_size::{MemUsageTree, MemUsageTreeCapture};
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_log_types::StoreId;
use re_mutex::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{Cache, CacheEntryAccess};

/// A wrapper around a cache that allows for shared access with its own lock.
///
/// This reduces lock contention by having one lock per cache type
/// instead of a single lock for all caches.
struct SharedCache {
    name: &'static str,
    cache: RwLock<Box<dyn Cache>>,
}

impl SharedCache {
    fn new<C: Cache + Default>() -> Self {
        let cache = Box::<C>::default();
        Self {
            name: cache.name(),
            cache: RwLock::new(cache),
        }
    }

    fn read(&self) -> RwLockReadGuard<'_, Box<dyn Cache>> {
        self.cache.read()
    }

    fn write(&self) -> RwLockWriteGuard<'_, Box<dyn Cache>> {
        self.cache.write()
    }
}

/// Does memoization of different objects for the immediate mode UI.
pub struct Memoizers {
    /// The store for which these caches are caching data.
    store_id: StoreId,

    /// Master map from cache type to the cache itself.
    ///
    /// The master lock is only held briefly to look up or insert a cache.
    /// Each cache has its own lock for actual access.
    caches: RwLock<HashMap<TypeId, Arc<SharedCache>>>,

    /// How much memory we used after the last call to [`Self::purge_memory`].
    memory_use_after_last_purge: u64,
}

impl Memoizers {
    /// Creates a new instance of [`Memoizers`] associated with a specific store.
    pub fn new(store_id: StoreId) -> Self {
        Self {
            caches: RwLock::new(HashMap::default()),
            store_id,
            memory_use_after_last_purge: 0,
        }
    }

    /// The store for which these caches are caching data.
    pub fn store_id(&self) -> &StoreId {
        &self.store_id
    }

    /// Call once per frame to potentially flush the cache(s).
    pub fn begin_frame(&self) {
        re_tracing::profile_function!();

        #[expect(clippy::iter_over_hash_type)] // order doesn't matter here
        for cache in self.caches.read().values() {
            re_tracing::profile_scope!(cache.name);
            cache.write().begin_frame();
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
            .read()
            .values()
            .map(|cache| (cache.name, cache.read().vram_usage()))
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
        for cache in self.caches.read().values() {
            re_tracing::profile_scope!(cache.name);
            cache.write().purge_memory();
        }

        self.memory_use_after_last_purge = self.capture_mem_usage_tree().size_bytes();
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
        for cache in self.caches.read().values() {
            re_tracing::profile_scope!(cache.name);
            cache.write().on_store_events(&relevant_events, entity_db);
        }
    }

    /// Gets or creates a shared cache for the given type.
    fn shared_cache_entry<C: Cache + Default>(&self) -> Arc<SharedCache> {
        let caches = {
            re_tracing::profile_wait!("master-cache-read-lock");
            self.caches.upgradable_read()
        };

        let type_id = TypeId::of::<C>();
        if let Some(cache) = caches.get(&type_id) {
            cache.clone()
        } else {
            let mut caches = {
                re_tracing::profile_wait!("master-cache-upgrade-lock");
                caches.upgrade()
            };
            caches
                .entry(type_id)
                .or_insert_with(|| Arc::new(SharedCache::new::<C>()))
                .clone()
        }
    }

    /// Accesses a cache for reading and writing.
    ///
    /// Adds the cache lazily if it wasn't already there.
    pub fn entry<C: Cache + Default, R>(&self, f: impl FnOnce(&mut C) -> R) -> R {
        let shared_cache = self.shared_cache_entry::<C>();

        // Now lock only this specific cache.
        let mut cache_guard = {
            re_tracing::profile_wait!("cache-write-lock", shared_cache.name);
            shared_cache.write()
        };
        let cache = cache_guard.as_mut();
        f((cache as &mut dyn std::any::Any)
            .downcast_mut::<C>()
            .expect(
                "Downcast failed, this indicates a bug in how `Memoizers` adds new cache types.",
            ))
    }

    /// Accesses an existing cache for reading.
    ///
    /// Returns `None` if the cache has not been created yet.
    pub fn read<C: Cache, R>(&self, f: impl FnOnce(&C) -> R) -> Option<R> {
        let shared_cache = {
            re_tracing::profile_wait!("master-cache-read-lock");
            let guard = self.caches.read();
            guard.get(&TypeId::of::<C>()).cloned()
        }?;

        let cache_guard = {
            re_tracing::profile_wait!("cache-read-lock", shared_cache.name);
            shared_cache.read()
        };
        let cache = cache_guard.as_ref();
        Some(f((cache as &dyn std::any::Any).downcast_ref::<C>().expect(
            "Downcast failed, this indicates a bug in how `Memoizers` adds new cache types.",
        )))
    }

    /// Tries to read an existing memoization cache entry, then computes it through mutable access on miss.
    ///
    /// Use this if you're working with init-only cache entries, expect your cache entry to be usually present
    /// and want to avoid the overhead of a write lock.
    /// Note that this _adds_ overhead for the miss path compared to `memoizer`, so don't use this if you expect many misses!
    /// (UI code typically doesn't need to care about this optimization, since it's usually single-threaded already.)
    pub fn read_or_compute<C: CacheEntryAccess<Key, Value> + Default, Key, Value>(
        &self,
        key: &Key,
    ) -> Value {
        let cache_entry = self.shared_cache_entry::<C>();

        let cache = {
            re_tracing::profile_wait!("cache-read-lock");
            cache_entry.cache.upgradable_read()
        };

        let cache_accessor = (cache.as_ref() as &dyn std::any::Any)
            .downcast_ref::<C>()
            .expect(
                "Downcast failed, this indicates a bug in how `Memoizers` adds new cache types.",
            );

        if let Some(value) = cache_accessor.read(key) {
            value
        } else {
            let mut cache = {
                re_tracing::profile_wait!("cache-upgrade-lock");
                cache.upgrade()
            };
            let cache_accessor = (cache.as_mut() as &mut dyn std::any::Any).downcast_mut::<C>().expect(
                "Downcast failed, this indicates a bug in how `Memoizers` adds new cache types.",
            );
            cache_accessor.compute(key)
        }
    }
}

impl MemUsageTreeCapture for Memoizers {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        re_tracing::profile_function!();

        let mut node = re_byte_size::MemUsageNode::new();

        let mut cache_trees: Vec<_> = self
            .caches
            .read()
            .values()
            .map(|cache| (cache.name, cache.read().capture_mem_usage_tree()))
            .collect();
        cache_trees.sort_by_key(|(cache_name, _)| *cache_name);

        for (cache_name, tree) in cache_trees {
            node.add(cache_name, tree);
        }

        node.into_tree()
    }
}
