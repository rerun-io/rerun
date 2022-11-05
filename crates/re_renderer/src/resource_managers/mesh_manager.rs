use crate::{
    mesh::{GpuMesh, Mesh},
    renderer::MeshRenderer,
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
    /// Takes ownership of a mesh.
    pub fn store_resource(&mut self, resource: Mesh, lifetime: ResourceLifeTime) -> MeshHandle {
        self.manager.store_resource(resource, lifetime)
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
        ctx.mesh_manager
            .manager
            .get_or_create_gpu_resource(handle, |resource, _lifetime| {
                // TODO(andreas): Use stack allocators for short lived meshes!
                GpuMesh::new(
                    &mut ctx.resource_pools,
                    &mut ctx.texture_manager_2d,
                    &ctx.renderers.get::<MeshRenderer>().unwrap(),
                    device,
                    queue,
                    resource,
                )
            })
    }

    /// Accesses a given resource under a read lock.
    pub(crate) fn get(&self, handle: MeshHandle) -> Result<&Mesh, ResourceManagerError> {
        self.manager.get(handle)
    }

    pub(crate) fn frame_maintenance(&mut self, frame_index: u64) {
        self.manager.frame_maintenance(frame_index);
    }
}
