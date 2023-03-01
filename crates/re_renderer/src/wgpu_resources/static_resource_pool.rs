use std::{collections::HashMap, hash::Hash, sync::atomic::Ordering};

use slotmap::{Key, SlotMap};

use super::resource::{PoolError, ResourceStatistics};

struct StoredResource<Res> {
    resource: Res,
    statistics: ResourceStatistics,
}

/// Generic resource pool for all resources that are fully described upon creation, i.e. never have any variable content.
///
/// This implies, a resource is uniquely defined by its description.
/// We call these resources "static" because they never change their content over their lifetime.
pub(super) struct StaticResourcePool<Handle: Key, Desc, Res> {
    resources: SlotMap<Handle, StoredResource<Res>>,
    lookup: HashMap<Desc, Handle>,
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
    fn to_pool_error<T>(get_result: Option<T>, handle: Handle) -> Result<T, PoolError> {
        get_result.ok_or_else(|| {
            if handle.is_null() {
                PoolError::NullHandle
            } else {
                PoolError::ResourceNotAvailable
            }
        })
    }

    pub fn get_or_create<F: FnOnce(&Desc) -> Res>(
        &mut self,
        desc: &Desc,
        creation_func: F,
    ) -> Handle {
        *self.lookup.entry(desc.clone()).or_insert_with(|| {
            re_log::debug!(?desc, "Created new resource");
            let resource = creation_func(desc);
            self.resources.insert(StoredResource {
                resource,
                statistics: ResourceStatistics {
                    frame_created: self.current_frame_index,
                    last_frame_used: self.current_frame_index.into(),
                },
            })
        })
    }

    pub fn recreate_resources<F: FnMut(&Desc) -> Option<Res>>(&mut self, mut recreation_func: F) {
        for (desc, handle) in &self.lookup {
            if let Some(new_resource) = recreation_func(desc) {
                let resource = self.resources.get_mut(*handle).unwrap();
                resource.statistics.frame_created = self.current_frame_index;
                resource.resource = new_resource;
            }
        }
    }

    pub fn get_resource(&self, handle: Handle) -> Result<&Res, PoolError> {
        Self::to_pool_error(
            self.resources.get(handle).map(|resource| {
                resource
                    .statistics
                    .last_frame_used
                    .store(self.current_frame_index, Ordering::Relaxed);
                &resource.resource
            }),
            handle,
        )
    }

    pub fn get_statistics(&self, handle: Handle) -> Result<&ResourceStatistics, PoolError> {
        Self::to_pool_error(
            self.resources
                .get(handle)
                .map(|resource| &resource.statistics),
            handle,
        )
    }

    pub fn num_resources(&self) -> usize {
        self.resources.len()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use slotmap::Key;

    use super::StaticResourcePool;
    use crate::wgpu_resources::resource::PoolError;

    slotmap::new_key_type! { pub struct ConcreteHandle; }

    #[derive(Clone, PartialEq, Eq, Hash, Debug)]
    pub struct ConcreteResourceDesc(u32);

    #[derive(PartialEq, Eq, Debug)]
    pub struct ConcreteResource(u32);

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
