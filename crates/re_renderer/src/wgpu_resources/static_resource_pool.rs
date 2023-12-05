use std::{collections::HashMap, hash::Hash, ops::Deref};

use parking_lot::{RwLock, RwLockReadGuard};
use slotmap::{Key, SlotMap};

use super::resource::{PoolError, ResourceStatistics};

pub struct StoredResource<Res> {
    resource: Res,
    statistics: ResourceStatistics,
}

impl<Res> Deref for StoredResource<Res> {
    type Target = Res;

    fn deref(&self) -> &Self::Target {
        &self.resource
    }
}

/// Generic resource pool for all resources that are fully described upon creation.
///
/// This implies, a resource is uniquely defined by its description.
/// We call these resources "static" because they never change their content during rendering.
/// However, the description may be respect to indirect changes which may to recreation of a resource.
/// The prime example of this is shader reloading:
/// * The resource is semantically the exact same despite having a different wgpu resource.
/// * We do **not** want its handle to change.
pub(super) struct StaticResourcePool<Handle: Key, Desc, Res> {
    resources: RwLock<SlotMap<Handle, StoredResource<Res>>>,
    lookup: RwLock<HashMap<Desc, Handle>>,
    pub current_frame_index: u64,
}

/// We cannot #derive(Default) as that would require Handle/Desc/Res to implement Default too.
impl<Handle: Key, Desc, Res> Default for StaticResourcePool<Handle, Desc, Res> {
    fn default() -> Self {
        Self {
            resources: Default::default(),
            lookup: Default::default(),
            current_frame_index: Default::default(),
        }
    }
}

impl<Handle, Desc, Res> StaticResourcePool<Handle, Desc, Res>
where
    Handle: Key,
    Desc: std::fmt::Debug + Clone + Eq + Hash,
{
    pub fn get_or_create<F: FnOnce(&Desc) -> Res>(&self, desc: &Desc, creation_func: F) -> Handle {
        // Ensure the lock isn't held in the creation case.
        if let Some(handle) = self.lookup.read().get(desc) {
            return *handle;
        }

        re_tracing::profile_scope!("Creating new static resource", std::any::type_name::<Res>());

        let resource = creation_func(desc);
        let handle = self.resources.write().insert(StoredResource {
            resource,
            statistics: ResourceStatistics {
                frame_created: self.current_frame_index,
                last_frame_used: self.current_frame_index.into(),
            },
        });
        self.lookup.write().insert(desc.clone(), handle);

        handle
    }

    pub fn recreate_resources<F: FnMut(&Desc) -> Option<Res>>(&mut self, mut recreation_func: F) {
        re_tracing::profile_function!();

        for (desc, handle) in self.lookup.get_mut() {
            if let Some(new_resource) = recreation_func(desc) {
                let resource = self.resources.get_mut().get_mut(*handle).unwrap();
                resource.statistics.frame_created = self.current_frame_index;
                resource.resource = new_resource;
            }
        }
    }

    /// Locks the resource pool for resolving handles.
    ///
    /// While it is locked, no new resources can be added.
    pub fn resources(&self) -> StaticResourcePoolReadLockAccessor<'_, Handle, Res> {
        StaticResourcePoolReadLockAccessor {
            resources: self.resources.read(),
            current_frame_index: self.current_frame_index,
        }
    }

    /// Takes out all resources from the pool.
    ///
    /// This is useful when the existing resources need to be accessed without
    /// taking a lock on the pool.
    pub fn take_resources(&mut self) -> StaticResourcePoolMemMoveAccessor<Handle, Res> {
        StaticResourcePoolMemMoveAccessor {
            resources: std::mem::take(self.resources.get_mut()),
            current_frame_index: self.current_frame_index,
        }
    }

    /// Counter part to `take_resources`.
    ///
    /// Logs an error if resources were added to the pool since `take_resources` was called.
    pub fn return_resources(&mut self, resources: StaticResourcePoolMemMoveAccessor<Handle, Res>) {
        let resources_added_meanwhile =
            std::mem::replace(self.resources.get_mut(), resources.resources);
        if !resources_added_meanwhile.is_empty() {
            re_log::error!(
                "{} resources were discarded because they were added to the resource pool while the resources were taken out.",
                resources_added_meanwhile.len()
            );
        }
    }

    pub fn num_resources(&self) -> usize {
        self.resources.read().len()
    }
}

fn to_pool_error<T>(get_result: Option<T>, handle: impl Key) -> Result<T, PoolError> {
    get_result.ok_or_else(|| {
        if handle.is_null() {
            PoolError::NullHandle
        } else {
            PoolError::ResourceNotAvailable
        }
    })
}

/// Accessor to the resource pool, either by taking a read lock or by moving out the resources.
pub trait StaticResourcePoolAccessor<Handle: Key, Res> {
    fn get(&self, handle: Handle) -> Result<&Res, PoolError>;
}

/// Accessor to the resource pool by taking a read lock.
pub struct StaticResourcePoolReadLockAccessor<'a, Handle: Key, Res> {
    resources: RwLockReadGuard<'a, SlotMap<Handle, StoredResource<Res>>>,
    current_frame_index: u64,
}

impl<'a, Handle: Key, Res> StaticResourcePoolAccessor<Handle, Res>
    for StaticResourcePoolReadLockAccessor<'a, Handle, Res>
{
    fn get(&self, handle: Handle) -> Result<&Res, PoolError> {
        to_pool_error(
            self.resources.get(handle).map(|resource| {
                resource.statistics.last_frame_used.store(
                    self.current_frame_index,
                    std::sync::atomic::Ordering::Relaxed,
                );
                &resource.resource
            }),
            handle,
        )
    }
}

impl<'a, Handle: Key, Res> StaticResourcePoolReadLockAccessor<'a, Handle, Res> {
    pub fn get_statistics(&self, handle: Handle) -> Result<&ResourceStatistics, PoolError> {
        to_pool_error(
            self.resources
                .get(handle)
                .map(|resource| &resource.statistics),
            handle,
        )
    }
}

/// Accessor to the resource pool by moving out the resources.
pub struct StaticResourcePoolMemMoveAccessor<Handle: Key, Res> {
    resources: SlotMap<Handle, StoredResource<Res>>,
    current_frame_index: u64,
}

impl<Handle: Key, Res> StaticResourcePoolAccessor<Handle, Res>
    for StaticResourcePoolMemMoveAccessor<Handle, Res>
{
    fn get(&self, handle: Handle) -> Result<&Res, PoolError> {
        to_pool_error(
            self.resources.get(handle).map(|resource| {
                resource.statistics.last_frame_used.store(
                    self.current_frame_index,
                    std::sync::atomic::Ordering::Relaxed,
                );
                &resource.resource
            }),
            handle,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use slotmap::Key;

    use super::StaticResourcePool;
    use crate::wgpu_resources::{
        resource::PoolError, static_resource_pool::StaticResourcePoolAccessor as _,
    };

    slotmap::new_key_type! { pub struct ConcreteHandle; }

    #[derive(Clone, PartialEq, Eq, Hash, Debug)]
    pub struct ConcreteResourceDesc(u32);

    #[derive(PartialEq, Eq, Debug)]
    pub struct ConcreteResource(u32);

    type Pool = StaticResourcePool<ConcreteHandle, ConcreteResourceDesc, ConcreteResource>;

    #[test]
    fn resource_reuse() {
        let pool = Pool::default();

        // New resource
        let res0 = {
            let new_resource_created = Cell::new(false);
            let handle = pool.get_or_create(&ConcreteResourceDesc(0), |d| {
                new_resource_created.set(true);
                ConcreteResource(d.0)
            });
            assert!(new_resource_created.get());
            handle
        };

        // Get same resource again
        {
            let new_resource_created = Cell::new(false);
            let handle = pool.get_or_create(&ConcreteResourceDesc(0), |d| {
                new_resource_created.set(true);
                ConcreteResource(d.0)
            });
            assert!(!new_resource_created.get());
            assert_eq!(handle, res0);
        }
    }

    #[test]
    fn get_resource() {
        let pool = Pool::default();
        let handle = pool.get_or_create(&ConcreteResourceDesc(0), |d| ConcreteResource(d.0));

        // Query with valid handle
        let resources = pool.resources();
        assert!(resources.get(handle).is_ok());
        assert_eq!(*resources.get(handle).unwrap(), ConcreteResource(0));

        // Query with null handle
        assert_eq!(
            resources.get(ConcreteHandle::null()),
            Err(PoolError::NullHandle)
        );

        // Query with invalid handle
        let pool = Pool::default();
        let resources = pool.resources();
        assert_eq!(resources.get(handle), Err(PoolError::ResourceNotAvailable));
    }
}
