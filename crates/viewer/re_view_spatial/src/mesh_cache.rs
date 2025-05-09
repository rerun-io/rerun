use std::sync::Arc;

use ahash::{HashMap, HashSet};

use itertools::Either;
use re_chunk_store::{ChunkStoreEvent, RowId};
use re_entity_db::VersionedInstancePathHash;
use re_log_types::hash::Hash64;
use re_renderer::RenderContext;
use re_types::{Component as _, components::MediaType};
use re_viewer_context::Cache;

use crate::mesh_loader::{LoadedMesh, NativeAsset3D, NativeMesh3D};

// ----------------------------------------------------------------------------

/// Key used for caching [`LoadedMesh`]es.
///
/// Note that this is more complex than most other caches,
/// since the cache key is not only used for mesh file blobs,
/// but also for manually logged meshes.
//
// TODO(andreas): Maybe these should be different concerns?
// Blobs need costly unpacking/reading/parsing, regular meshes don't.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct MeshCacheKey {
    pub versioned_instance_path_hash: VersionedInstancePathHash,
    pub query_result_hash: Hash64,
    pub media_type: Option<MediaType>,
}

/// Caches meshes based on their [`MeshCacheKey`].
#[derive(Default)]
pub struct MeshCache(HashMap<RowId, HashMap<MeshCacheKey, Option<Arc<LoadedMesh>>>>);

/// Either a [`re_types::archetypes::Asset3D`] or [`re_types::archetypes::Mesh3D`] to be cached.
#[derive(Debug, Clone)]
pub enum AnyMesh<'a> {
    Asset {
        asset: NativeAsset3D<'a>,
    },
    Mesh {
        mesh: NativeMesh3D<'a>,

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
        self.0
            .entry(key.versioned_instance_path_hash.row_id)
            .or_default()
            .entry(key)
            .or_insert_with(|| {
                re_log::trace!("Loading CPU mesh {name:?}…");

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
    fn purge_memory(&mut self) {
        self.0.clear();
    }

    fn on_store_events(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        let row_ids_removed: HashSet<RowId> = events
            .iter()
            .flat_map(|event| {
                let is_deletion = || event.kind == re_chunk_store::ChunkStoreDiffKind::Deletion;
                let contains_mesh_data = || {
                    let contains_asset_blob = event
                        .chunk
                        .components()
                        .contains_component(&Asset3D::descriptor_blob());

                    let contains_vertex_positions = event
                        .chunk
                        .components()
                        .contains_component(&Mesh3D::descriptor_vertex_positions());

                    contains_asset_blob || contains_vertex_positions
                };

                if is_deletion() && contains_mesh_data() {
                    Either::Left(event.chunk.row_ids())
                } else {
                    Either::Right(std::iter::empty())
                }
            })
            .collect();

        self.0
            .retain(|row_id, _per_key| !row_ids_removed.contains(row_id));
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
