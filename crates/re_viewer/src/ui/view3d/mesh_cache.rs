use super::scene::MeshSourceData;
use crate::mesh_loader::{CpuMesh, GpuMesh};
use re_log_types::MeshFormat;
use std::sync::Arc;

// ----------------------------------------------------------------------------

#[derive(Default)]
pub struct CpuMeshCache(nohash_hasher::IntMap<u64, Option<Arc<CpuMesh>>>);

impl CpuMeshCache {
    pub fn load(
        &mut self,
        mesh_id: u64,
        name: &str,
        mesh_data: &MeshSourceData,
    ) -> Option<Arc<CpuMesh>> {
        crate::profile_function!();

        self.0
            .entry(mesh_id)
            .or_insert_with(|| {
                tracing::debug!("Loading CPU mesh {name:?}…");

                let result = match mesh_data {
                    MeshSourceData::Mesh3D(mesh3d) => CpuMesh::load(name.to_owned(), mesh3d),
                    MeshSourceData::StaticGlb(glb_bytes) => {
                        CpuMesh::load_raw(name.to_owned(), MeshFormat::Glb, glb_bytes)
                    }
                };

                match result {
                    Ok(cpu_mesh) => Some(Arc::new(cpu_mesh)),
                    Err(err) => {
                        tracing::warn!("Failed to load mesh {name:?}: {}", re_error::format(&err));
                        None
                    }
                }
            })
            .clone()
    }
}

// ----------------------------------------------------------------------------

#[derive(Default)]
pub struct GpuMeshCache(nohash_hasher::IntMap<u64, Option<GpuMesh>>);

impl GpuMeshCache {
    pub fn load(&mut self, three_d: &three_d::Context, mesh_id: u64, cpu_mesh: &CpuMesh) {
        crate::profile_function!();
        self.0
            .entry(mesh_id)
            .or_insert_with(|| match cpu_mesh.to_gpu(three_d) {
                Ok(gpu_mesh) => Some(gpu_mesh),
                Err(err) => {
                    tracing::warn!(
                        "Failed to load mesh {:?}: {}",
                        cpu_mesh.name(),
                        re_error::format(&err)
                    );
                    None
                }
            });
    }

    pub fn set_instances(
        &mut self,
        mesh_id: u64,
        instances: &three_d::Instances,
    ) -> three_d::ThreeDResult<()> {
        if let Some(Some(gpu_mesh)) = self.0.get_mut(&mesh_id) {
            for model in &mut gpu_mesh.meshes {
                model.set_instances(instances)?;
            }
        }
        Ok(())
    }

    pub fn get(&self, mesh_id: u64) -> Option<&GpuMesh> {
        self.0.get(&mesh_id)?.as_ref()
    }
}
