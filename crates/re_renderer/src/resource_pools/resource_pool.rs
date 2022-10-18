use std::{
    collections::HashMap,
    fmt::Debug,
    hash::Hash,
    sync::atomic::{AtomicU64, Ordering},
};

use slotmap::{Key, SlotMap};

#[derive(thiserror::Error, Debug)]
pub enum PoolError {
    #[error("Requested resource isn't available because the handle is no longer valid")]
    ResourceNotAvailable,

    #[error("The passed resource handle was null")]
    NullHandle,
}

/// A resource that can be owned & lifetime tracked by [`ResourcePool`]
pub(crate) trait Resource {
    /// Called every time a resource handle was resolved to its [`Resource`] object.
    /// (typically on [`ResourcePool::get_resource`])
    fn on_handle_resolve(&self, _current_frame_index: u64) {}

    /// Pinning a resource means that it won't be disposed if left unused.
    fn pin(&self) {}
}

/// A resource that keeps track of the last frame it was used.
///
/// All resources should implement this, except those which are regarded lightweight enough to keep around indefinitely but heavy enough
/// that we don't want to create them every frame (i.e. need a [`ResourcePool`])
pub(crate) trait UsageTrackedResource {
    const PIN_BIT: u64 = 1u64 << 63;

    fn usage_state(&self) -> &AtomicU64;
}

impl<T: UsageTrackedResource> Resource for T {
    fn on_handle_resolve(&self, current_frame_index: u64) {
        let mut usage_state = self.usage_state().load(Ordering::Relaxed);
        loop {
            let new_usage_state = current_frame_index | (usage_state & Self::PIN_BIT);
            match self.usage_state().compare_exchange_weak(
                usage_state,
                new_usage_state,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(old) => usage_state = old,
            }
        }
    }

    /// Pinned resources are not garbage collected.
    fn pin(&self) {
        self.usage_state()
            .fetch_or(Self::PIN_BIT, Ordering::Release);
    }
}

/// Generic resource pool used as base for specialized pools
pub(crate) struct ResourcePool<Handle: Key, Desc, Res> {
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
    pub fn get_handle<F: FnOnce(&Desc) -> Res>(&mut self, desc: &Desc, creation_func: F) -> Handle {
        *self.lookup.entry(desc.clone()).or_insert_with(|| {
            let resource = creation_func(desc); // TODO(andreas): Handle creation failure
            self.resources.insert(resource)
        })
    }

    pub fn resource_descs(&self) -> impl Iterator<Item = &Desc> {
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
            resource.usage_state().load(Ordering::Acquire) >= self.current_frame_index
        });
        self.lookup.retain(|desc, handle| {
            let retain = self.resources.contains_key(*handle);
            if !retain {
                re_log::debug!(
                    "discarded resource with desc {:?} since it hasn't been used in frame {}",
                    desc,
                    self.current_frame_index
                );
            }
            retain
        });

        self.current_frame_index = frame_index;
    }
}

pub(crate) trait ResourcePoolFacade<'a, Handle, Desc, Res>
where
    Handle: 'a + Key,
    Desc: 'a + Clone + Eq + Hash,
    Res: 'a + Resource,
{
    fn pool(&'a self) -> &ResourcePool<Handle, Desc, Res>;

    fn get_resource(&'a self, handle: Handle) -> Result<&Res, PoolError> {
        let current_frame_index = self.pool().current_frame_index;

        self.pool()
            .resources
            .get(handle)
            .map(|resource| {
                resource.on_handle_resolve(current_frame_index);
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

    fn register_resource_usage(&'a self, handle: Handle) {
        let _ = self.get_resource(handle);
    }

    fn pin_resource(&'a self, handle: Handle) {
        if let Some(resource) = self.pool().resources.get(handle) {
            resource.pin();
        }
    }
}
