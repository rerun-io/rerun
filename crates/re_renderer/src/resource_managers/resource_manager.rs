use std::sync::Arc;

use slotmap::{Key, SlotMap};

use crate::wgpu_resources::PoolError;

/// Handle to a resource that is stored in a resource manager.
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ResourceHandle<InnerHandle: slotmap::Key> {
    /// Resource handle that keeps the resource alive as long as there are handles.
    ///
    /// Once the user drops the last handle, the resource will be discarded on next frame maintenance.
    /// (actual reclaiming of GPU resources may take longer)
    LongLived(Arc<InnerHandle>),

    /// Handle that is valid for a single frame
    Frame {
        key: InnerHandle,

        /// This handle is only valid for this frame.
        /// Querying it during any other frame will fail.
        valid_frame_index: u64,
    },

    /// No handle, causes error on resolve.
    Invalid,
}

#[derive(thiserror::Error, Debug)]
pub enum ResourceManagerError {
    #[error("The requested resource is no longer valid. It was valid for the frame index {current_frame_index}, but the current frame index is {valid_frame_index}")]
    ExpiredResource {
        current_frame_index: u64,
        valid_frame_index: u64,
    },

    #[error("The requested resource isn't available because the handle is no longer stored in the resource manager")]
    ResourceNotFound,

    #[error("The passed resource handle was null")]
    NullHandle,

    #[error("Failed accessing resource pools: {0}")]
    ResourcePoolError(PoolError),

    #[error("Invalid mesh given as input: {0}")]
    InvalidMesh(#[from] crate::mesh::MeshError),

    #[error("Failed to transfer data to the GPU: {0}")]
    FailedTransferringDataToGpu(#[from] crate::allocator::CpuWriteGpuReadError),
}

#[derive(Clone, Copy)]
pub enum ResourceLifeTime {
    /// A resources that lives only for a single frame.
    ///
    /// All handles to it will be invalidated at the end of the frame,
    /// both cpu and (if present) gpu data will be freed at that point.
    /// This allows us to use a more efficient allocation strategy for gpu data.
    SingleFrame,

    /// A resource that lives until its last handle was dropped.
    ///
    /// Once the last handle is dropped, it freed during frame maintenance.
    LongLived,
}

pub struct ResourceManager<InnerHandle: Key, GpuRes> {
    /// We store a refcounted handle along side every long lived resource so we can tell if it is still alive.
    /// This mechanism is similar to `crate::wgpu_resources::DynamicResourcePool`, only that we don't retain for another frame.
    ///
    /// Perf note:
    /// If we get *a lot* of resources this might scale poorly (well, linearly)
    /// If this happens, we need to make the handle more complex and give it an Arc<Mutex<>> of a free list where it can enter itself.
    /// (which makes passing the handles & having many short lived handles more costly)
    long_lived_resources: SlotMap<InnerHandle, (GpuRes, Arc<InnerHandle>)>,
    single_frame_resources: SlotMap<InnerHandle, GpuRes>,

    frame_index: u64,
}

impl<InnerHandle: Key, GpuRes> Default for ResourceManager<InnerHandle, GpuRes> {
    fn default() -> Self {
        Self {
            long_lived_resources: Default::default(),
            single_frame_resources: Default::default(),
            frame_index: Default::default(),
        }
    }
}

impl<InnerHandle, GpuRes> ResourceManager<InnerHandle, GpuRes>
where
    InnerHandle: Key,
    GpuRes: Clone,
{
    /// Creates a new resource.
    pub fn store_resource(
        &mut self,
        resource: GpuRes,
        lifetime: ResourceLifeTime,
    ) -> ResourceHandle<InnerHandle> {
        match lifetime {
            ResourceLifeTime::SingleFrame => ResourceHandle::<InnerHandle>::Frame {
                key: self.single_frame_resources.insert(resource),
                valid_frame_index: self.frame_index,
            },
            ResourceLifeTime::LongLived => {
                let mut ref_counted_key = Arc::new(Default::default());
                self.long_lived_resources.insert_with_key(|key| {
                    ref_counted_key = Arc::new(key);
                    (resource, ref_counted_key.clone())
                });
                ResourceHandle::<InnerHandle>::LongLived(ref_counted_key)
            }
        }
    }

    pub(crate) fn get(
        &self,
        handle: &ResourceHandle<InnerHandle>,
    ) -> Result<&GpuRes, ResourceManagerError> {
        match handle {
            ResourceHandle::LongLived(key) => self
                .long_lived_resources
                .get(**key)
                .map(|(res, _)| res)
                .ok_or_else(|| {
                    if key.is_null() {
                        ResourceManagerError::NullHandle
                    } else {
                        ResourceManagerError::ResourceNotFound
                    }
                }),
            ResourceHandle::Frame {
                key,
                valid_frame_index,
            } => {
                self.check_frame_resource_lifetime(*valid_frame_index)?;
                self.single_frame_resources.get(*key).ok_or_else(|| {
                    if key.is_null() {
                        ResourceManagerError::NullHandle
                    } else {
                        ResourceManagerError::ResourceNotFound
                    }
                })
            }
            ResourceHandle::Invalid => Err(ResourceManagerError::NullHandle),
        }
    }

    fn check_frame_resource_lifetime(
        &self,
        valid_frame_index: u64,
    ) -> Result<(), ResourceManagerError> {
        if valid_frame_index != self.frame_index {
            Err(ResourceManagerError::ExpiredResource {
                current_frame_index: self.frame_index,
                valid_frame_index,
            })
        } else {
            Ok(())
        }
    }

    pub(crate) fn begin_frame(&mut self, frame_index: u64) {
        // Kill single frame resources.
        self.single_frame_resources.clear();

        // And figure out which long lived ones need to be garbage collected.
        // If the strong count went down to 1, we must be the only ones holding on to handle.
        //
        // thread safety:
        // Since the count is pushed from 1 to 2 by `alloc`, it should not be possible to ever
        // get temporarily get back down to 1 without dropping the last user available copy of the Arc<Handle>.
        self.long_lived_resources
            .retain(|_, (_, strong_handle)| Arc::strong_count(strong_handle) > 1);

        self.frame_index = frame_index;
    }
}
