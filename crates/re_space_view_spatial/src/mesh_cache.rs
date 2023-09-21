use std::sync::Arc;

use re_data_store::VersionedInstancePathHash;
use re_renderer::RenderContext;
use re_viewer_context::Cache;

use crate::mesh_loader::LoadedMesh;

// ----------------------------------------------------------------------------

/// Caches meshes based on their [`VersionedInstancePathHash`], i.e. a specific instance of a specific
/// entity path for a specific row in the store.
#[derive(Default)]
pub struct MeshCache(ahash::HashMap<VersionedInstancePathHash, Option<Arc<LoadedMesh>>>);

/// Either a `re_types::archetypes::Asset3D` or [`re_types::archetypes::Mesh3D`] to be cached.
#[derive(Debug, Clone, Copy)]
pub enum AnyMesh<'a> {
    Asset(&'a re_components::Mesh3D),
    Mesh(&'a re_types::archetypes::Mesh3D),
}

impl MeshCache {
    pub fn entry(
        &mut self,
        name: &str,
        key: VersionedInstancePathHash,
        mesh: AnyMesh<'_>,
        render_ctx: &RenderContext,
    ) -> Option<Arc<LoadedMesh>> {
        re_tracing::profile_function!();

        self.0
            .entry(key)
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
