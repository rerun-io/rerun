use std::sync::Arc;

use re_log_types::{MeshFormat, MeshId};

use crate::mesh_loader::CpuMesh;

use super::scene::MeshSourceData;

#[cfg(feature = "wgpu")]
use re_renderer::resource_managers::{MeshManager, TextureManager2D};

// ----------------------------------------------------------------------------

#[derive(Default)]
pub struct CpuMeshCache(nohash_hasher::IntMap<MeshId, Option<Arc<CpuMesh>>>);

impl CpuMeshCache {
    pub fn load(
        &mut self,
        name: &str,
        mesh_data: &MeshSourceData,
        #[cfg(feature = "wgpu")] mesh_manager: &mut MeshManager,
        #[cfg(feature = "wgpu")] texture_manager: &mut TextureManager2D,
    ) -> Option<Arc<CpuMesh>> {
        crate::profile_function!();

        let mesh_id = mesh_data.mesh_id();

        self.0
            .entry(mesh_id)
            .or_insert_with(|| {
                re_log::debug!("Loading CPU mesh {name:?}…");

                let result = match mesh_data {
                    MeshSourceData::Mesh3D(mesh3d) => CpuMesh::load(
                        name.to_owned(),
                        mesh3d,
                        #[cfg(feature = "wgpu")]
                        mesh_manager,
                        #[cfg(feature = "wgpu")]
                        texture_manager,
                    ),
                    MeshSourceData::StaticGlb(_mesh_id, glb_bytes) => CpuMesh::load_raw(
                        mesh_id,
                        name.to_owned(),
                        MeshFormat::Glb,
                        glb_bytes,
                        #[cfg(feature = "wgpu")]
                        mesh_manager,
                        #[cfg(feature = "wgpu")]
                        texture_manager,
                    ),
                };

                match result {
                    Ok(cpu_mesh) => Some(Arc::new(cpu_mesh)),
                    Err(err) => {
                        re_log::warn!("Failed to load mesh {name:?}: {}", re_error::format(&err));
                        None
                    }
                }
            })
            .clone()
    }

    /// Returns a cached cylinder mesh built around the x-axis in the range [0..1] and with radius 1. The default material is used.
    #[cfg(feature = "glow")]
    pub fn cylinder(&mut self) -> Arc<CpuMesh> {
        crate::profile_function!();
        let mesh_id = MeshId(uuid::uuid!("4a9b0c13-89a3-4648-968d-0f40cf0afb7d"));
        let mesh = self
            .0
            .entry(mesh_id)
            .or_insert_with(|| {
                re_log::debug!("Generating CPU mesh for cylinder.");
                Some(Arc::new(CpuMesh::cylinder(mesh_id, 4)))
            })
            .clone()
            .unwrap();
        mesh
    }

    /// Returns a cached cone mesh built around the x-axis in the range [0..1] and with radius 1 at -1.0. The default material is used.
    #[cfg(feature = "glow")]
    pub fn cone(&mut self) -> Arc<CpuMesh> {
        crate::profile_function!();
        let mesh_id = MeshId(uuid::uuid!("c4c87d1f-9cf9-4f56-9c60-dec3e65892ff"));
        let mesh = self
            .0
            .entry(mesh_id)
            .or_insert_with(|| {
                re_log::debug!("Generating CPU mesh for cone.");
                Some(Arc::new(CpuMesh::cone(mesh_id, 4)))
            })
            .clone()
            .unwrap();
        mesh
    }
}

// ----------------------------------------------------------------------------

#[cfg(feature = "glow")]
#[derive(Default)]
pub struct GpuMeshCache(nohash_hasher::IntMap<MeshId, Option<crate::misc::mesh_loader::GpuMesh>>);

#[cfg(feature = "glow")]
impl GpuMeshCache {
    pub fn load(&mut self, three_d: &three_d::Context, mesh_id: MeshId, cpu_mesh: &CpuMesh) {
        crate::profile_function!();
        self.0
            .entry(mesh_id)
            .or_insert_with(|| Some(cpu_mesh.to_gpu(three_d)));
    }

    pub fn set_instances(&mut self, mesh_id: MeshId, instances: &three_d::Instances) {
        if let Some(Some(gpu_mesh)) = self.0.get_mut(&mesh_id) {
            for model in &mut gpu_mesh.meshes {
                model.set_instances(instances);
            }
        }
    }

    pub fn get(&self, mesh_id: MeshId) -> Option<&crate::misc::mesh_loader::GpuMesh> {
        self.0.get(&mesh_id)?.as_ref()
    }
}
