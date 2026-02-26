use std::collections::HashMap;

use re_chunk_store::{ChunkStoreHandle, ChunkStoreHandleWeak};
use re_tuid::Tuid;

/// Opaque identifier for a store slot in the [`StorePool`].
///
/// Used in `memory:///store/{store_slot_id}` URLs to make stores globally resolvable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct StoreSlotId(Tuid);

impl Default for StoreSlotId {
    fn default() -> Self {
        Self(Tuid::new())
    }
}

impl StoreSlotId {
    pub fn new() -> Self {
        Self::default()
    }
}

impl std::fmt::Display for StoreSlotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::str::FromStr for StoreSlotId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Tuid::from_str(s).map(Self)
    }
}

/// A lookup index of [`ChunkStoreHandle`]s keyed by [`StoreSlotId`].
///
/// The pool holds **weak** references. The strong (owning) references live in
/// [`Layer`](super::Layer)s. When all layers drop a store, the weak entry
/// expires naturally. Call [`StorePool::cleanup`] to sweep expired entries.
#[derive(Default)]
pub struct StorePool {
    stores: HashMap<StoreSlotId, ChunkStoreHandleWeak>,
}

impl StorePool {
    /// Register a store, returning its new [`StoreSlotId`].
    pub fn register(&mut self, handle: &ChunkStoreHandle) -> StoreSlotId {
        let id = StoreSlotId::new();
        self.stores.insert(id, handle.downgrade());
        id
    }

    /// Register under an existing ID (e.g. for `memory://` re-registration).
    pub fn register_with_id(&mut self, id: StoreSlotId, handle: &ChunkStoreHandle) {
        self.stores.insert(id, handle.downgrade());
    }

    /// Resolve by upgrading the `Weak`. Returns `None` if expired or unknown.
    pub fn get(&self, id: &StoreSlotId) -> Option<ChunkStoreHandle> {
        let weak = self.stores.get(id)?;
        weak.upgrade()
    }

    /// Drop expired weak entries.
    pub fn cleanup(&mut self) {
        self.stores.retain(|_, weak| weak.upgrade().is_some());
    }
}

#[cfg(test)]
mod tests {
    use re_chunk_store::{ChunkStore, ChunkStoreHandle};
    use re_log_types::{StoreId, StoreKind};

    use super::*;

    fn test_store_handle() -> ChunkStoreHandle {
        let store_id = StoreId::new(StoreKind::Recording, "test", "test");
        let config = re_chunk_store::ChunkStoreConfig::CHANGELOG_DISABLED;
        ChunkStoreHandle::new(ChunkStore::new(store_id, config))
    }

    #[test]
    fn store_slot_id_display_from_str_roundtrip() {
        let id = StoreSlotId::new();
        let s = id.to_string();
        let parsed: StoreSlotId = s.parse().expect("should parse");
        assert_eq!(id, parsed);
    }

    #[test]
    fn register_and_get() {
        let mut pool = StorePool::default();
        let handle = test_store_handle();
        let id = pool.register(&handle);

        let retrieved = pool.get(&id).expect("should find store");
        assert!(std::ptr::eq(
            std::ptr::from_ref(&*handle.read()),
            std::ptr::from_ref(&*retrieved.read())
        ));
    }

    #[test]
    fn get_returns_none_after_drop() {
        let mut pool = StorePool::default();
        let handle = test_store_handle();
        let id = pool.register(&handle);
        drop(handle);
        assert!(pool.get(&id).is_none(), "should be expired");
    }

    #[test]
    fn cleanup_removes_expired() {
        let mut pool = StorePool::default();
        let handle = test_store_handle();
        let _ = pool.register(&handle);
        drop(handle);
        pool.cleanup();
        assert!(pool.stores.is_empty(), "should have been cleaned up");
    }

    #[test]
    fn cleanup_keeps_alive() {
        let mut pool = StorePool::default();
        let handle = test_store_handle();
        let id = pool.register(&handle);
        pool.cleanup();
        assert!(pool.get(&id).is_some(), "should NOT have been cleaned up");
    }

    #[test]
    fn register_with_id() {
        let mut pool = StorePool::default();
        let handle = test_store_handle();
        let id = StoreSlotId::new();
        pool.register_with_id(id, &handle);

        let retrieved = pool.get(&id).expect("should find store");
        assert!(std::ptr::eq(
            std::ptr::from_ref(&*handle.read()),
            std::ptr::from_ref(&*retrieved.read())
        ));
    }

    #[test]
    fn get_nonexistent() {
        let pool = StorePool::default();
        let id = StoreSlotId::new();
        assert!(pool.get(&id).is_none());
    }
}
