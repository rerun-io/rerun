use super::scene::MeshSourceData;
use crate::mesh_loader::{CpuMesh, GpuMesh};
use egui::util::hash;
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
                re_log::debug!("Loading CPU mesh {name:?}…");

                let result = match mesh_data {
                    MeshSourceData::Mesh3D(mesh3d) => CpuMesh::load(name.to_owned(), mesh3d),
                    MeshSourceData::StaticGlb(glb_bytes) => {
                        CpuMesh::load_raw(name.to_owned(), MeshFormat::Glb, glb_bytes)
                    }
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
    pub fn cylinder(&mut self) -> (u64, Arc<CpuMesh>) {
        crate::profile_function!();
        let mesh_id = hash("CYLINDER_MESH");
        let mesh = self
            .0
            .entry(mesh_id)
            .or_insert_with(|| {
                re_log::debug!("Generating CPU mesh for cylinder.");
                Some(Arc::new(CpuMesh::cylinder(4)))
            })
            .clone()
            .unwrap();
        (mesh_id, mesh)
    }

    /// Returns a cached cone mesh built around the x-axis in the range [0..1] and with radius 1 at -1.0. The default material is used.
    pub fn cone(&mut self) -> (u64, Arc<CpuMesh>) {
        crate::profile_function!();
        let mesh_id = hash("CONE_MESH");
        let mesh = self
            .0
            .entry(mesh_id)
            .or_insert_with(|| {
                re_log::debug!("Generating CPU mesh for cone.");
                Some(Arc::new(CpuMesh::cone(4)))
            })
            .clone()
            .unwrap();
        (mesh_id, mesh)
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
            .or_insert_with(|| Some(cpu_mesh.to_gpu(three_d)));
    }

    pub fn set_instances(&mut self, mesh_id: u64, instances: &three_d::Instances) {
        if let Some(Some(gpu_mesh)) = self.0.get_mut(&mesh_id) {
            for model in &mut gpu_mesh.meshes {
                model.set_instances(instances);
            }
        }
    }

    pub fn get(&self, mesh_id: u64) -> Option<&GpuMesh> {
        self.0.get(&mesh_id)?.as_ref()
    }
}

// ----------------------------------------------------------------------------

impl re_memory::GenNode for CpuMeshCache {
    fn node(&self, global: &mut re_memory::Global) -> re_memory::Node {
        let mut summary = re_memory::Summary::default();
        for (key, value) in &self.0 {
            summary.add_fixed(std::mem::size_of_val(key));
            if let Some(value) = value {
                summary.shared += global.sum_up_arc(value);
            }
        }
        summary.into()
    }
}
