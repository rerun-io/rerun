use std::sync::Arc;

use ahash::{HashMap, HashSet};
use itertools::Either;
use re_byte_size::SizeBytes as _;
use re_chunk_store::{ChunkStoreEvent, RowId};
use re_entity_db::{EntityDb, VersionedInstancePathHash};
use re_log_types::hash::Hash64;
use re_renderer::RenderContext;
use re_sdk_types::archetypes::{Asset3D, Mesh3D};
use re_sdk_types::components::MediaType;
use re_viewer_context::{Cache, CacheMemoryReport, CacheMemoryReportItem};

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

impl re_byte_size::SizeBytes for MeshCacheKey {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            versioned_instance_path_hash: _,
            query_result_hash: _,
            media_type,
        } = self;
        media_type.heap_size_bytes()
    }
}

struct MeshEntry {
    mesh: Option<Arc<LoadedMesh>>,
    last_used_generation: u64,
}

impl re_byte_size::SizeBytes for MeshEntry {
    fn heap_size_bytes(&self) -> u64 {
        self.mesh.heap_size_bytes()
    }
}

/// Caches meshes based on their [`MeshCacheKey`].
#[derive(Default)]
pub struct MeshCache {
    cache: HashMap<RowId, HashMap<MeshCacheKey, MeshEntry>>,
    generation: u64,
}

/// Either a [`re_sdk_types::archetypes::Asset3D`] or [`re_sdk_types::archetypes::Mesh3D`] to be cached.
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
        let entry = self
            .cache
            .entry(key.versioned_instance_path_hash.row_id)
            .or_default()
            .entry(key)
            .or_insert_with(|| {
                re_log::trace!("Loading CPU mesh {name:?}…");

                let result = LoadedMesh::load(name.to_owned(), mesh, render_ctx);

                match result {
                    Ok(cpu_mesh) => MeshEntry {
                        mesh: Some(Arc::new(cpu_mesh)),
                        last_used_generation: 0,
                    },
                    Err(err) => {
                        re_log::warn!("Failed to load mesh {name:?}: {}", re_error::format(&err));
                        MeshEntry {
                            mesh: None,
                            last_used_generation: 0,
                        }
                    }
                }
            });
        entry.last_used_generation = self.generation;

        entry.mesh.clone()
    }
}

impl Cache for MeshCache {
    fn begin_frame(&mut self) {
        // We aggressively clear caches that weren't used in the last frame because
        // `query_result_hash` in `MeshCacheKey` includes overrides in the hash. And
        // we currently have no way of knowing which hash should be removed because
        // of overrides changing.
        self.cache.retain(|_, meshes| {
            meshes.retain(|_, mesh| mesh.last_used_generation == self.generation);

            !meshes.is_empty()
        });
        self.generation += 1;
    }

    fn purge_memory(&mut self) {
        self.cache.clear();
    }

    fn memory_report(&self) -> CacheMemoryReport {
        let mut full_bytes_gpu = 0;
        let mut items: Vec<_> = self
            .cache
            .iter()
            .map(|(row_id, meshes)| {
                let bytes_gpu = meshes
                    .values()
                    .filter_map(|entry| entry.mesh.as_ref())
                    .map(|mesh| {
                        mesh.mesh_instances
                            .iter()
                            .map(|s| s.gpu_mesh.gpu_byte_size())
                            .sum::<u64>()
                    })
                    .sum();
                full_bytes_gpu += bytes_gpu;
                CacheMemoryReportItem {
                    item_name: row_id.short_string(),
                    bytes_cpu: meshes.total_size_bytes(),
                    bytes_gpu: Some(bytes_gpu),
                }
            })
            .collect();
        items.sort_by(|a, b| a.item_name.cmp(&b.item_name));
        CacheMemoryReport {
            bytes_cpu: self.cache.total_size_bytes(),
            bytes_gpu: Some(full_bytes_gpu),
            per_cache_item_info: items,
        }
    }

    fn name(&self) -> &'static str {
        "Meshes"
    }

    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], _entity_db: &EntityDb) {
        re_tracing::profile_function!();

        let row_ids_removed: HashSet<RowId> = events
            .iter()
            .flat_map(|event| {
                let is_deletion = || event.kind == re_chunk_store::ChunkStoreDiffKind::Deletion;
                let contains_mesh_data = || {
                    let contains_asset_blob = event
                        .chunk
                        .components()
                        .contains_component(Asset3D::descriptor_blob().component);

                    let contains_vertex_positions = event
                        .chunk
                        .components()
                        .contains_component(Mesh3D::descriptor_vertex_positions().component);

                    contains_asset_blob || contains_vertex_positions
                };

                if is_deletion() && contains_mesh_data() {
                    Either::Left(event.chunk.row_ids())
                } else {
                    Either::Right(std::iter::empty())
                }
            })
            .collect();

        self.cache
            .retain(|row_id, _meshes| !row_ids_removed.contains(row_id));
    }
}
