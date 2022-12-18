use std::sync::Arc;

use crate::{
    mesh::{GpuMesh, Mesh},
    renderer::MeshRenderer,
    wgpu_resources::{GpuBindGroupLayoutHandle, WgpuResourcePools},
};

use super::{
    resource_manager::ResourceManager, ResourceHandle, ResourceLifeTime, ResourceManagerError,
    TextureManager2D,
};

slotmap::new_key_type! { pub struct MeshHandleInner; }

pub type GpuMeshHandle = ResourceHandle<MeshHandleInner>;

pub struct MeshManager {
    manager: ResourceManager<MeshHandleInner, GpuMesh>,
    mesh_bound_group_layout: GpuBindGroupLayoutHandle,

    // For convenience to reduce amount of times we need to pass them around
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl MeshManager {
    pub(crate) fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        mesh_renderer: &MeshRenderer,
    ) -> Self {
        MeshManager {
            manager: Default::default(),
            mesh_bound_group_layout: mesh_renderer.bind_group_layout,
            device,
            queue,
        }
    }

    /// Takes ownership of a mesh.
    pub fn create(
        &mut self,
        gpu_resources: &mut WgpuResourcePools,
        texture_manager_2d: &TextureManager2D,
        mesh: &Mesh,
        lifetime: ResourceLifeTime,
    ) -> Result<GpuMeshHandle, ResourceManagerError> {
        Ok(self.manager.store_resource(
            GpuMesh::new(
                gpu_resources,
                texture_manager_2d,
                self.mesh_bound_group_layout,
                &self.device,
                &self.queue,
                mesh,
            )?,
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
