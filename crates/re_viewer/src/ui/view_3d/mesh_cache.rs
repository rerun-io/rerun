use std::sync::Arc;

use re_log_types::{MeshFormat, MeshId};

use crate::mesh_loader::CpuMesh;

use super::scene::MeshSourceData;

use re_renderer::RenderContext;

// ----------------------------------------------------------------------------

#[derive(Default)]
pub struct CpuMeshCache(nohash_hasher::IntMap<MeshId, Option<Arc<CpuMesh>>>);

impl CpuMeshCache {
    pub fn load(
        &mut self,
        name: &str,
        mesh_data: &MeshSourceData,
        render_ctx: &mut RenderContext,
    ) -> Option<Arc<CpuMesh>> {
        crate::profile_function!();

        let mesh_id = mesh_data.mesh_id();

        self.0
            .entry(mesh_id)
            .or_insert_with(|| {
                re_log::debug!("Loading CPU mesh {name:?}…");

                let result = match mesh_data {
                    MeshSourceData::Mesh3D(mesh3d) => {
                        CpuMesh::load(name.to_owned(), mesh3d, render_ctx)
                    }
                    MeshSourceData::StaticGlb(_mesh_id, glb_bytes) => {
                        CpuMesh::load_raw(name.to_owned(), MeshFormat::Glb, glb_bytes, render_ctx)
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
}
