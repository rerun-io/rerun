use parking_lot::{ArcRwLockReadGuard, RwLockReadGuard};
use re_chunk_store::{ChunkStore, ChunkStoreHandle};

use crate::{QueryCache, QueryCacheHandle};

// ---

// TODO(cmc): This whole business should really be defined elsewhere, but for now this the best we
// have, and it's really not worth adding yet another crate just for this.

/// Anything that can expose references to a [`ChunkStore`] and its [`QueryCache`].
///
/// Used to abstract over [`StorageEngine`] and its different types of guards, such as [`StorageEngineArcReadGuard`].
pub trait StorageEngineLike {
    fn with<F: FnOnce(&ChunkStore, &QueryCache) -> R, R>(&self, f: F) -> R;

    fn try_with<F: FnOnce(&ChunkStore, &QueryCache) -> R, R>(&self, f: F) -> Option<R> {
        Some(self.with(f))
    }
}

/// Keeps track of handles towards a [`ChunkStore`] and its [`QueryCache`].
///
/// A [`StorageEngine`] doesn't add any feature on top of what [`ChunkStoreHandle`] and
/// [`QueryCacheHandle`] already offer: the job of the [`StorageEngine`] is to leverage the type
/// system in order to protect against deadlocks and race conditions at compile time.
///
/// The handles stored within will never be publicly accessible past construction.
///
/// The underlying [`ChunkStore`] and [`QueryCache`] can be accessed through one of the
/// following methods:
/// * [`StorageEngine::read`]
/// * [`StorageEngine::read_arc`]
/// * [`StorageEngine::write`]
/// * [`StorageEngine::write_arc`]
#[derive(Clone)]
pub struct StorageEngine {
    store: ChunkStoreHandle,
    cache: QueryCacheHandle,
}

impl StorageEngineLike for StorageEngine {
    #[inline]
    fn with<F: FnOnce(&ChunkStore, &QueryCache) -> R, R>(&self, f: F) -> R {
        let this = self.read();
        f(this.store(), this.cache())
    }

    #[inline]
    fn try_with<F: FnOnce(&ChunkStore, &QueryCache) -> R, R>(&self, f: F) -> Option<R> {
        let this = self.try_read()?;
        Some(f(this.store(), this.cache()))
    }
}

impl StorageEngine {
    /// Creates a new [`StorageEngine`] with the specified [`ChunkStore`] and [`QueryCache`] handles.
    ///
    /// # Safety
    ///
    /// It is the responsibility of the caller to make sure that the given handles have not escaped
    /// anywhere else before constructing this type, otherwise the [`StorageEngine`] cannot make
    /// any safety guarantees.
    #[inline]
    #[expect(unsafe_code)]
    pub unsafe fn new(store: ChunkStoreHandle, cache: QueryCacheHandle) -> Self {
        Self { store, cache }
    }
}

impl StorageEngine {
    #[inline]
    pub fn read(&self) -> StorageEngineReadGuard<'_> {
        StorageEngineReadGuard {
            cache: self.cache.read(),
            store: self.store.read(),
        }
    }

    #[inline]
    pub fn try_read(&self) -> Option<StorageEngineReadGuard<'_>> {
        let cache = self.cache.try_read()?;
        let store = self.store.try_read()?;
        Some(StorageEngineReadGuard { store, cache })
    }

    #[inline]
    pub fn try_read_arc(&self) -> Option<StorageEngineArcReadGuard> {
        let cache = self.cache.try_read_arc()?;
        let store = self.store.try_read_arc()?;
        Some(StorageEngineArcReadGuard { store, cache })
    }

    #[inline]
    pub fn write(&self) -> StorageEngineWriteGuard<'_> {
        StorageEngineWriteGuard {
            cache: self.cache.write(),
            store: self.store.write(),
        }
    }

    #[inline]
    pub fn try_write(&self) -> Option<StorageEngineWriteGuard<'_>> {
        let cache = self.cache.try_write()?;
        let store = self.store.try_write()?;
        Some(StorageEngineWriteGuard { store, cache })
    }

    #[inline]
    pub fn read_arc(&self) -> StorageEngineArcReadGuard {
        StorageEngineArcReadGuard {
            cache: self.cache.read_arc(),
            store: self.store.read_arc(),
        }
    }

    #[inline]
    pub fn write_arc(&self) -> StorageEngineArcWriteGuard {
        StorageEngineArcWriteGuard {
            cache: self.cache.write_arc(),
            store: self.store.write_arc(),
        }
    }

    #[inline]
    pub fn try_write_arc(&self) -> Option<StorageEngineArcWriteGuard> {
        let cache = self.cache.try_write_arc()?;
        let store = self.store.try_write_arc()?;
        Some(StorageEngineArcWriteGuard { store, cache })
    }
}

// --- Read Guards ---

// NOTE: None of these fields should ever be publicly exposed, either directly or through a method,
// as it is always possible to go back to an actual `RwLock` via `RwLockReadGuard::rwlock`.
// Doing so would defeat the deadlock protection that the `StorageEngine` offers.
// Exposing references to the actual `ChunkStore` and `QueryCache` if ofc fine.
pub struct StorageEngineReadGuard<'a> {
    store: parking_lot::RwLockReadGuard<'a, ChunkStore>,
    cache: parking_lot::RwLockReadGuard<'a, QueryCache>,
}

impl Clone for StorageEngineReadGuard<'_> {
    // Cloning the guard is safe, since the lock stays locked all along.
    fn clone(&self) -> Self {
        Self {
            store: parking_lot::RwLock::read(RwLockReadGuard::rwlock(&self.store)),
            cache: parking_lot::RwLock::read(RwLockReadGuard::rwlock(&self.cache)),
        }
    }
}

impl StorageEngineReadGuard<'_> {
    #[inline]
    pub fn store(&self) -> &ChunkStore {
        &self.store
    }

    #[inline]
    pub fn cache(&self) -> &QueryCache {
        &self.cache
    }
}

impl StorageEngineLike for StorageEngineReadGuard<'_> {
    #[inline]
    fn with<F: FnOnce(&ChunkStore, &QueryCache) -> R, R>(&self, f: F) -> R {
        f(self.store(), self.cache())
    }
}

impl re_byte_size::SizeBytes for StorageEngineReadGuard<'_> {
    fn heap_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();
        let Self { store, cache } = self;
        store.heap_size_bytes() + cache.heap_size_bytes()
    }
}

impl re_byte_size::MemUsageTreeCapture for StorageEngineReadGuard<'_> {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_tracing::profile_function!();
        let Self { store, cache } = self;
        re_byte_size::MemUsageNode::new()
            .with_child("ChunkStore", store.capture_mem_usage_tree())
            .with_child("QueryCache", cache.capture_mem_usage_tree())
            .into_tree()
    }
}

// NOTE: None of these fields should ever be publicly exposed, either directly or through a method,
// as it is always possible to go back to an actual `RwLock` via `ArcRwLockReadGuard::rwlock`.
// Doing so would defeat the deadlock protection that the `StorageEngine` offers.
// Exposing references to the actual `ChunkStore` and `QueryCache` if ofc fine.
pub struct StorageEngineArcReadGuard {
    store: parking_lot::ArcRwLockReadGuard<parking_lot::RawRwLock, ChunkStore>,
    cache: parking_lot::ArcRwLockReadGuard<parking_lot::RawRwLock, QueryCache>,
}

impl StorageEngineArcReadGuard {
    #[inline]
    pub fn store(&self) -> &ChunkStore {
        &self.store
    }

    #[inline]
    pub fn cache(&self) -> &QueryCache {
        &self.cache
    }
}

impl StorageEngineLike for StorageEngineArcReadGuard {
    #[inline]
    fn with<F: FnOnce(&ChunkStore, &QueryCache) -> R, R>(&self, f: F) -> R {
        f(self.store(), self.cache())
    }
}

impl Clone for StorageEngineArcReadGuard {
    // Cloning the guard is safe, since the lock stays locked all along.
    fn clone(&self) -> Self {
        Self {
            store: parking_lot::RwLock::read_arc(ArcRwLockReadGuard::rwlock(&self.store)),
            cache: parking_lot::RwLock::read_arc(ArcRwLockReadGuard::rwlock(&self.cache)),
        }
    }
}

// --- Write Guards ---

// NOTE: None of these fields should ever be publicly exposed, either directly or through a method,
// as it is always possible to go back to an actual `RwLock` via `RwLockWriteGuard::rwlock`.
// Doing so would defeat the deadlock protection that the `StorageEngine` offers.
// Exposing references to the actual `ChunkStore` and `QueryCache` if ofc fine.
pub struct StorageEngineWriteGuard<'a> {
    store: parking_lot::RwLockWriteGuard<'a, ChunkStore>,
    cache: parking_lot::RwLockWriteGuard<'a, QueryCache>,
}

impl<'a> StorageEngineWriteGuard<'a> {
    #[inline]
    pub fn downgrade(self) -> StorageEngineReadGuard<'a> {
        StorageEngineReadGuard {
            store: parking_lot::RwLockWriteGuard::downgrade(self.store),
            cache: parking_lot::RwLockWriteGuard::downgrade(self.cache),
        }
    }
}

impl StorageEngineWriteGuard<'_> {
    #[inline]
    pub fn store(&mut self) -> &mut ChunkStore {
        &mut self.store
    }

    #[inline]
    pub fn cache(&mut self) -> &mut QueryCache {
        &mut self.cache
    }
}

// NOTE: None of these fields should ever be publicly exposed, either directly or through a method,
// as it is always possible to go back to an actual `RwLock` via `ArcRwLockWriteGuard::rwlock`.
// Doing so would defeat the deadlock protection that the `StorageEngine` offers.
// Exposing references to the actual `ChunkStore` and `QueryCache` if ofc fine.
pub struct StorageEngineArcWriteGuard {
    store: parking_lot::ArcRwLockWriteGuard<parking_lot::RawRwLock, ChunkStore>,
    cache: parking_lot::ArcRwLockWriteGuard<parking_lot::RawRwLock, QueryCache>,
}

impl StorageEngineArcWriteGuard {
    #[inline]
    pub fn downgrade(self) -> StorageEngineArcReadGuard {
        StorageEngineArcReadGuard {
            store: parking_lot::ArcRwLockWriteGuard::downgrade(self.store),
            cache: parking_lot::ArcRwLockWriteGuard::downgrade(self.cache),
        }
    }
}

impl StorageEngineArcWriteGuard {
    #[inline]
    pub fn store(&mut self) -> &mut ChunkStore {
        &mut self.store
    }

    #[inline]
    pub fn cache(&mut self) -> &mut QueryCache {
        &mut self.cache
    }
}
