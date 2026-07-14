use re_byte_size::MemUsageTree;
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;

/// A cache for memoizing things in order to speed up immediate mode UI & other immediate mode style things.
///
/// Caches are stored in [`crate::Memoizers`], and each [`crate::Memoizers`] instance belongs to a
/// single [`re_log_types::StoreId`]. This means cache implementations are already scoped to one
/// store (recording or blueprint) and must not include the store id in their internal keys.
///
/// Cache implementations may still need finer-grained keys, such as [`crate::ViewId`], entity paths,
/// timelines, or query ranges. In particular, view-related caches should explicitly decide whether
/// their data is truly per-view (`ViewId` key needed) or can be shared across all views of the same
/// store (`ViewId` key not needed).
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
}

/// Trait for [`Cache`]es that are internally a list of key-value pairs that are computed once
/// and can be trivially returned without holding the lock.
///
/// Implementing this is required for [`crate::Memoizers::read_or_compute`].
pub trait CacheEntryAccess<Key, Value>: Cache {
    /// Reads the cache entry for the given key, if it exists.
    fn read(&self, key: &Key) -> Option<Value>;

    /// Computes the cache entry for the given key and returns it.
    ///
    /// While we generally expect this to be called only ever once for a given key,
    /// in high contended situations it may be called repeatedly for the same key.
    /// Implementations *have* to handle this gracefully.
    fn compute(&mut self, key: &Key) -> Value;
}
