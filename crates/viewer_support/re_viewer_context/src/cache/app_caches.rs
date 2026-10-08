use re_byte_size::{MemUsageTree, MemUsageTreeCapture};

use super::Memoizers;
use crate::AppCache;

/// App-level caches for data that is not tied to any particular store.
///
/// Only caches implementing [`AppCache`] can be stored here.
#[derive(Default)]
pub struct AppCaches {
    memoizers: Memoizers,
}

impl AppCaches {
    /// Call once per frame to potentially flush the caches.
    pub fn begin_frame(&self) {
        re_tracing::profile_function!();

        self.memoizers.begin_frame();
    }

    /// Attempt to free up memory.
    ///
    /// Called BEFORE `begin_frame` (if at all).
    pub fn purge_memory(&mut self) {
        re_tracing::profile_function!();

        self.memoizers.purge_memory();
    }

    /// Accesses a memoization cache for reading and writing.
    ///
    /// Adds the cache lazily if it wasn't already there.
    pub fn memoizer<C: AppCache + Default, R>(&self, f: impl FnOnce(&mut C) -> R) -> R {
        self.memoizers.entry::<C, R>(f)
    }
}

impl MemUsageTreeCapture for AppCaches {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        self.memoizers.capture_mem_usage_tree()
    }
}
