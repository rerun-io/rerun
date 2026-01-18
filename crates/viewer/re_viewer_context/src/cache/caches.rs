use std::any::TypeId;

use ahash::HashMap;
use parking_lot::Mutex;
use re_byte_size::{MemUsageTree, MemUsageTreeCapture};
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_log_types::StoreId;

/// Does memoization of different objects for the immediate mode UI.
pub struct Caches {
    caches: Mutex<HashMap<TypeId, Box<dyn Cache>>>,

    /// The store for which these caches are caching data.
    pub store_id: StoreId,
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
        re_tracing::profile_function!();
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
    pub fn on_rrd_manifest(&self, entity_db: &EntityDb) {
        re_tracing::profile_function!();

        if self.store_id != *entity_db.store_id() {
            return;
        }

        #[expect(clippy::iter_over_hash_type)]
        for cache in self.caches.lock().values_mut() {
            cache.on_rrd_manifest(entity_db);
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

        #[expect(clippy::iter_over_hash_type)]
        for cache in self.caches.lock().values_mut() {
            cache.on_store_events(&relevant_events, entity_db);
        }
    }

    /// Accesses a cache for reading and writing.
    ///
    /// Adds the cache lazily if it wasn't already there.
    pub fn entry<C: Cache + Default, R>(&self, f: impl FnOnce(&mut C) -> R) -> R {
        let mut guard = self.caches.lock();
        let cache = guard
            .entry(TypeId::of::<C>())
            .or_insert_with(|| Box::<C>::default())
            .as_mut();
        f((cache as &mut dyn std::any::Any)
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

impl MemUsageTreeCapture for CacheMemoryReport {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        let Self {
            bytes_cpu,
            bytes_gpu,
            per_cache_item_info,
        } = self;

        let mut node = re_byte_size::MemUsageNode::new();
        node.add("cpu", MemUsageTree::Bytes(*bytes_cpu));
        if let Some(gpu_bytes) = bytes_gpu {
            node.add("gpu", MemUsageTree::Bytes(*gpu_bytes));
        }

        // Add per-item breakdown
        if !per_cache_item_info.is_empty() {
            for item in per_cache_item_info {
                let mut item_node = re_byte_size::MemUsageNode::new();
                item_node.add("cpu", MemUsageTree::Bytes(item.bytes_cpu));
                if let Some(gpu_bytes) = item.bytes_gpu {
                    item_node.add("gpu", MemUsageTree::Bytes(gpu_bytes));
                }
                node.add(item.item_name.clone(), item_node.into_tree());
            }
        }

        node.into_tree()
    }
}

impl MemUsageTreeCapture for Caches {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        re_tracing::profile_function!();

        let mut node = re_byte_size::MemUsageNode::new();

        let mut reports: Vec<_> = self.memory_reports().into_iter().collect();
        reports.sort_by_key(|(cache_name, _)| *cache_name);

        for (cache_name, report) in reports {
            node.add(cache_name, report.capture_mem_usage_tree());
        }

        node.into_tree()
    }
}
