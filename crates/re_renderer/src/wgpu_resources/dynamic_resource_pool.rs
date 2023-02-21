use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::Debug,
    hash::Hash,
    sync::{atomic::AtomicU64, Arc},
};

use parking_lot::RwLock;
use slotmap::{Key, SlotMap};

use smallvec::{smallvec, SmallVec};

use super::resource::PoolError;

pub trait SizedResourceDesc {
    fn resource_size_in_bytes(&self) -> u64;
}

pub struct DynamicResource<Handle, Desc, Res> {
    pub inner: Res,
    pub creation_desc: Desc,
    pub handle: Handle,
}

impl<Handle, Desc, Res> std::ops::Deref for DynamicResource<Handle, Desc, Res> {
    type Target = Res;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// Resources are held as Option so its easier to move them out.
type AliveResourceMap<Handle, Desc, Res> =
    SlotMap<Handle, Option<Arc<DynamicResource<Handle, Desc, Res>>>>;

struct DynamicResourcePoolProtectedState<Handle: Key, Desc, Res> {
    /// Handles to all alive resources.
    /// We story any ref counted handle we give out in [`DynamicResourcePool::alloc`] here in order to keep it alive.
    /// Every [`DynamicResourcePool::begin_frame`] we check if the pool is now the only owner of the handle
    /// and if so mark it as deallocated.
    /// Being a [`SecondaryMap`] allows us to upgrade "weak" handles to strong handles,
    /// something required by [`super::GpuBindGroupPool`]
    alive_resources: AliveResourceMap<Handle, Desc, Res>,

    /// Any resource that has been deallocated last frame.
    /// We keep them around for a bit longer to allow reclamation
    last_frame_deallocated: HashMap<Desc, SmallVec<[Res; 4]>>,
}

/// Generic resource pool for all resources that have varying contents beyond their description.
///
/// Unlike in [`super::static_resource_pool::StaticResourcePool`], a resource is not uniquely identified by its description.
pub(super) struct DynamicResourcePool<Handle: Key, Desc, Res> {
    state: RwLock<DynamicResourcePoolProtectedState<Handle, Desc, Res>>,

    current_frame_index: u64,
    total_resource_size_in_bytes: AtomicU64,
}

/// We cannot #derive(Default) as that would require Handle/Desc/Res to implement Default too.
impl<Handle: Key, Desc, Res> Default for DynamicResourcePool<Handle, Desc, Res> {
    fn default() -> Self {
        Self {
            state: RwLock::new(DynamicResourcePoolProtectedState {
                alive_resources: Default::default(),
                last_frame_deallocated: Default::default(),
            }),
            current_frame_index: Default::default(),
            total_resource_size_in_bytes: AtomicU64::new(0),
        }
    }
}

impl<Handle, Desc, Res> DynamicResourcePool<Handle, Desc, Res>
where
    Handle: Key,
    Desc: Clone + Eq + Hash + Debug + SizedResourceDesc,
{
    pub fn alloc<F: FnOnce(&Desc) -> Res>(
        &self,
        desc: &Desc,
        creation_func: F,
    ) -> Arc<DynamicResource<Handle, Desc, Res>> {
        let mut state = self.state.write();

        // First check if we can reclaim a resource we have around from a previous frame.
        let inner_resource =
            if let Entry::Occupied(mut entry) = state.last_frame_deallocated.entry(desc.clone()) {
                re_log::trace!(?desc, "Reclaimed previously discarded resource",);

                let handle = entry.get_mut().pop().unwrap();
                if entry.get().is_empty() {
                    entry.remove();
                }
                handle
            // Otherwise create a new resource
            } else {
                let resource = creation_func(desc);
                self.total_resource_size_in_bytes.fetch_add(
                    desc.resource_size_in_bytes(),
                    std::sync::atomic::Ordering::Relaxed,
                );
                resource
            };

        let handle = state.alive_resources.insert_with_key(|handle| {
            Some(Arc::new(DynamicResource {
                inner: inner_resource,
                creation_desc: desc.clone(),
                handle,
            }))
        });

        state.alive_resources[handle].as_ref().unwrap().clone()
    }

    pub fn get_from_handle(
        &self,
        handle: Handle,
    ) -> Result<Arc<DynamicResource<Handle, Desc, Res>>, PoolError> {
        self.state
            .read()
            .alive_resources
            .get(handle)
            .map(|resource| resource.as_ref().unwrap().clone())
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
        let state = self.state.get_mut();

        // Throw out any resources that we haven't reclaimed last frame.
        for (desc, resources) in state.last_frame_deallocated.drain() {
            re_log::trace!(
                count = resources.len(),
                ?desc,
                "Drained dangling resources from last frame",
            );
            for resource in resources {
                on_destroy_resource(&resource);
                self.total_resource_size_in_bytes.fetch_sub(
                    desc.resource_size_in_bytes(),
                    std::sync::atomic::Ordering::Relaxed,
                );
            }
        }

        // If the strong count went down to 1, we must be the only ones holding on to handle.
        //
        // thread safety:
        // Since the count is pushed from 1 to 2 by `alloc`, it should not be possible to ever
        // get temporarily get back down to 1 without dropping the last user available copy of the Arc<Handle>.
        state.alive_resources.retain(|_, resource| {
            let resolved = resource
                .take()
                .expect("Alive resources should never be None");

            match Arc::try_unwrap(resolved) {
                Ok(r) => {
                    match state.last_frame_deallocated.entry(r.creation_desc) {
                        Entry::Occupied(mut e) => {
                            e.get_mut().push(r.inner);
                        }
                        Entry::Vacant(e) => {
                            e.insert(smallvec![r.inner]);
                        }
                    }
                    false
                }
                Err(r) => {
                    *resource = Some(r);
                    true
                }
            }
        });
    }

    pub fn num_resources(&self) -> usize {
        let state = self.state.read();
        state.alive_resources.len() + state.last_frame_deallocated.values().flatten().count()
    }

    pub fn total_resource_size_in_bytes(&self) -> u64 {
        self.total_resource_size_in_bytes
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

// #[cfg(test)]
// mod tests {
//     use std::{
//         cell::Cell,
//         sync::{
//             atomic::{AtomicUsize, Ordering},
//             Arc,
//         },
//     };

//     use slotmap::Key;

//     use super::{DynamicResourcePool, SizedResourceDesc};
//     use crate::wgpu_resources::resource::PoolError;

//     slotmap::new_key_type! { pub struct ConcreteHandle; }

//     #[derive(Clone, PartialEq, Eq, Hash, Debug)]
//     pub struct ConcreteResourceDesc(u32);

//     impl SizedResourceDesc for ConcreteResourceDesc {
//         fn resource_size_in_bytes(&self) -> u64 {
//             1
//         }
//     }

//     #[derive(Debug)]
//     pub struct ConcreteResource {
//         id: u32,
//         drop_counter: Arc<AtomicUsize>,
//     }

//     impl Drop for ConcreteResource {
//         fn drop(&mut self) {
//             self.drop_counter.fetch_add(1, Ordering::Release);
//         }
//     }

//     type Pool = DynamicResourcePool<ConcreteHandle, ConcreteResourceDesc, ConcreteResource>;

//     #[test]
//     fn resource_alloc_and_reuse() {
//         let mut pool = Pool::default();
//         let drop_counter = Arc::new(AtomicUsize::new(0));

//         let initial_resource_descs = [0, 0, 1, 2, 2, 3];

//         // Alloc on a new pool always returns a new resource.
//         allocate_resources(&initial_resource_descs, &mut pool, true, &drop_counter);

//         // After frame maintenance we get used resources.
//         // Still, no resources were dropped.
//         {
//             let drop_counter_before = drop_counter.load(Ordering::Acquire);
//             let mut called_destroy = false;
//             pool.begin_frame(1, |_| called_destroy = true);

//             assert!(!called_destroy);
//             assert_eq!(drop_counter_before, drop_counter.load(Ordering::Acquire),);
//         }

//         // Allocate the same resources again, this should *not* create any new resources.
//         allocate_resources(&initial_resource_descs, &mut pool, false, &drop_counter);
//         // Doing it again, it will again create resources.
//         allocate_resources(&initial_resource_descs, &mut pool, true, &drop_counter);

//         // Doing frame maintenance twice will drop all resources
//         {
//             let drop_counter_before = drop_counter.load(Ordering::Acquire);
//             let mut called_destroy = false;
//             pool.begin_frame(2, |_| called_destroy = true);
//             assert!(!called_destroy);
//             pool.begin_frame(3, |_| called_destroy = true);
//             assert!(called_destroy);
//             let drop_counter_now = drop_counter.load(Ordering::Acquire);
//             assert_eq!(
//                 drop_counter_before + initial_resource_descs.len() * 2,
//                 drop_counter_now
//             );
//             assert_eq!(pool.total_resource_size_in_bytes(), 0);
//         }

//         // Holding on to a handle avoids both re-use and dropping.
//         {
//             let drop_counter_before = drop_counter.load(Ordering::Acquire);
//             let handle0 = pool.alloc(&ConcreteResourceDesc(0), |d| ConcreteResource {
//                 id: d.0,
//                 drop_counter: drop_counter.clone(),
//             });
//             let handle1 = pool.alloc(&ConcreteResourceDesc(0), |d| ConcreteResource {
//                 id: d.0,
//                 drop_counter: drop_counter.clone(),
//             });
//             assert_ne!(handle0, handle1);
//             drop(handle1);

//             let mut called_destroy = false;
//             pool.begin_frame(4, |_| called_destroy = true);
//             assert!(!called_destroy);
//             assert_eq!(drop_counter_before, drop_counter.load(Ordering::Acquire),);
//             pool.begin_frame(5, |_| called_destroy = true);
//             assert!(called_destroy);
//             assert_eq!(
//                 drop_counter_before + 1,
//                 drop_counter.load(Ordering::Acquire),
//             );
//         }
//     }

//     fn allocate_resources(
//         descs: &[u32],
//         pool: &mut DynamicResourcePool<ConcreteHandle, ConcreteResourceDesc, ConcreteResource>,
//         expect_allocation: bool,
//         drop_counter: &Arc<AtomicUsize>,
//     ) {
//         let drop_counter_before = drop_counter.load(Ordering::Acquire);
//         let byte_count_before = pool.total_resource_size_in_bytes();
//         for &desc in descs {
//             // Previous loop iteration didn't drop Resources despite dropping a handle.
//             assert_eq!(drop_counter_before, drop_counter.load(Ordering::Acquire));

//             let new_resource_created = Cell::new(false);
//             let handle = pool.alloc(&ConcreteResourceDesc(desc), |d| {
//                 new_resource_created.set(true);
//                 ConcreteResource {
//                     id: d.0,
//                     drop_counter: drop_counter.clone(),
//                 }
//             });
//             assert_eq!(new_resource_created.get(), expect_allocation);

//             // Resource pool keeps the handle alive, but otherwise we're the only owners.
//             assert_eq!(Arc::strong_count(&handle), 2);
//         }

//         if expect_allocation {
//             assert_eq!(
//                 byte_count_before
//                     + descs
//                         .iter()
//                         .map(|d| ConcreteResourceDesc(*d).resource_size_in_bytes())
//                         .sum::<u64>(),
//                 pool.total_resource_size_in_bytes()
//             );
//         } else {
//             assert_eq!(byte_count_before, pool.total_resource_size_in_bytes());
//         }
//     }

//     #[test]
//     fn get_resource() {
//         let mut pool = Pool::default();
//         let drop_counter = Arc::new(AtomicUsize::new(0));

//         // Query with valid handle
//         let handle = pool.alloc(&ConcreteResourceDesc(0), |d| ConcreteResource {
//             id: d.0,
//             drop_counter: drop_counter.clone(),
//         });
//         assert!(pool.get_resource(*handle).is_ok());
//         assert!(matches!(
//             *pool.get_resource(*handle).unwrap(),
//             ConcreteResource {
//                 id: 0,
//                 drop_counter: _
//             }
//         ));

//         // Query with null handle
//         assert!(matches!(
//             pool.get_resource(ConcreteHandle::null()),
//             Err(PoolError::NullHandle)
//         ));

//         // Query with invalid handle
//         let inner_handle = *handle;
//         drop(handle);
//         pool.begin_frame(0, |_| {});
//         pool.begin_frame(1, |_| {});
//         assert!(matches!(
//             pool.get_resource(inner_handle),
//             Err(PoolError::ResourceNotAvailable)
//         ));
//     }
// }
