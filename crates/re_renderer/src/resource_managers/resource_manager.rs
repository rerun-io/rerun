use slotmap::{Key, SecondaryMap, SlotMap};

use crate::resource_pools::PoolError;

/// Handle to a resource that is stored in a
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ResourceHandle<InnerHandle: slotmap::Key> {
    /// Handle that is valid until user explicitly removes the resource from respective resource manager.
    LongLived(InnerHandle),

    /// Handle that is valid for a single frame
    Frame {
        key: InnerHandle,
        /// This handle is only valid for this frame.
        /// Querying it during any other frame will fail.
        valid_frame_index: u64,
    },
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum ResourceManagerError {
    #[error("The requested resource is no longer valid. It was valid for the frame index {current_frame_index}, but the current frame index is {valid_frame_index}")]
    ExpiredResource {
        current_frame_index: u64,
        valid_frame_index: u64,
    },

    #[error("The requested resource isn't available because the handle is no longer valid")]
    ResourceNotAvailable,

    #[error("The passed resource handle was null")]
    NullHandle,

    #[error("Failed accessing resource pools")]
    ResourcePoolError(PoolError),
}

#[derive(Clone, Copy)]
pub enum ResourceLifeTime {
    /// A resources that lives only for a single frame.
    ///
    /// All handles to it will be invalidated at the end of the frame,
    /// both cpu and (if present) gpu data will be freed at that point.
    /// This allows us to use a more efficient allocation strategy for gpu data.
    SingleFrame,

    /// A resource that lives for an indefinite (user specified) amount of time
    /// until explicitly deallocated.
    /// TODO(andreas): Deallocation is not yet possible.
    LongLived,
}

pub struct ResourceManager<InnerHandle: Key, Res, GpuRes> {
    long_lived_resources: SlotMap<InnerHandle, Res>,
    long_lived_resources_gpu: SecondaryMap<InnerHandle, GpuRes>,

    single_frame_resources: SlotMap<InnerHandle, Res>,
    single_frame_resources_gpu: SecondaryMap<InnerHandle, GpuRes>,

    frame_index: u64,
}

impl<InnerHandle: Key, Res, GpuRes> Default for ResourceManager<InnerHandle, Res, GpuRes> {
    fn default() -> Self {
        Self {
            long_lived_resources: Default::default(),
            long_lived_resources_gpu: Default::default(),
            single_frame_resources: Default::default(),
            single_frame_resources_gpu: Default::default(),
            frame_index: Default::default(),
        }
    }
}

impl<InnerHandle, Res, GpuRes> ResourceManager<InnerHandle, Res, GpuRes>
where
    InnerHandle: Key,
    GpuRes: Clone,
{
    /// Creates a new resource.
    pub fn store_resource(
        &mut self,
        resource: Res,
        lifetime: ResourceLifeTime,
    ) -> ResourceHandle<InnerHandle> {
        match lifetime {
            ResourceLifeTime::SingleFrame => ResourceHandle::<InnerHandle>::Frame {
                key: self.single_frame_resources.insert(resource),
                valid_frame_index: self.frame_index,
            },
            ResourceLifeTime::LongLived => {
                ResourceHandle::<InnerHandle>::LongLived(self.long_lived_resources.insert(resource))
            }
        }
    }

    /// Accesses a given resource under a read lock.
    pub(crate) fn get(
        &self,
        handle: ResourceHandle<InnerHandle>,
    ) -> Result<&Res, ResourceManagerError> {
        let (slotmap, key) = match handle {
            ResourceHandle::LongLived(key) => (&self.long_lived_resources, key),
            ResourceHandle::Frame {
                key,
                valid_frame_index,
            } => {
                self.check_frame_resource_lifetime(valid_frame_index)?;
                (&self.single_frame_resources, key)
            }
        };

        slotmap.get(key).ok_or_else(|| {
            if key.is_null() {
                ResourceManagerError::NullHandle
            } else {
                ResourceManagerError::ResourceNotAvailable
            }
        })
    }

    fn check_frame_resource_lifetime(
        &self,
        valid_frame_index: u64,
    ) -> Result<(), ResourceManagerError> {
        if valid_frame_index != self.frame_index {
            return Err(ResourceManagerError::ExpiredResource {
                current_frame_index: self.frame_index,
                valid_frame_index,
            });
        } else {
            Ok(())
        }
    }

    /// Retrieve gpu representation of a resource.
    ///
    /// Uploads to gpu if not already done.
    pub(crate) fn get_or_create_gpu_resource<
        F: FnOnce(&Res, ResourceLifeTime) -> Result<GpuRes, ResourceManagerError>,
    >(
        &mut self,
        handle: ResourceHandle<InnerHandle>,
        create_gpu_resource: F,
    ) -> anyhow::Result<GpuRes, ResourceManagerError> {
        let (slotmap, slotmap_gpu, key, lifetime) = match handle {
            ResourceHandle::<InnerHandle>::LongLived(key) => (
                &self.long_lived_resources,
                &mut self.long_lived_resources_gpu,
                key,
                ResourceLifeTime::LongLived,
            ),
            ResourceHandle::<InnerHandle>::Frame {
                key,
                valid_frame_index,
            } => {
                self.check_frame_resource_lifetime(valid_frame_index)?;
                (
                    &self.single_frame_resources,
                    &mut self.single_frame_resources_gpu,
                    key,
                    ResourceLifeTime::SingleFrame,
                )
            }
        };

        Ok(if let Some(gpu_resource) = slotmap_gpu.get(key) {
            gpu_resource.clone()
        } else {
            let resource = slotmap.get(key).ok_or_else(|| {
                if key.is_null() {
                    ResourceManagerError::NullHandle
                } else {
                    ResourceManagerError::ResourceNotAvailable
                }
            })?;

            // TODO(andreas): Should we throw out the cpu data now, at least for long lived Resources?
            let resource_gpu = create_gpu_resource(resource, lifetime)?;
            slotmap_gpu.insert(key, resource_gpu.clone());
            resource_gpu
        })
    }

    pub(crate) fn frame_maintenance(&mut self, frame_index: u64) {
        self.single_frame_resources.clear();
        self.single_frame_resources_gpu.clear();
        self.frame_index = frame_index;
    }
}
