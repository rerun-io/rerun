use std::sync::Arc;

use re_entity_db::VersionedInstancePathHash;
use re_log_types::hash::Hash64;
use re_renderer::RenderContext;
use re_types::components::MediaType;
use re_viewer_context::Cache;

use crate::mesh_loader::LoadedMesh;

// ----------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct MeshCacheKey {
    pub versioned_instance_path_hash: VersionedInstancePathHash,
    pub query_result_hash: Hash64,
    pub media_type: Option<MediaType>,
}

/// Caches meshes based on their [`MeshCacheKey`].
#[derive(Default)]
pub struct MeshCache(ahash::HashMap<MeshCacheKey, Option<Arc<LoadedMesh>>>);

/// Either a [`re_types::archetypes::Asset3D`] or [`re_types::archetypes::Mesh3D`] to be cached.
#[derive(Debug, Clone, Copy)]
pub enum AnyMesh<'a> {
    Asset(&'a re_types::archetypes::Asset3D),
    Mesh {
        mesh: &'a re_types::archetypes::Mesh3D,

        /// If there are any textures associated with that mesh (albedo etc), they use this
        /// hash for texture manager lookup.
        texture_key: u64,
    },
}

impl MeshCache {
    pub fn entry(
        &mut self,
        name: &str,
        key: MeshCacheKey,
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
