use std::{
    collections::{hash_map::Keys, HashMap},
    fmt::Debug,
    hash::Hash,
    sync::atomic::{AtomicU64, Ordering},
};

use slotmap::{Key, SlotMap};

#[derive(thiserror::Error, Debug)]
pub enum PoolError {
    #[error("Requested resource isn't available yet of the handle is no longer valid")]
    ResourceNotAvailable,

    #[error("The passed resource handle was null")]
    NullHandle,
}

/// A resource that can be owned & lifetime tracked by `ResourcePool`
pub(crate) trait Resource {
    /// Called every time a resource handle was resolved to its `Resource` object.
    /// (typically on `ResourcePool::get`)
    fn on_handle_resolve(&self, _current_frame_index: u64) {}
}

/// A resource that keeps track of the last frame it was used.
///
/// All resources should implement this, except those which are regarded lightweight enough to keep around indefinitely but heavy enough
/// that we don't want to create them every frame (i.e. need a `ResourcePool`)
pub(crate) trait UsageTrackedResource {
    fn last_frame_used(&self) -> &AtomicU64;
}

impl<T: UsageTrackedResource> Resource for T {
    fn on_handle_resolve(&self, current_frame_index: u64) {
        self.last_frame_used()
            .fetch_max(current_frame_index, Ordering::Release);
    }
}

/// Generic resource pool used as base for specialized pools
pub(super) struct ResourcePool<Handle: Key, Desc, Res> {
    resources: SlotMap<Handle, Res>,
    lookup: HashMap<Desc, Handle>,
    current_frame_index: u64,
}

impl<Handle: Key, Desc, Res> Default for ResourcePool<Handle, Desc, Res> {
    fn default() -> Self {
        Self {
            resources: Default::default(),
            lookup: Default::default(),
            current_frame_index: Default::default(),
        }
    }
}

impl<Handle, Desc, Res> ResourcePool<Handle, Desc, Res>
where
    Handle: Key,
    Desc: Clone + Eq + Hash,
    Res: Resource,
{
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

    pub fn resource_descs(&self) -> Keys<'_, Desc, Handle> {
        self.lookup.keys()
    }
}

impl<Handle, Desc, Res> ResourcePool<Handle, Desc, Res>
where
    Handle: Key,
    Res: UsageTrackedResource,
    Desc: Debug,
{
    pub fn discard_unused_resources(&mut self, frame_index: u64) {
        self.resources.retain(|_, resource| {
            resource.last_frame_used().load(Ordering::Acquire) >= self.current_frame_index
        });
        self.lookup.retain(|desc, handle| {
            let retain = self.resources.contains_key(*handle);
            if !retain {
                re_log::debug!("discarded resource with desc {:?}", desc);
            }
            retain
        });

        self.current_frame_index = frame_index;
    }
}
