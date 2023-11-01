use crate::{
    mesh::{GpuMesh, Mesh},
    renderer::MeshRenderer,
    wgpu_resources::GpuBindGroupLayoutHandle,
    RenderContext,
};

use super::{
    resource_manager::ResourceManager, ResourceHandle, ResourceLifeTime, ResourceManagerError,
};

slotmap::new_key_type! { pub struct MeshHandleInner; }

pub type GpuMeshHandle = ResourceHandle<MeshHandleInner>;

pub struct MeshManager {
    manager: ResourceManager<MeshHandleInner, GpuMesh>,
    mesh_bind_group_layout: GpuBindGroupLayoutHandle,
}

impl MeshManager {
    pub(crate) fn new(mesh_renderer: &MeshRenderer) -> Self {
        MeshManager {
            manager: Default::default(),
            mesh_bind_group_layout: mesh_renderer.bind_group_layout,
        }
    }

    /// Takes ownership of a mesh.
    pub fn create(
        &mut self,
        ctx: &RenderContext,
        mesh: &Mesh,
        lifetime: ResourceLifeTime,
    ) -> Result<GpuMeshHandle, ResourceManagerError> {
        re_tracing::profile_function!();
        Ok(self.manager.store_resource(
            GpuMesh::new(ctx, self.mesh_bind_group_layout, mesh)?,
            lifetime,
        ))
    }

    /// Accesses a given resource.
    pub(crate) fn get(&self, handle: &GpuMeshHandle) -> Result<&GpuMesh, ResourceManagerError> {
        self.manager.get(handle)
    }

    pub(crate) fn begin_frame(&mut self, frame_index: u64) {
        self.manager.begin_frame(frame_index);
    }
}
