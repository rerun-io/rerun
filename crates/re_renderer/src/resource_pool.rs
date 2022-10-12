use std::{collections::HashMap, hash::Hash};

use slotmap::{Key, SlotMap};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PoolError {
    #[error("Requested resource isn't available yet of the handle is no longer valid")]
    ResourceNotAvailable,
    #[error("The passed resource handle was null")]
    NullHandle,
}

pub(crate) trait Resource {
    //fn last_frame_used(&self) -> u64;
    fn register_use(&self, current_frame_index: u64);
}

/// Generic resource pool used as base for specialized pools
pub(crate) struct ResourcePool<Handle: Key, Desc, Res> {
    resources: SlotMap<Handle, Res>,
    lookup: HashMap<Desc, Handle>,
    current_frame_index: u64,
}

impl<Handle, Desc, Res> ResourcePool<Handle, Desc, Res>
where
    Handle: Key,
    Desc: Clone + Eq + Hash,
    Res: Resource,
{
    pub fn new() -> Self {
        ResourcePool {
            resources: SlotMap::with_key(),
            lookup: HashMap::new(),
            current_frame_index: 0,
        }
    }

    pub fn request<F: FnOnce(&Desc) -> Res>(&mut self, desc: &Desc, creation_func: F) -> Handle {
        *self.lookup.entry(desc.clone()).or_insert_with(|| {
            let resource = creation_func(desc); // TODO(andreas): Handle creation failure
            self.resources.insert(resource)
        })
    }

    pub fn frame_maintenance(&mut self, frame_index: u64) {
        // TODO: Remove resource that we haven't used for a while. Details should be configurable
        self.current_frame_index = frame_index;
    }

    pub fn resource(&self, handle: Handle) -> Result<&Res, PoolError> {
        self.resources
            .get(handle)
            .map(|resource| {
                resource.register_use(self.current_frame_index);
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
}
