use std::any::{Any, TypeId};

use ahash::HashMap;
use parking_lot::Mutex;

/// Does memoization of different objects for the immediate mode UI.
#[derive(Default)]
pub struct Caches(Mutex<HashMap<TypeId, Box<dyn Cache>>>);

impl Caches {
    /// Call once per frame to potentially flush the cache(s).
    pub fn begin_frame(&self) {
        re_tracing::profile_function!();
        for cache in self.0.lock().values_mut() {
            cache.begin_frame();
        }
    }

    /// Attempt to free up memory.
    pub fn purge_memory(&self) {
        re_tracing::profile_function!();
        for cache in self.0.lock().values_mut() {
            cache.purge_memory();
        }
    }

    /// Accesses a cache for reading and writing.
    ///
    /// Adds the cache lazily if it wasn't already there.
    pub fn entry<C: Cache + Default, R>(&self, f: impl FnOnce(&mut C) -> R) -> R {
        #[allow(clippy::unwrap_or_default)] // or_default doesn't work here.
        f(self
            .0
            .lock()
            .entry(TypeId::of::<C>())
            .or_insert(Box::<C>::default())
            .as_any_mut()
            .downcast_mut::<C>()
            .expect("Downcast failed, this indicates a bug in how `Caches` adds new cache types."))
    }
}

/// A cache for memoizing things in order to speed up immediate mode UI & other immediate mode style things.
pub trait Cache: std::any::Any {
    /// Called once per frame to potentially flush the cache.
    fn begin_frame(&mut self);

    /// Attempt to free up memory.
    fn purge_memory(&mut self);

    // TODO(andreas): Track bytes used for each cache and show in the memory panel!
    //fn bytes_used(&self) -> usize;

    /// Converts itself to a mutable reference of [`Any`], which enables mutable downcasting to concrete types.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
