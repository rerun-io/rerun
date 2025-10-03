use std::{
    any::{Any, TypeId},
    sync::Arc,
};

use ahash::HashMap;
use parking_lot::{
    ArcRwLockReadGuard, MappedRwLockReadGuard, Mutex, RawRwLock, RwLock, RwLockReadGuard,
    RwLockWriteGuard,
};
use re_chunk_store::ChunkStoreEvent;
use re_log_types::StoreId;

/// Various "ad-hoc" caches that are used for a given store.
pub struct Caches {
    caches: RwLock<HashMap<TypeId, Mutex<Box<dyn Cache>>>>,
    store_id: StoreId,
}

impl Caches {
    /// Creates a new instance of `Caches` associated with a specific store.
    pub fn new(store_id: StoreId) -> Self {
        Self {
            caches: RwLock::new(HashMap::default()),
            store_id,
        }
    }

    /// Call once per frame to potentially flush the cache(s).
    pub fn begin_frame(&self) {
        re_tracing::profile_function!();

        #[expect(clippy::iter_over_hash_type)]
        for cache in self.caches.read().values() {
            cache.write().begin_frame();
        }
    }

    pub fn memory_reports(&self) -> HashMap<&'static str, CacheMemoryReport> {
        self.caches
            .read()
            .values()
            .map(|cache| {
                let cache_read_locked = cache.read();
                (cache_read_locked.name(), cache_read_locked.memory_report())
            })
            .collect()
    }

    /// Attempt to free up memory.
    pub fn purge_memory(&self) {
        re_tracing::profile_function!();

        #[expect(clippy::iter_over_hash_type)]
        for cache in self.caches.read().values() {
            cache.write().purge_memory();
        }
    }

    /// React to the chunk store's changelog, if needed.
    ///
    /// Useful to e.g. invalidate unreachable data.
    pub fn on_store_events(&self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        let relevant_events = events
            .iter()
            .filter(|event| event.store_id == self.store_id)
            .collect::<Vec<_>>();
        if relevant_events.is_empty() {
            return;
        }

        // TODO:
        // TODO: par_iter
        // #[expect(clippy::iter_over_hash_type)]
        // for cache in self.caches.read().values_mut() {
        //     cache.on_store_events(&relevant_events);
        // }
    }

    /// Retrieve cache entry of a given type.
    ///
    /// Adds the cache lazily if it wasn't already there.
    fn get_or_create_entry<C: Cache + Default>(
        &self,
    ) -> MappedRwLockReadGuard<'_, Mutex<Box<dyn Cache>>> {
        // Almost always, the entry already exists.
        // Only if it doesn't, we have to read lock the map.
        // Given that we think this is VERY rare, it's also fine to hold that read lock for a bit longer.
        {
            let caches_read_locked = self.caches.read();

            if let Ok(mapped_rw_lock_read_guard) =
                RwLockReadGuard::try_map(caches_read_locked, |caches| {
                    caches.get(&TypeId::of::<C>())
                })
            {
                return mapped_rw_lock_read_guard;
            }
        }

        // Add the entry if needed.
        // Note that by now someone else might have added it.
        let mut caches_write_locked = self.caches.write();
        caches_write_locked
            .entry(TypeId::of::<C>())
            .or_insert_with(|| Mutex::new(Box::new(C::default())));
        self.get_or_create_entry::<C>()
    }

    /// Accesses a cache for reading and writing.
    ///
    /// Adds the cache lazily if it wasn't already there.
    pub fn entry<C: Cache + Default, R>(&self, f: impl FnOnce(&mut C) -> R) -> R {
        let cache = self.get_or_create_entry::<C>();
        let mut locked_cache = cache.lock();
        let typed_cache = locked_cache
            .as_any_mut()
            .downcast_mut::<C>()
            .expect("Downcast failed, this indicates a bug in how `Caches` adds new cache types.");
        f(typed_cache)
    }
}

/// Memory usage information of a single cache-item.
pub struct CacheMemoryReportItem {
    pub item_name: String,
    pub bytes_cpu: u64,
    pub bytes_gpu: Option<u64>,
}

/// A report of how much memory a certain cache is using, used for
/// debugging memory usage in the memory panel.
pub struct CacheMemoryReport {
    pub bytes_cpu: u64,
    pub bytes_gpu: Option<u64>,

    /// Memory information per cache-item.
    pub per_cache_item_info: Vec<CacheMemoryReportItem>,
}

/// A cache for memoizing things in order to speed up immediate mode UI & other immediate mode style things.
///
/// See also egus's cache system, in [`egui::cache`] (<https://docs.rs/egui/latest/egui/cache/index.html>).
pub trait Cache: std::any::Any + Send + Sync {
    /// Called once per frame to potentially flush the cache.
    fn begin_frame(&mut self) {}

    /// Attempt to free up memory.
    fn purge_memory(&mut self);

    fn name(&self) -> &'static str;

    /// Construct a [`CacheMemoryReport`] for this cache.
    fn memory_report(&self) -> CacheMemoryReport;

    /// React to the chunk store's changelog, if needed.
    ///
    /// Useful to e.g. invalidate unreachable data.
    /// Since caches are created per store, each cache consistently receives events only for the same store.
    fn on_store_events(&mut self, events: &[&ChunkStoreEvent]) {
        _ = events;
    }

    /// Converts itself to a reference of [`Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn Any;

    /// Converts itself to a mutable reference of [`Any`], which enables mutable downcasting to concrete types.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
