use std::{
    collections::HashMap,
    fmt::Debug,
    hash::Hash,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use slotmap::{Key, SlotMap};

#[derive(thiserror::Error, Debug)]
pub enum PoolError {
    #[error("Requested resource isn't available yet because the handle is no longer valid")]
    ResourceNotAvailable,

    #[error("The passed resource handle was null")]
    NullHandle,
}

/// A resource that can be owned & lifetime tracked by [`ResourcePool`]
pub(crate) trait Resource {
    /// Called every time a resource handle was resolved to its [`Resource`] object.
    /// (typically on [`ResourcePool::get_resource`])
    fn on_handle_resolve(&self, _current_frame_index: u64) {}
}

/// A resource that keeps track of the last frame it was used.
///
/// All resources should implement this, except those which are regarded lightweight enough to keep around indefinitely but heavy enough
/// that we don't want to create them every frame (i.e. need a [`ResourcePool`])
pub(crate) trait UsageTrackedResource {
    fn last_frame_used(&self) -> &AtomicU64;
}

impl<T: UsageTrackedResource> Resource for T {
    fn on_handle_resolve(&self, current_frame_index: u64) {
        self.last_frame_used()
            .fetch_max(current_frame_index, Ordering::Release);
    }
}

/// Generic resource pool for all resources that are fully described upon creation, i.e. never have any variable content.
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
            let resource = creation_func(desc); // TODO(andreas): Handle creation failure
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

impl<Handle, Desc, Res> StaticResourcePool<Handle, Desc, Res>
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

// ------------------------------------------------------------------------------------------------

/// Generic resource pool for all resources that have varying contents beyond their description.
#[derive(Default)]
pub(super) struct DynamicResourcePool<Handle: Key, Desc, Res> {
    // All known resources of this type.
    resources: SlotMap<Handle, (Desc, Res)>,

    // Handles to all alive resources.
    alive_handles: Vec<Arc<Handle>>,

    // Any resource that has been allocated last frame.
    // We keep them around for a bit longer to allow reclamation
    last_frame_deallocated: HashMap<Desc, Arc<Handle>>,

    current_frame_index: u64,
}

impl<Handle, Desc, Res> DynamicResourcePool<Handle, Desc, Res>
where
    Handle: Key,
    Desc: Clone + Eq + Hash,
    Res: Resource,
{
    fn alloc(&mut self) -> anyhow::Result<Arc<Handle>> {
        // First check if we can reclaim a resource we have around from previous frame.

        // TODO:
    }

    fn get_resource(&self, handle: Arc<Handle>) -> Result<&Res, PoolError> {
        self.resources
            .get(*handle)
            .map(|resource| {
                resource.on_handle_resolve(self.current_frame_index);
                &resource.1
            })
            .ok_or_else(|| {
                if handle.is_null() {
                    PoolError::NullHandle
                } else {
                    PoolError::ResourceNotAvailable
                }
            })
    }

    fn frame_maintenance(&mut self, current_frame_index: u64) {
        self.current_frame_index = current_frame_index;

        // Throw out any resources that we haven't reclaimed last frame.
        // TODO: Trace log this!
        self.last_frame_deallocated.clear();

        // If the strong count went down to 1, we must be the only ones holding on to handle.
        // thread safety:
        // Since the count is pushed from 1 to 2 by `alloc`
        // it should not be possible to ever get temporarily get back down to 1 without dropping the last
        // user available copy of the Arc<Handle>.
        // Want drain_filter here - https://github.com/rust-lang/rust/issues/43244

        let mut i = 0;
        while i < self.alive_handles.len() {
            if Arc::<Handle>::strong_count(&self.alive_handles[i]) == 1 {
                self.last_frame_deallocated
                    .push(self.alive_handles.remove(i));
            } else {
                i += 1;
            }
        }
    }
}
