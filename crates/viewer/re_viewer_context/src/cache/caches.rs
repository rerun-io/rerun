use std::any::{Any, TypeId};

use ahash::HashMap;
use parking_lot::Mutex;
use re_chunk_store::ChunkStoreEvent;
use re_log_types::StoreId;

/// Does memoization of different objects for the immediate mode UI.
pub struct Caches {
    caches: Mutex<HashMap<TypeId, Box<dyn Cache>>>,
    store_id: StoreId,
}

impl Caches {
    /// Creates a new instance of `Caches` associated with a specific store.
    pub fn new(store_id: StoreId) -> Self {
        Self {
            caches: Mutex::new(HashMap::default()),
            store_id,
        }
    }

    /// Call a function with a reference to the caches map.
    pub fn with_caches<R>(&self, f: impl FnOnce(&HashMap<TypeId, Box<dyn Cache>>) -> R) -> R {
        let guard = self.caches.lock();

        f(&guard)
    }

    /// Call once per frame to potentially flush the cache(s).
    pub fn begin_frame(&self) {
        re_tracing::profile_function!();

        #[expect(clippy::iter_over_hash_type)]
        for cache in self.caches.lock().values_mut() {
            cache.begin_frame();
        }
    }

    pub fn memory_reports(&self) -> HashMap<&'static str, CacheMemoryReport> {
        self.caches
            .lock()
            .values()
            .map(|cache| (cache.name(), cache.memory_report()))
            .collect()
    }

    /// Attempt to free up memory.
    pub fn purge_memory(&self) {
        re_tracing::profile_function!();

        #[expect(clippy::iter_over_hash_type)]
        for cache in self.caches.lock().values_mut() {
            cache.purge_memory();
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

        #[expect(clippy::iter_over_hash_type)]
        for cache in self.caches.lock().values_mut() {
            cache.on_store_events(&relevant_events);
        }
    }

    /// Accesses a cache for reading and writing.
    ///
    /// Adds the cache lazily if it wasn't already there.
    pub fn entry<C: Cache + Default, R>(&self, f: impl FnOnce(&mut C) -> R) -> R {
        #[allow(clippy::unwrap_or_default)] // or_default doesn't work here.
        f(self
            .caches
            .lock()
            .entry(TypeId::of::<C>())
            .or_insert(Box::<C>::default())
            .as_any_mut()
            .downcast_mut::<C>()
            .expect("Downcast failed, this indicates a bug in how `Caches` adds new cache types."))
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

    /// Converts itself to a mutable reference of [`Any`], which enables mutable downcasting to concrete types.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
