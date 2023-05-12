use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::Debug,
    hash::Hash,
    sync::{atomic::AtomicU64, Arc},
};

use parking_lot::RwLock;
use slotmap::{Key, SlotMap};

use smallvec::SmallVec;

use super::resource::PoolError;

pub trait DynamicResourcesDesc {
    fn resource_size_in_bytes(&self) -> u64;

    /// If true, a unused resources will be kept around for while and then re-used in following frames.
    /// If false, it will be destroyed on [`DynamicResourcePool::begin_frame`].
    fn allow_reuse(&self) -> bool;
}

pub struct DynamicResource<Handle, Desc: Debug, Res> {
    pub inner: Res,
    pub creation_desc: Desc,
    pub handle: Handle,
}

impl<Handle, Desc, Res> std::ops::Deref for DynamicResource<Handle, Desc, Res>
where
    Desc: Debug,
{
    type Target = Res;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

struct DynamicResourcePoolProtectedState<Handle: Key, Desc: Debug, Res> {
    /// All resources, including both resources that are in use and those that are marked as dead via [`Self::last_frame_deallocated`]
    ///
    /// We store any ref counted handle we give out in [`DynamicResourcePool::alloc`] here in order to keep it alive.
    /// Every [`DynamicResourcePool::begin_frame`] we check if the pool is now the only owner of the handle
    /// and if so mark it as deallocated.
    all_resources: SlotMap<Handle, Arc<DynamicResource<Handle, Desc, Res>>>,

    /// Any resource that has been deallocated last frame.
    /// We keep them around for a bit longer to allow reclamation
    last_frame_deallocated: HashMap<Desc, SmallVec<[Handle; 4]>>,
}

/// Generic resource pool for all resources that have varying contents beyond their description.
///
/// Unlike in [`super::static_resource_pool::StaticResourcePool`], a resource can not be uniquely
/// identified by its description, as the same description can apply to several different resources.
pub(super) struct DynamicResourcePool<Handle: Key, Desc: Debug, Res> {
    state: RwLock<DynamicResourcePoolProtectedState<Handle, Desc, Res>>,

    current_frame_index: u64,
    total_resource_size_in_bytes: AtomicU64,
}

/// We cannot #derive(Default) as that would require Handle/Desc/Res to implement Default too.
impl<Handle: Key, Desc, Res> Default for DynamicResourcePool<Handle, Desc, Res>
where
    Desc: Debug,
{
    fn default() -> Self {
        Self {
            state: RwLock::new(DynamicResourcePoolProtectedState {
                all_resources: Default::default(),
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
    Desc: Clone + Eq + Hash + Debug + DynamicResourcesDesc,
{
    pub fn alloc<F: FnOnce(&Desc) -> Res>(
        &self,
        desc: &Desc,
        creation_func: F,
    ) -> Arc<DynamicResource<Handle, Desc, Res>> {
        crate::profile_function!();
        let mut state = self.state.write();

        // First check if we can reclaim a resource we have around from a previous frame.
        if desc.allow_reuse() {
            if let Entry::Occupied(mut entry) = state.last_frame_deallocated.entry(desc.clone()) {
                re_log::trace!(?desc, "Reclaimed previously discarded resource");

                let handle = entry.get_mut().pop().unwrap();
                if entry.get().is_empty() {
                    entry.remove();
                }

                return state.all_resources[handle].clone();
            }
        }

        // Otherwise create a new resource
        re_log::trace!(?desc, "Allocated new resource");
        let inner_resource = {
            crate::profile_scope!("creation_func");
            creation_func(desc)
        };
        self.total_resource_size_in_bytes.fetch_add(
            desc.resource_size_in_bytes(),
            std::sync::atomic::Ordering::Relaxed,
        );

        let handle = state.all_resources.insert_with_key(|handle| {
            Arc::new(DynamicResource {
                inner: inner_resource,
                creation_desc: desc.clone(),
                handle,
            })
        });

        state.all_resources[handle].clone()
    }

    pub fn get_from_handle(
        &self,
        handle: Handle,
    ) -> Result<Arc<DynamicResource<Handle, Desc, Res>>, PoolError> {
        self.state
            .read()
            .all_resources
            .get(handle)
            .cloned()
            .ok_or_else(|| {
                if handle.is_null() {
                    PoolError::NullHandle
                } else {
                    PoolError::ResourceNotAvailable
                }
            })
    }

    pub fn begin_frame(&mut self, frame_index: u64, mut on_destroy_resource: impl FnMut(&Res)) {
        crate::profile_function!();
        self.current_frame_index = frame_index;
        let state = self.state.get_mut();

        let update_stats = |creation_desc: &Desc| {
            self.total_resource_size_in_bytes.fetch_sub(
                creation_desc.resource_size_in_bytes(),
                std::sync::atomic::Ordering::Relaxed,
            );
        };

        // Throw out any resources that we haven't reclaimed last frame.
        for (desc, resources) in state.last_frame_deallocated.drain() {
            re_log::trace!(
                count = resources.len(),
                ?desc,
                "Drained dangling resources from last frame:",
            );
            for resource in resources {
                let Some(removed_resource) = state.all_resources.remove(resource) else {
                    debug_assert!(false, "a resource was marked as destroyed last frame that we no longer kept track of");
                    continue;
                };
                update_stats(&desc);
                on_destroy_resource(&removed_resource);
            }
        }

        // If the strong count went down to 1, we must be the only ones holding on to handle.
        // If that's the case, push it to the re-use-next-frame list or discard.
        //
        // thread safety:
        // Since the count is pushed from 1 to 2 by `alloc`, it should not be possible to ever
        // get temporarily get back down to 1 without dropping the last user available copy of the Arc<Handle>.
        state.all_resources.retain(|_, resource| {
            if Arc::strong_count(resource) == 1 {
                if resource.creation_desc.allow_reuse() {
                    state
                        .last_frame_deallocated
                        .entry(resource.creation_desc.clone())
                        .or_default()
                        .push(resource.handle);
                    true
                } else {
                    update_stats(&resource.creation_desc);
                    on_destroy_resource(&resource.inner);
                    false
                }
            } else {
                true
            }
        });
    }

    pub fn num_resources(&self) -> usize {
        self.state.read().all_resources.len()
    }

    pub fn total_resource_size_in_bytes(&self) -> u64 {
        self.total_resource_size_in_bytes
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl<Handle, Desc, Res> Drop for DynamicResourcePool<Handle, Desc, Res>
where
    Handle: Key,
    Desc: Debug,
{
    fn drop(&mut self) {
        // TODO(andreas): We're failing this check currently on re_viewer's shutdown.
        // This is primarily the case due to the way we store the render ctx itself and other things on egui-wgpu's paint callback resources
        // We shouldn't do this as it makes a whole lot of other things cumbersome. Instead, we should store it directly on the `App`
        // where we control the drop order.

        // for (_, alive_resource) in self.state.read().alive_resources.iter() {
        //     let alive_resource = alive_resource
        //         .as_ref()
        //         .expect("Alive resources should never be None");
        //     let ref_count = Arc::strong_count(alive_resource);

        //     assert!(ref_count == 1,
        //             "Resource has still {} owner(s) at the time of pool destruction. Description desc was {:?}",
        //             ref_count - 1,
        //             &alive_resource.creation_desc);
        // }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, sync::Arc};

    use super::{DynamicResourcePool, DynamicResourcesDesc};

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

    thread_local! {
        static DROP_COUNTER: Cell<usize> = Cell::new(0);
    }

    #[derive(Debug)]
    pub struct ConcreteResource;

    impl Drop for ConcreteResource {
        fn drop(&mut self) {
            DROP_COUNTER.with(|c| {
                c.set(c.get() + 1);
            });
        }
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
            let drop_counter_before = DROP_COUNTER.with(|c| c.get());
            let mut called_destroy = false;
            pool.begin_frame(1, |_| called_destroy = true);

            assert!(!called_destroy);
            assert_eq!(drop_counter_before, DROP_COUNTER.with(|c| c.get()),);
        }

        // Allocate the same resources again, this should *not* create any new resources.
        allocate_resources(&initial_resource_descs, &mut pool, false);
        // Doing it again, it will again create resources.
        allocate_resources(&initial_resource_descs, &mut pool, true);

        // Doing frame maintenance twice will drop all resources
        {
            let drop_counter_before = DROP_COUNTER.with(|c| c.get());
            let mut called_destroy = false;
            pool.begin_frame(2, |_| called_destroy = true);
            assert!(!called_destroy);
            pool.begin_frame(3, |_| called_destroy = true);
            assert!(called_destroy);
            let drop_counter_now = DROP_COUNTER.with(|c| c.get());
            assert_eq!(
                drop_counter_before + initial_resource_descs.len() * 2,
                drop_counter_now
            );
            assert_eq!(pool.total_resource_size_in_bytes(), 0);
        }

        // Holding on to the resource avoids both re-use and dropping.
        {
            let drop_counter_before = DROP_COUNTER.with(|c| c.get());
            let resource0 = pool.alloc(&ConcreteResourceDesc(0), |_| ConcreteResource);
            let resource1 = pool.alloc(&ConcreteResourceDesc(0), |_| ConcreteResource);
            assert_ne!(resource0.handle, resource1.handle);
            drop(resource1);

            let mut called_destroy = false;
            pool.begin_frame(4, |_| called_destroy = true);
            assert!(!called_destroy);
            assert_eq!(drop_counter_before, DROP_COUNTER.with(|c| c.get()),);
            pool.begin_frame(5, |_| called_destroy = true);
            assert!(called_destroy);
            assert_eq!(drop_counter_before + 1, DROP_COUNTER.with(|c| c.get()),);
        }
    }

    // Two resources have two different handles.
    #[test]
    fn individual_handles() {
        let mut pool = Pool::default();
        let res0 = pool.alloc(&ConcreteResourceDesc(0), |_| ConcreteResource);
        let res1 = pool.alloc(&ConcreteResourceDesc(0), |_| ConcreteResource);
        assert_ne!(res0.handle, res1.handle);
        pool.begin_frame(1234, |_| {});
    }

    // A resource gets the same handle when re-used.
    // (important for BindGroup re-use!)
    #[test]
    fn handle_unchanged_on_reuse() {
        let mut pool = Pool::default();
        let res0 = pool.alloc(&ConcreteResourceDesc(0), |_| ConcreteResource);
        let handle0 = res0.handle;
        drop(res0);
        pool.begin_frame(1234, |_| {});
        let res1 = pool.alloc(&ConcreteResourceDesc(0), |_| ConcreteResource);

        assert_eq!(handle0, res1.handle);
        pool.begin_frame(1235, |_| {});
    }

    fn allocate_resources(
        descs: &[u32],
        pool: &mut DynamicResourcePool<ConcreteHandle, ConcreteResourceDesc, ConcreteResource>,
        expect_allocation: bool,
    ) {
        let drop_counter_before = DROP_COUNTER.with(|c| c.get());
        let byte_count_before = pool.total_resource_size_in_bytes();
        for &desc in descs {
            // Previous loop iteration didn't drop Resources despite dropping a handle.
            assert_eq!(drop_counter_before, DROP_COUNTER.with(|c| c.get()));

            let new_resource_created = Cell::new(false);
            let resource = pool.alloc(&ConcreteResourceDesc(desc), |_| {
                new_resource_created.set(true);
                ConcreteResource
            });
            assert_eq!(new_resource_created.get(), expect_allocation);

            // Resource pool keeps the handle alive, but otherwise we're the only owners.
            assert_eq!(Arc::strong_count(&resource), 2);
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
}
