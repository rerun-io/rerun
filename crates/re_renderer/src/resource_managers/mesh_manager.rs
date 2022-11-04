use crate::{
    mesh::{GpuMesh, Mesh},
    RenderContext,
};

use super::{
    resource_manager::ResourceManager, ResourceHandle, ResourceLifeTime, ResourceManagerError,
};

slotmap::new_key_type! { pub struct MeshHandleInner; }

pub type MeshHandle = ResourceHandle<MeshHandleInner>;

#[derive(Default)]
pub struct MeshManager {
    manager: ResourceManager<MeshHandleInner, Mesh, GpuMesh>,
}

impl MeshManager {
    /// Takes ownership of a new mesh.
    pub fn store_resource(&mut self, resource: Mesh, lifetime: ResourceLifeTime) -> MeshHandle {
        self.manager.take_ownership(resource, lifetime)
    }

    /// Retrieve gpu representation of a mesh.
    ///
    /// Uploads to gpu if not already done.
    pub(crate) fn get_or_create_gpu_resource(
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        handle: MeshHandle,
    ) -> Result<GpuMesh, ResourceManagerError> {
        ctx.meshes
            .manager
            .get_or_create_gpu_resource(handle, |resource, _lifetime| {
                GpuMesh::new(&mut ctx.resource_pools, device, queue, resource)
            })
    }

    pub(crate) fn frame_maintenance(&mut self, frame_index: u64) {
        self.manager.frame_maintenance(frame_index);
    }
}
