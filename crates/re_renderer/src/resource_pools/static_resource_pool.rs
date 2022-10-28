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
    Res: Resource,
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
