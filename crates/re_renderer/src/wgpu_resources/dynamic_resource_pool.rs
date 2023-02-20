use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::Debug,
    hash::Hash,
    sync::Arc,
};

use slotmap::{Key, SecondaryMap, SlotMap};

use smallvec::{smallvec, SmallVec};

use super::resource::PoolError;

pub trait DynamicResourcesDesc {
    fn resource_size_in_bytes(&self) -> u64;

    /// If true, a unused resources will be kept around for while and then re-used in following frames.
    /// If false, it will be destroyed on [`DynamicResourcePool::begin_frame`].
    fn allow_reuse(&self) -> bool;
}

/// Generic resource pool for all resources that have varying contents beyond their description.
///
/// Unlike in [`super::static_resource_pool::StaticResourcePool`], a resource is not uniquely identified by its description.
pub(super) struct DynamicResourcePool<Handle: Key, Desc, Res> {
    /// All known resources of this type.
    resources: SlotMap<Handle, (Desc, Res)>,

    /// Handles to all alive resources.
    /// We story any ref counted handle we give out in [`DynamicResourcePool::alloc`] here in order to keep it alive.
    /// Every [`DynamicResourcePool::begin_frame`] we check if the pool is now the only owner of the handle
    /// and if so mark it as deallocated.
    /// Being a [`SecondaryMap`] allows us to upgrade "weak" handles to strong handles,
    /// something required by [`super::GpuBindGroupPool`]
    alive_handles: SecondaryMap<Handle, Arc<Handle>>,

    /// Any resource that has been deallocated last frame.
    /// We keep them around for a bit longer to allow reclamation
    last_frame_deallocated: HashMap<Desc, SmallVec<[Arc<Handle>; 4]>>,

    current_frame_index: u64,
    total_resource_size_in_bytes: u64,
}

/// We cannot #derive(Default) as that would require Handle/Desc/Res to implement Default too.
impl<Handle: Key, Desc, Res> Default for DynamicResourcePool<Handle, Desc, Res> {
    fn default() -> Self {
        Self {
            resources: Default::default(),
            alive_handles: Default::default(),
            last_frame_deallocated: Default::default(),
            current_frame_index: Default::default(),
            total_resource_size_in_bytes: 0,
        }
    }
}

impl<Handle, Desc, Res> DynamicResourcePool<Handle, Desc, Res>
where
    Handle: Key,
    Desc: Clone + Eq + Hash + Debug + DynamicResourcesDesc,
{
    fn alloc_internal<F: FnOnce(&Desc) -> Res>(
        &mut self,
        desc: &Desc,
        creation_func: F,
    ) -> Arc<Handle> {
        // First check if we can reclaim a resource we have around from a previous frame.
        if desc.allow_reuse() {
            if let Entry::Occupied(mut entry) = self.last_frame_deallocated.entry(desc.clone()) {
                re_log::trace!(?desc, "Reclaimed previously discarded resource",);

                let handle = entry.get_mut().pop().unwrap();
                if entry.get().is_empty() {
                    entry.remove();
                }
                return handle;
            }
        }

        // Otherwise create a new resource
        let resource = creation_func(desc);
        self.total_resource_size_in_bytes += desc.resource_size_in_bytes();
        Arc::new(self.resources.insert((desc.clone(), resource)))
    }

    pub fn alloc<F: FnOnce(&Desc) -> Res>(&mut self, desc: &Desc, creation_func: F) -> Arc<Handle> {
        let handle = self.alloc_internal(desc, creation_func);
        self.alive_handles.insert(*handle, Arc::clone(&handle));
        handle
    }

    pub fn get_resource(&self, handle: Handle) -> Result<&Res, PoolError> {
        self.resources
            .get(handle)
            .map(|(_, resource)| resource)
            .ok_or_else(|| {
                if handle.is_null() {
                    PoolError::NullHandle
                } else {
                    PoolError::ResourceNotAvailable
                }
            })
    }

    pub fn begin_frame(&mut self, frame_index: u64, mut on_destroy_resource: impl FnMut(&Res)) {
        self.current_frame_index = frame_index;

        // Throw out any resources that we haven't reclaimed last frame.
        for (desc, handles) in self.last_frame_deallocated.drain() {
            re_log::trace!(
                count = handles.len(),
                ?desc,
                "Drained dangling resources from last frame",
            );
            for handle in handles {
                if let Some((desc, res)) = self.resources.remove(*handle) {
                    on_destroy_resource(&res);
                    self.total_resource_size_in_bytes -= desc.resource_size_in_bytes();
                }
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

                // If allowed, put it on the "last frame deallocated" list instead of destroying the resource immediately.
                if desc.allow_reuse() {
                    match self.last_frame_deallocated.entry(desc.clone()) {
                        Entry::Occupied(mut e) => {
                            e.get_mut().push(Arc::clone(strong_handle));
                        }
                        Entry::Vacant(e) => {
                            e.insert(smallvec![Arc::clone(strong_handle)]);
                        }
                    }
                } else if let Some((desc, res)) = self.resources.remove(handle) {
                    on_destroy_resource(&res);
                    self.total_resource_size_in_bytes -= desc.resource_size_in_bytes();
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

    pub fn num_resources(&self) -> usize {
        self.resources.len()
    }

    pub fn total_resource_size_in_bytes(&self) -> u64 {
        self.total_resource_size_in_bytes
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

    use super::{DynamicResourcePool, DynamicResourcesDesc};
    use crate::wgpu_resources::resource::PoolError;

    slotmap::new_key_type! { pub struct ConcreteHandle; }

    #[derive(Clone, PartialEq, Eq, Hash, Debug)]
    pub struct ConcreteResourceDesc(u32);

    impl DynamicResourcesDesc for ConcreteResourceDesc {
        fn resource_size_in_bytes(&self) -> u64 {
            1
        }

        fn allow_reuse(&self) -> bool {
            true
        }
    }

    #[derive(Debug)]
    pub struct ConcreteResource {
        id: u32,
        drop_counter: Arc<AtomicUsize>,
    }

    impl Drop for ConcreteResource {
        fn drop(&mut self) {
            self.drop_counter.fetch_add(1, Ordering::Release);
        }
    }

    type Pool = DynamicResourcePool<ConcreteHandle, ConcreteResourceDesc, ConcreteResource>;

    #[test]
    fn resource_alloc_and_reuse() {
        let mut pool = Pool::default();
        let drop_counter = Arc::new(AtomicUsize::new(0));

        let initial_resource_descs = [0, 0, 1, 2, 2, 3];

        // Alloc on a new pool always returns a new resource.
        allocate_resources(&initial_resource_descs, &mut pool, true, &drop_counter);

        // After frame maintenance we get used resources.
        // Still, no resources were dropped.
        {
            let drop_counter_before = drop_counter.load(Ordering::Acquire);
            let mut called_destroy = false;
            pool.begin_frame(1, |_| called_destroy = true);

            assert!(!called_destroy);
            assert_eq!(drop_counter_before, drop_counter.load(Ordering::Acquire),);
        }

        // Allocate the same resources again, this should *not* create any new resources.
        allocate_resources(&initial_resource_descs, &mut pool, false, &drop_counter);
        // Doing it again, it will again create resources.
        allocate_resources(&initial_resource_descs, &mut pool, true, &drop_counter);

        // Doing frame maintenance twice will drop all resources
        {
            let drop_counter_before = drop_counter.load(Ordering::Acquire);
            let mut called_destroy = false;
            pool.begin_frame(2, |_| called_destroy = true);
            assert!(!called_destroy);
            pool.begin_frame(3, |_| called_destroy = true);
            assert!(called_destroy);
            let drop_counter_now = drop_counter.load(Ordering::Acquire);
            assert_eq!(
                drop_counter_before + initial_resource_descs.len() * 2,
                drop_counter_now
            );
            assert_eq!(pool.total_resource_size_in_bytes(), 0);
        }

        // Holding on to a handle avoids both re-use and dropping.
        {
            let drop_counter_before = drop_counter.load(Ordering::Acquire);
            let handle0 = pool.alloc(&ConcreteResourceDesc(0), |d| ConcreteResource {
                id: d.0,
                drop_counter: drop_counter.clone(),
            });
            let handle1 = pool.alloc(&ConcreteResourceDesc(0), |d| ConcreteResource {
                id: d.0,
                drop_counter: drop_counter.clone(),
            });
            assert_ne!(handle0, handle1);
            drop(handle1);

            let mut called_destroy = false;
            pool.begin_frame(4, |_| called_destroy = true);
            assert!(!called_destroy);
            assert_eq!(drop_counter_before, drop_counter.load(Ordering::Acquire),);
            pool.begin_frame(5, |_| called_destroy = true);
            assert!(called_destroy);
            assert_eq!(
                drop_counter_before + 1,
                drop_counter.load(Ordering::Acquire),
            );
        }
    }

    // TODO: Add test for resources without re-use

    fn allocate_resources(
        descs: &[u32],
        pool: &mut DynamicResourcePool<ConcreteHandle, ConcreteResourceDesc, ConcreteResource>,
        expect_allocation: bool,
        drop_counter: &Arc<AtomicUsize>,
    ) {
        let drop_counter_before = drop_counter.load(Ordering::Acquire);
        let byte_count_before = pool.total_resource_size_in_bytes();
        for &desc in descs {
            // Previous loop iteration didn't drop Resources despite dropping a handle.
            assert_eq!(drop_counter_before, drop_counter.load(Ordering::Acquire));

            let new_resource_created = Cell::new(false);
            let handle = pool.alloc(&ConcreteResourceDesc(desc), |d| {
                new_resource_created.set(true);
                ConcreteResource {
                    id: d.0,
                    drop_counter: drop_counter.clone(),
                }
            });
            assert_eq!(new_resource_created.get(), expect_allocation);

            // Resource pool keeps the handle alive, but otherwise we're the only owners.
            assert_eq!(Arc::strong_count(&handle), 2);
        }

        if expect_allocation {
            assert_eq!(
                byte_count_before
                    + descs
                        .iter()
                        .map(|d| ConcreteResourceDesc(*d).resource_size_in_bytes())
                        .sum::<u64>(),
                pool.total_resource_size_in_bytes()
            );
        } else {
            assert_eq!(byte_count_before, pool.total_resource_size_in_bytes());
        }
    }

    #[test]
    fn get_resource() {
        let mut pool = Pool::default();
        let drop_counter = Arc::new(AtomicUsize::new(0));

        // Query with valid handle
        let handle = pool.alloc(&ConcreteResourceDesc(0), |d| ConcreteResource {
            id: d.0,
            drop_counter: drop_counter.clone(),
        });
        assert!(pool.get_resource(*handle).is_ok());
        assert!(matches!(
            *pool.get_resource(*handle).unwrap(),
            ConcreteResource {
                id: 0,
                drop_counter: _
            }
        ));

        // Query with null handle
        assert!(matches!(
            pool.get_resource(ConcreteHandle::null()),
            Err(PoolError::NullHandle)
        ));

        // Query with invalid handle
        let inner_handle = *handle;
        drop(handle);
        pool.begin_frame(0, |_| {});
        pool.begin_frame(1, |_| {});
        assert!(matches!(
            pool.get_resource(inner_handle),
            Err(PoolError::ResourceNotAvailable)
        ));
    }
}
