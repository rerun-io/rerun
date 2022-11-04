use std::{collections::HashMap, hash::Hash};

use slotmap::{Key, SlotMap};

use super::resource::*;

/// Generic resource pool for all resources that are fully described upon creation, i.e. never have any variable content.
///
/// This implies, a resource is uniquely defined by its description.
/// We call these resources "static" because they never change their content over their lifetime.
pub(super) struct StaticResourcePool<Handle: Key, Desc, Res> {
    resources: SlotMap<Handle, Res>,
    lookup: HashMap<Desc, Handle>,
    current_frame_index: u64,
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
    Desc: Clone + Eq + Hash,
    Res: GpuResource,
{
    pub fn get_or_create<F: FnOnce(&Desc) -> Res>(
        &mut self,
        desc: &Desc,
        creation_func: F,
    ) -> Handle {
        *self.lookup.entry(desc.clone()).or_insert_with(|| {
            let resource = creation_func(desc);
            self.resources.insert(resource)
        })
    }

    pub fn get_resource(&self, handle: Handle) -> Result<&Res, PoolError> {
        self.resources
            .get(handle)
            .map(|resource| {
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

    // TODO(cmc): Necessary for now, although not great. We'll see if we can/need-to find
    // a better way to handle this once all 3 shader-related PRs have landed.
    pub fn get_resource_mut(&mut self, handle: Handle) -> Result<&mut Res, PoolError> {
        self.resources
            .get_mut(handle)
            .map(|resource| {
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

    pub fn resource_descs(&self) -> impl Iterator<Item = &Desc> {
        self.lookup.keys()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use slotmap::Key;

    use super::StaticResourcePool;
    use crate::resource_pools::resource::{GpuResource, PoolError};

    slotmap::new_key_type! { pub struct ConcreteHandle; }

    #[derive(Clone, PartialEq, Eq, Hash, Debug)]
    pub struct ConcreteResourceDesc(u32);

    #[derive(PartialEq, Eq, Debug)]
    pub struct ConcreteResource(u32);

    impl GpuResource for ConcreteResource {
        fn on_handle_resolve(&self, _current_frame_index: u64) {}
    }

    type Pool = StaticResourcePool<ConcreteHandle, ConcreteResourceDesc, ConcreteResource>;

    #[test]
    fn resource_reuse() {
        let mut pool = Pool::default();

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
        let mut pool = Pool::default();

        // Query with valid handle
        let handle = pool.get_or_create(&ConcreteResourceDesc(0), |d| ConcreteResource(d.0));
        assert!(pool.get_resource(handle).is_ok());
        assert_eq!(*pool.get_resource(handle).unwrap(), ConcreteResource(0));

        // Query with null handle
        assert_eq!(
            pool.get_resource(ConcreteHandle::null()),
            Err(PoolError::NullHandle)
        );

        // Query with invalid handle
        pool = Pool::default();
        assert_eq!(
            pool.get_resource(handle),
            Err(PoolError::ResourceNotAvailable)
        );
    }
}
