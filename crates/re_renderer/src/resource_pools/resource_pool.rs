use std::{
    collections::HashMap,
    hash::Hash,
    sync::atomic::{AtomicU64, Ordering},
};

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
    fn on_handle_resolve(&self, _current_frame_index: u64) {}
}

/// A resource that keeps track of the last frame it was used.
///
/// In contrast, there are some resource that we don't care about when it was used the last time!
/// This makes sense for resources that are regarded lightweight enough
/// to keep around indefinitely but heavy enough that we don't want to create them every frame.
pub(crate) trait UsageTrackedResource {
    fn last_frame_used(&self) -> &AtomicU64;
}

impl<T: UsageTrackedResource> Resource for T {
    fn on_handle_resolve(&self, current_frame_index: u64) {
        self.last_frame_used()
            .fetch_max(current_frame_index, Ordering::Relaxed);
    }
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

    pub fn get(&self, handle: Handle) -> Result<&Res, PoolError> {
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
}

impl<Handle, Desc, Res> ResourcePool<Handle, Desc, Res>
where
    Handle: Key,
    Res: UsageTrackedResource,
{
    pub fn frame_maintenance(&mut self, frame_index: u64) {
        // TODO: Remove resource that we haven't used for a while. Details should be configurable
        self.current_frame_index = frame_index;
    }
}
