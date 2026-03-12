use re_byte_size::MemUsageTree;
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;

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
}
