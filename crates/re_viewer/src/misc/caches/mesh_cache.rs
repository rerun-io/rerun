use std::sync::Arc;

use re_log_types::{Mesh3D, MeshId};
use re_renderer::RenderContext;
use re_viewer_context::Cache;

use crate::mesh_loader::LoadedMesh;

// ----------------------------------------------------------------------------

#[derive(Default)]
pub struct MeshCache(nohash_hasher::IntMap<MeshId, Option<Arc<LoadedMesh>>>);

impl MeshCache {
    pub fn entry(
        &mut self,
        name: &str,
        mesh: &Mesh3D,
        render_ctx: &mut RenderContext,
    ) -> Option<Arc<LoadedMesh>> {
        crate::profile_function!();

        let mesh_id = mesh.mesh_id();

        self.0
            .entry(mesh_id)
            .or_insert_with(|| {
                re_log::debug!("Loading CPU mesh {name:?}…");

                let result = LoadedMesh::load(name.to_owned(), mesh, render_ctx);

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

impl Cache for MeshCache {
    fn begin_frame(&mut self) {}

    fn purge_memory(&mut self) {
        self.0.clear();
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
