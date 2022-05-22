use super::scene::MeshSourceData;
use crate::mesh_loader::GpuMesh;
use log_types::MeshFormat;

#[derive(Default)]
pub struct MeshCache(nohash_hasher::IntMap<u64, Option<GpuMesh>>);

impl MeshCache {
    pub fn load(
        &mut self,
        three_d: &three_d::Context,
        mesh_id: u64,
        name: &str,
        mesh_data: &MeshSourceData,
    ) {
        crate::profile_function!();
        self.0.entry(mesh_id).or_insert_with(|| {
            tracing::debug!("Loading mesh {}…", name);
            let result = match mesh_data {
                MeshSourceData::Mesh3D(mesh3d) => {
                    crate::mesh_loader::load(three_d, name.to_owned(), mesh3d)
                }
                MeshSourceData::StaticGlb(glb_bytes) => crate::mesh_loader::load_raw(
                    three_d,
                    name.to_owned(),
                    MeshFormat::Glb,
                    glb_bytes,
                ),
            };

            match result {
                Ok(gpu_mesh) => Some(gpu_mesh),
                Err(err) => {
                    tracing::warn!("{}: Failed to load mesh: {}", name, err);
                    None
                }
            }
        });
    }

    pub fn set_instances(
        &mut self,
        mesh_id: u64,
        instances: &three_d::Instances,
    ) -> three_d::ThreeDResult<()> {
        if let Some(Some(gpu_mesh)) = self.0.get_mut(&mesh_id) {
            for model in &mut gpu_mesh.models {
                model.set_instances(instances)?;
            }
        }
        Ok(())
    }

    pub fn get(&self, mesh_id: u64) -> Option<&GpuMesh> {
        self.0.get(&mesh_id)?.as_ref()
    }
}
