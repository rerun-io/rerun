use std::any::Any;

use ahash::HashMap;

use re_log_types::component_types;

use super::TensorStats;

/// Does memoization of different things for the immediate mode UI.
#[derive(Default)]
pub struct Caches {
    tensor_stats: nohash_hasher::IntMap<component_types::TensorId, TensorStats>,

    caches: HashMap<std::any::TypeId, Box<dyn Cache>>,
}

impl Caches {
    /// Call once per frame to potentially flush the cache(s).
    pub fn begin_frame(&mut self) {
        for cache in self.caches.values_mut() {
            cache.begin_frame();
        }
    }

    /// Attempt to free up memory.
    pub fn purge_memory(&mut self) {
        let Self {
            tensor_stats,
            caches,
        } = self;
        tensor_stats.clear();

        for cache in caches.values_mut() {
            cache.purge_memory();
        }
    }

    pub fn tensor_stats(&mut self, tensor: &re_log_types::component_types::Tensor) -> &TensorStats {
        self.tensor_stats
            .entry(tensor.tensor_id)
            .or_insert_with(|| TensorStats::new(tensor))
    }

    /// Retrieves a cache for reading and writing.
    ///
    /// Adds the cache lazily if it wasn't already there.
    pub fn get_mut<T: Cache + Default>(&mut self) -> &mut T {
        let cache = self
            .caches
            .entry(std::any::TypeId::of::<T>())
            .or_insert(Box::<T>::default());

        cache
            .as_any_mut()
            .downcast_mut::<T>()
            .expect("Downcast failed, this indicates a bug in how `Caches` adds new cache types.")
    }
}

/// A cache for memoizing things in order to speed up immediate mode UI & other immediate mode style things.
pub trait Cache: std::any::Any {
    /// Called once per frame to potentially flush the cache.
    fn begin_frame(&mut self);

    /// Attempt to free up memory.
    fn purge_memory(&mut self);

    /// Converts itself to a mutable reference of [`Any`], which enables mutable downcasting to concrete types.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
