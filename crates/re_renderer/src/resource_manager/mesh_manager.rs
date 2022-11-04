use slotmap::{Key, SecondaryMap, SlotMap};

use crate::{
    mesh::{GpuMesh, Mesh},
    resource_pools::WgpuResourcePools,
    RenderContext,
};

use super::{ResourceHandle, ResourceManagerError};

slotmap::new_key_type! { pub struct MeshHandleInner; }

pub type MeshHandle = ResourceHandle<MeshHandleInner>;

pub struct MeshManager {
    long_lived_meshes: SlotMap<MeshHandleInner, Mesh>,
    long_lived_meshes_gpu: SecondaryMap<MeshHandleInner, GpuMesh>,

    frame_meshes: SlotMap<MeshHandleInner, Mesh>,
    frame_meshes_gpu: SecondaryMap<MeshHandleInner, GpuMesh>,

    frame_index: u64,
}

impl Default for MeshManager {
    fn default() -> Self {
        Self {
            long_lived_meshes: Default::default(),
            long_lived_meshes_gpu: Default::default(),
            frame_meshes: Default::default(),
            frame_meshes_gpu: Default::default(),
            frame_index: Default::default(),
        }
    }
}

impl MeshManager {
    /// Creates a new, long lived mesh.
    ///
    /// For short lived meshes use [`Self::new_frame_mesh`] as it has more efficient resource usage for this scenario.
    /// TODO(andreas): Should be able to destroy long lived meshes
    pub fn new_long_lived_mesh(&mut self, resource: Mesh) -> MeshHandle {
        MeshHandle::LongLived(self.long_lived_meshes.insert(resource))
    }

    /// Creates a mesh that lives for the duration of the frame
    ///
    /// Using the handle in the following frame will cause an error.
    pub fn new_frame_mesh(&mut self, resource: Mesh) -> MeshHandle {
        MeshHandle::Frame {
            key: self.frame_meshes.insert(resource),
            valid_frame_index: self.frame_index,
        }
    }

    /// Retrieve a mesh.
    ///
    /// Uploads to gpu if not already done.
    pub(crate) fn to_gpu(
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        handle: MeshHandle,
    ) -> Result<GpuMesh, ResourceManagerError> {
        let (slotmap, slotmap_gpu, key) = match handle {
            MeshHandle::LongLived(key) => (
                &ctx.meshes.long_lived_meshes,
                &mut ctx.meshes.long_lived_meshes_gpu,
                key,
            ),
            MeshHandle::Frame {
                key,
                valid_frame_index,
            } => {
                if valid_frame_index != ctx.meshes.frame_index {
                    return Err(ResourceManagerError::ExpiredResource {
                        current_frame_index: ctx.meshes.frame_index,
                        valid_frame_index,
                    });
                }
                (
                    &ctx.meshes.frame_meshes,
                    &mut ctx.meshes.frame_meshes_gpu,
                    key,
                )
            }
        };

        Ok(match slotmap_gpu.get(key) {
            Some(gpu_resource) => gpu_resource.clone(),
            None => {
                let resource = slotmap.get(key).ok_or_else(|| {
                    if key.is_null() {
                        ResourceManagerError::NullHandle
                    } else {
                        ResourceManagerError::ResourceNotAvailable
                    }
                })?;

                // TODO(andreas): Should we throw out the cpu data now, at least for long lived meshes?
                let resource_gpu = GpuMesh::new(&mut ctx.resource_pools, device, queue, resource);
                slotmap_gpu.insert(key, resource_gpu.clone());
                resource_gpu
            }
        })
    }

    pub(crate) fn frame_maintenance(&mut self, frame_index: u64) {
        self.frame_meshes.clear();
        self.frame_index = frame_index;
    }
}
