use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::Debug,
    hash::Hash,
    sync::Arc,
};

use slotmap::{Key, SecondaryMap, SlotMap};

use smallvec::{smallvec, SmallVec};

use super::resource::*;

/// Generic resource pool for all resources that have varying contents beyond their description.
///
/// Unlike in [`super::static_resource_pool::StaticResourcePool`], a resource is not uniquely identified by its description.
pub(super) struct DynamicResourcePool<Handle: Key, Desc, Res> {
    /// All known resources of this type.
    resources: SlotMap<Handle, (Desc, Res)>,

    /// Handles to all alive resources.
    /// We story any ref counted handle we give out in [`DynamicResourcePool::alloc`] here in order to keep it alive.
    /// Every [`DynamicResourcePool::frame_maintenance`] we check if the pool is now the only owner of the handle
    /// and if so mark it as deallocated.
    /// Being a [`SecondaryMap`] allows us to upgrade "weak" handles to strong handles,
    /// something required by [`super::BindGroupPool`]
    alive_handles: SecondaryMap<Handle, Arc<Handle>>,

    /// Any resource that has been deallocated last frame.
    /// We keep them around for a bit longer to allow reclamation
    last_frame_deallocated: HashMap<Desc, SmallVec<[Arc<Handle>; 4]>>,

    current_frame_index: u64,
}

impl<Handle: Key, Desc, Res> Default for DynamicResourcePool<Handle, Desc, Res> {
    fn default() -> Self {
        Self {
            resources: Default::default(),
            alive_handles: Default::default(),
            last_frame_deallocated: Default::default(),
            current_frame_index: Default::default(),
        }
    }
}

impl<Handle, Desc, Res> DynamicResourcePool<Handle, Desc, Res>
where
    Handle: Key,
    Desc: Clone + Eq + Hash + Debug,
    Res: Resource,
{
    pub fn alloc<F: FnOnce(&Desc) -> Res>(&mut self, desc: &Desc, creation_func: F) -> Arc<Handle> {
        // First check if we can reclaim a resource we have around from a previous frame.
        let handle =
            if let Entry::Occupied(mut entry) = self.last_frame_deallocated.entry(desc.clone()) {
                re_log::trace!(?desc, "Reclaimed previously discarded resource",);

                let handle = entry.get_mut().pop().unwrap();
                if entry.get().is_empty() {
                    entry.remove();
                }
                handle
            // Otherwise create a new resource
            } else {
                let resource = creation_func(desc);
                Arc::new(self.resources.insert((desc.clone(), resource)))
            };

        self.alive_handles.insert(*handle, Arc::clone(&handle));
        handle
    }

    pub fn get_resource(&self, handle: Handle) -> Result<&Res, PoolError> {
        self.resources
            .get(handle)
            .map(|(_, resource)| {
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

    pub fn frame_maintenance(&mut self, frame_index: u64) {
        self.current_frame_index = frame_index;

        // Throw out any resources that we haven't reclaimed last frame.
        for (desc, handles) in self.last_frame_deallocated.drain() {
            re_log::debug!(
                count = handles.len(),
                ?desc,
                "Drained dangling resources from last frame",
            );
            for handle in handles {
                self.resources.remove(*handle);
            }
        }

        // If the strong count went down to 1, we must be the only ones holding on to handle.
        //
        // thread safety:
        // Since the count is pushed from 1 to 2 by `alloc`, it should not be possible to ever
        // get temporarily get back down to 1 without dropping the last user available copy of the Arc<Handle>.
        self.alive_handles.retain(|handle, strong_handle| {
            if Arc::strong_count(strong_handle) == 1 {
                let desc = &self.resources[handle].0;
                match self.last_frame_deallocated.entry(desc.clone()) {
                    Entry::Occupied(mut e) => {
                        e.get_mut().push(Arc::clone(strong_handle));
                    }
                    Entry::Vacant(e) => {
                        e.insert(smallvec![Arc::clone(strong_handle)]);
                    }
                }
                false
            } else {
                true
            }
        });
    }

    /// Upgrades a "weak" handle to a reference counted handle by looking it up.
    /// Returns a reference in order to avoid needlessly increasing the ref-count.
    pub(super) fn get_strong_handle(&self, handle: Handle) -> &Arc<Handle> {
        &self.alive_handles[handle]
    }
}
