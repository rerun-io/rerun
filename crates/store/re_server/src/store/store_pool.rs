use std::collections::HashMap;

use re_tuid::Tuid;

use super::ResolvedStore;
use super::resolved_store::ResolvedStoreWeak;

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

/// A lookup index of [`ResolvedStore`]s keyed by [`StoreSlotId`].
///
/// The pool holds **weak** references. The strong (owning) references live in
/// [`Layer`](super::Layer)s. When all layers drop a store, the weak entry
/// expires naturally. Call [`StorePool::cleanup`] to sweep expired entries.
#[derive(Default)]
pub struct StorePool {
    stores: HashMap<StoreSlotId, ResolvedStoreWeak>,
}

impl StorePool {
    /// Register a store, returning its new [`StoreSlotId`].
    pub fn register(&mut self, resolved: &ResolvedStore) -> StoreSlotId {
        let id = StoreSlotId::new();
        self.stores.insert(id, resolved.downgrade());
        id
    }

    /// Register under an existing ID (e.g. for `memory://` re-registration).
    pub fn register_with_id(&mut self, id: StoreSlotId, resolved: &ResolvedStore) {
        self.stores.insert(id, resolved.downgrade());
    }

    /// Resolve by upgrading the `Weak`. Returns `None` if expired or unknown.
    pub fn get(&self, id: &StoreSlotId) -> Option<ResolvedStore> {
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

    fn test_resolved_store() -> ResolvedStore {
        let store_id = StoreId::new(StoreKind::Recording, "test", "test");
        let config = re_chunk_store::ChunkStoreConfig::CHANGELOG_DISABLED;
        ResolvedStore::Eager(ChunkStoreHandle::new(ChunkStore::new(store_id, config)))
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
        let resolved = test_resolved_store();
        let id = pool.register(&resolved);

        let retrieved = pool.get(&id).expect("should find store");
        assert_eq!(resolved.store_id(), retrieved.store_id());
    }

    #[test]
    fn get_returns_none_after_drop() {
        let mut pool = StorePool::default();
        let resolved = test_resolved_store();
        let id = pool.register(&resolved);
        drop(resolved);
        assert!(pool.get(&id).is_none(), "should be expired");
    }

    #[test]
    fn cleanup_removes_expired() {
        let mut pool = StorePool::default();
        let resolved = test_resolved_store();
        let _ = pool.register(&resolved);
        drop(resolved);
        pool.cleanup();
        assert!(pool.stores.is_empty(), "should have been cleaned up");
    }

    #[test]
    fn cleanup_keeps_alive() {
        let mut pool = StorePool::default();
        let resolved = test_resolved_store();
        let id = pool.register(&resolved);
        pool.cleanup();
        assert!(pool.get(&id).is_some(), "should NOT have been cleaned up");
    }

    #[test]
    fn register_with_id() {
        let mut pool = StorePool::default();
        let resolved = test_resolved_store();
        let id = StoreSlotId::new();
        pool.register_with_id(id, &resolved);

        let retrieved = pool.get(&id).expect("should find store");
        assert_eq!(resolved.store_id(), retrieved.store_id());
    }

    #[test]
    fn get_nonexistent() {
        let pool = StorePool::default();
        let id = StoreSlotId::new();
        assert!(pool.get(&id).is_none());
    }
}
