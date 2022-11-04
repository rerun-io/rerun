use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::Debug,
    hash::Hash,
    sync::Arc,
};

use slotmap::{Key, SecondaryMap, SlotMap};

use smallvec::{smallvec, SmallVec};

use super::resource::*;

/// Generic resource pool for all resources that have varying contents beyond their description.
///
/// Unlike in [`super::static_resource_pool::StaticResourcePool`], a resource is not uniquely identified by its description.
pub(super) struct DynamicResourcePool<Handle: Key, Desc, Res> {
    /// All known resources of this type.
    resources: SlotMap<Handle, (Desc, Res)>,

    /// Handles to all alive resources.
    /// We story any ref counted handle we give out in [`DynamicResourcePool::alloc`] here in order to keep it alive.
    /// Every [`DynamicResourcePool::frame_maintenance`] we check if the pool is now the only owner of the handle
    /// and if so mark it as deallocated.
    /// Being a [`SecondaryMap`] allows us to upgrade "weak" handles to strong handles,
    /// something required by [`super::BindGroupPool`]
    alive_handles: SecondaryMap<Handle, Arc<Handle>>,

    /// Any resource that has been deallocated last frame.
    /// We keep them around for a bit longer to allow reclamation
    last_frame_deallocated: HashMap<Desc, SmallVec<[Arc<Handle>; 4]>>,

    current_frame_index: u64,
}

/// We cannot #derive(Default) as that would require Handle/Desc/Res to implement Default too.
impl<Handle: Key, Desc, Res> Default for DynamicResourcePool<Handle, Desc, Res> {
    fn default() -> Self {
        Self {
            resources: Default::default(),
            alive_handles: Default::default(),
            last_frame_deallocated: Default::default(),
            current_frame_index: Default::default(),
        }
    }
}

impl<Handle, Desc, Res> DynamicResourcePool<Handle, Desc, Res>
where
    Handle: Key,
    Desc: Clone + Eq + Hash + Debug,
    Res: GpuResource,
{
    pub fn alloc<F: FnOnce(&Desc) -> Res>(&mut self, desc: &Desc, creation_func: F) -> Arc<Handle> {
        // First check if we can reclaim a resource we have around from a previous frame.
        let handle =
            if let Entry::Occupied(mut entry) = self.last_frame_deallocated.entry(desc.clone()) {
                re_log::trace!(?desc, "Reclaimed previously discarded resource",);

                let handle = entry.get_mut().pop().unwrap();
                if entry.get().is_empty() {
                    entry.remove();
                }
                handle
            // Otherwise create a new resource
            } else {
                let resource = creation_func(desc);
                Arc::new(self.resources.insert((desc.clone(), resource)))
            };

        self.alive_handles.insert(*handle, Arc::clone(&handle));
        handle
    }

    pub fn get_resource(&self, handle: Handle) -> Result<&Res, PoolError> {
        self.resources
            .get(handle)
            .map(|(_, resource)| {
                resource.on_handle_resolve(self.current_frame_index);
                resource
            })
            .ok_or_else(|| {
                if handle.is_null() {
                    PoolError::NullHandle
                } else {
                    PoolError::ResourceNotAvailable
                }
            })
    }

    pub fn frame_maintenance(&mut self, frame_index: u64) {
        self.current_frame_index = frame_index;

        // Throw out any resources that we haven't reclaimed last frame.
        for (desc, handles) in self.last_frame_deallocated.drain() {
            re_log::debug!(
                count = handles.len(),
                ?desc,
                "Drained dangling resources from last frame",
            );
            for handle in handles {
                self.resources.remove(*handle);
            }
        }

        // If the strong count went down to 1, we must be the only ones holding on to handle.
        //
        // thread safety:
        // Since the count is pushed from 1 to 2 by `alloc`, it should not be possible to ever
        // get temporarily get back down to 1 without dropping the last user available copy of the Arc<Handle>.
        self.alive_handles.retain(|handle, strong_handle| {
            if Arc::strong_count(strong_handle) == 1 {
                let desc = &self.resources[handle].0;
                match self.last_frame_deallocated.entry(desc.clone()) {
                    Entry::Occupied(mut e) => {
                        e.get_mut().push(Arc::clone(strong_handle));
                    }
                    Entry::Vacant(e) => {
                        e.insert(smallvec![Arc::clone(strong_handle)]);
                    }
                }
                false
            } else {
                true
            }
        });
    }

    /// Upgrades a "weak" handle to a reference counted handle by looking it up.
    /// Returns a reference in order to avoid needlessly increasing the ref-count.
    pub(super) fn get_strong_handle(&self, handle: Handle) -> &Arc<Handle> {
        &self.alive_handles[handle]
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };

    use slotmap::Key;

    use super::DynamicResourcePool;
    use crate::resource_pools::resource::{GpuResource, PoolError};

    slotmap::new_key_type! { pub struct ConcreteHandle; }

    #[derive(Clone, PartialEq, Eq, Hash, Debug)]
    pub struct ConcreteResourceDesc(u32);

    #[derive(PartialEq, Eq, Debug)]
    pub struct ConcreteResource(u32);

    static RESOURCE_DROP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    impl Drop for ConcreteResource {
        fn drop(&mut self) {
            RESOURCE_DROP_COUNTER.fetch_add(1, Ordering::Relaxed);
        }
    }

    impl GpuResource for ConcreteResource {
        fn on_handle_resolve(&self, _current_frame_index: u64) {}
    }

    type Pool = DynamicResourcePool<ConcreteHandle, ConcreteResourceDesc, ConcreteResource>;

    #[test]
    fn resource_alloc_and_reuse() {
        let mut pool = Pool::default();

        let initial_resource_descs = [0, 0, 1, 2, 2, 3];

        // Alloc on a new pool always returns a new resource.
        allocate_resources(&initial_resource_descs, &mut pool, true);

        // After frame maintenance we get used resources.
        // Still, no resources were dropped.
        {
            let drop_counter_before = RESOURCE_DROP_COUNTER.load(Ordering::Relaxed);
            pool.frame_maintenance(1);

            assert_eq!(
                drop_counter_before,
                RESOURCE_DROP_COUNTER.load(Ordering::Relaxed),
            );
        }

        // Allocate the same resources again, this should *not* create any new resources.
        allocate_resources(&initial_resource_descs, &mut pool, false);
        // Doing it again, it will again create resources.
        allocate_resources(&initial_resource_descs, &mut pool, true);

        // Doing frame maintenance twice will drop all resources
        {
            let drop_counter_before = RESOURCE_DROP_COUNTER.load(Ordering::Relaxed);
            pool.frame_maintenance(2);
            pool.frame_maintenance(3);
            let drop_counter_now = RESOURCE_DROP_COUNTER.load(Ordering::Relaxed);
            assert_eq!(
                drop_counter_before + initial_resource_descs.len() * 2,
                drop_counter_now
            );
        }

        // Holding on to a handle avoids both re-use and dropping.
        {
            let drop_counter_before = RESOURCE_DROP_COUNTER.load(Ordering::Relaxed);
            let handle0 = pool.alloc(&ConcreteResourceDesc(0), |d| ConcreteResource(d.0));
            let handle1 = pool.alloc(&ConcreteResourceDesc(0), |d| ConcreteResource(d.0));
            assert_ne!(handle0, handle1);
            drop(handle1);

            pool.frame_maintenance(4);
            assert_eq!(
                drop_counter_before,
                RESOURCE_DROP_COUNTER.load(Ordering::Relaxed),
            );
            pool.frame_maintenance(5);
            assert_eq!(
                drop_counter_before + 1,
                RESOURCE_DROP_COUNTER.load(Ordering::Relaxed),
            );
        }
    }

    fn allocate_resources(
        descs: &[u32],
        pool: &mut DynamicResourcePool<ConcreteHandle, ConcreteResourceDesc, ConcreteResource>,
        expect_allocation: bool,
    ) {
        let drop_counter_before = RESOURCE_DROP_COUNTER.load(Ordering::Relaxed);
        for &desc in descs {
            // Previous loop iteration didn't drop Resources despite dropping a handle.
            assert_eq!(
                drop_counter_before,
                RESOURCE_DROP_COUNTER.load(Ordering::Relaxed),
            );

            let new_resource_created = Cell::new(false);
            let handle = pool.alloc(&ConcreteResourceDesc(desc), |d| {
                new_resource_created.set(true);
                ConcreteResource(d.0)
            });
            assert_eq!(new_resource_created.get(), expect_allocation);

            // Resource pool keeps the handle alive, but otherwise we're the only owners.
            assert_eq!(Arc::strong_count(&handle), 2);
        }
    }

    #[test]
    fn get_resource() {
        let mut pool = Pool::default();

        // Query with valid handle
        let handle = pool.alloc(&ConcreteResourceDesc(0), |d| ConcreteResource(d.0));
        assert!(pool.get_resource(*handle).is_ok());
        assert_eq!(*pool.get_resource(*handle).unwrap(), ConcreteResource(0));

        // Query with null handle
        assert_eq!(
            pool.get_resource(ConcreteHandle::null()),
            Err(PoolError::NullHandle)
        );

        // Query with invalid handle
        let inner_handle = *handle;
        drop(handle);
        pool.frame_maintenance(0);
        pool.frame_maintenance(1);
        assert_eq!(
            pool.get_resource(inner_handle),
            Err(PoolError::ResourceNotAvailable)
        );
    }
}
