use std::hash::Hash;

use ahash::HashMap;
use re_mutex::Mutex;

/// Keeps values that were accessed during the previous frame.
pub struct FrameRetainedCache<Key, Value> {
    inner: Mutex<FrameRetainedCacheInner<Key, Value>>,
}

struct FrameRetainedCacheInner<Key, Value> {
    entries: HashMap<Key, CacheEntry<Value>>,
}

struct CacheEntry<Value> {
    value: Value,
    accessed: bool,
}

impl<Key, Value> Default for FrameRetainedCache<Key, Value> {
    fn default() -> Self {
        Self {
            inner: Mutex::new(FrameRetainedCacheInner {
                entries: HashMap::default(),
            }),
        }
    }
}

impl<Key: Eq + Hash, Value: Clone> FrameRetainedCache<Key, Value> {
    /// Returns the value for `key`, creating it on a cache miss.
    pub fn get_or_try_create_with<Err>(
        &self,
        key: Key,
        create: impl FnOnce() -> Result<Value, Err>,
    ) -> Result<Value, Err> {
        let mut inner = self.inner.lock();
        match inner.entries.entry(key) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().accessed = true;
                Ok(entry.get().value.clone())
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                let value = create()?;
                entry.insert(CacheEntry {
                    value: value.clone(),
                    accessed: true,
                });
                Ok(value)
            }
        }
    }

    /// Drops values that were not accessed since the previous frame boundary.
    pub fn begin_frame(&self) {
        let mut inner = self.inner.lock();
        inner.entries.retain(|_, entry| {
            let retain = entry.accessed;
            entry.accessed = false;
            retain
        });
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::FrameRetainedCache;

    #[test]
    fn reuses_accessed_entries() {
        let cache = FrameRetainedCache::<u64, u64>::default();
        let creations = Cell::new(0);
        let create = || {
            creations.set(creations.get() + 1);
            Ok::<_, ()>(42)
        };

        assert_eq!(cache.get_or_try_create_with(1, create), Ok(42));
        cache.begin_frame();
        assert_eq!(cache.get_or_try_create_with(1, create), Ok(42));
        cache.begin_frame();
        assert_eq!(creations.get(), 1);
    }

    #[test]
    fn evicts_after_one_frame_without_access() {
        let cache = FrameRetainedCache::<u64, u64>::default();
        let creations = Cell::new(0);
        let create = || {
            creations.set(creations.get() + 1);
            Ok::<_, ()>(42)
        };

        assert_eq!(cache.get_or_try_create_with(1, create), Ok(42));
        cache.begin_frame();
        cache.begin_frame();
        assert_eq!(cache.get_or_try_create_with(1, create), Ok(42));
        assert_eq!(creations.get(), 2);
    }
}
