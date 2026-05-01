use std::sync::Arc;

use ahash::{HashMap, HashMapExt as _};
use nohash_hasher::{IntMap, IntSet};

use re_chunk::{Chunk, ChunkId};
use re_log_encoding::{ChunkProvider, RawRrdManifest, RrdManifest};
use re_log_types::{AbsoluteTimeRange, EntityPath, StoreId, Timeline};

use crate::{
    ChunkStore, ChunkStoreConfig, ChunkStoreHandle, ChunkStoreResult, ChunkTrackingMode,
    EntityTree, ExtractPropertiesError, LatestAtQuery, QueryResults, RangeQuery, StoreSchema,
    extract_properties_from_chunks,
};

/// A [`ChunkStore`] backed by a [`ChunkProvider`], with index loaded but chunks loaded on demand.
///
/// Constructed from a [`ChunkProvider`]; store selection (which manifest to extract from the
/// `RrdFooter`, etc.) is the provider's concern.
///
/// On construction, the `ChunkStore`'s virtual index is populated via `insert_rrd_manifest()`.
/// Physical chunks are **never retained** in the inner store — [`Self::load_chunks`] forwards to
/// the provider and returns the `Vec<Arc<Chunk>>` to the caller, who is responsible for the
/// resulting memory. This is deliberately cache-free to keep the OSS server from `OOMing` on
/// large RRDs.
//TODO(RR-4503): caching support.
pub struct LazyStore {
    store: ChunkStoreHandle,
    provider: Arc<dyn ChunkProvider>,

    /// Precomputed map from `ChunkId` to manifest row index.
    chunk_id_to_index: HashMap<ChunkId, usize>,

    /// Precomputed per-chunk timeline ranges.
    timeline_ranges: HashMap<ChunkId, IntMap<Timeline, AbsoluteTimeRange>>,
}

impl LazyStore {
    /// Build a lazy store from any chunk provider.
    ///
    /// The provider's manifest is used to populate the inner [`ChunkStore`]'s virtual index; the
    /// provider's `load_chunks` serves on-demand reads.
    ///
    /// Infallible: every fallible step (manifest parsing, file open, etc.) happens during the
    /// provider's own construction.
    pub fn new(provider: Arc<dyn ChunkProvider>) -> Self {
        let manifest = Arc::clone(provider.manifest());

        // `ALL_DISABLED` here is irrelevant, this store will never see chunks.
        let mut store =
            ChunkStore::new(manifest.store_id().clone(), ChunkStoreConfig::ALL_DISABLED);

        #[expect(clippy::let_underscore_must_use)]
        let _ = store.insert_rrd_manifest(Arc::clone(&manifest));

        let chunk_id_to_index: HashMap<ChunkId, usize> = manifest
            .col_chunk_ids()
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, i))
            .collect();

        let timeline_ranges = Self::build_timeline_ranges(&manifest);

        Self {
            store: ChunkStoreHandle::new(store),
            provider,
            chunk_id_to_index,
            timeline_ranges,
        }
    }

    fn build_timeline_ranges(
        manifest: &RrdManifest,
    ) -> HashMap<ChunkId, IntMap<Timeline, AbsoluteTimeRange>> {
        let mut result: HashMap<ChunkId, IntMap<Timeline, AbsoluteTimeRange>> = HashMap::new();
        for per_entity in manifest.temporal_map().values() {
            for (timeline, per_component) in per_entity {
                for per_chunk in per_component.values() {
                    for (&chunk_id, entry) in per_chunk {
                        let e = result.entry(chunk_id).or_default();
                        e.entry(*timeline)
                            .and_modify(|existing| {
                                *existing = existing.union(entry.time_range);
                            })
                            .or_insert(entry.time_range);
                    }
                }
            }
        }
        result
    }

    /// Load specific chunks via the underlying provider.
    ///
    /// The inner [`ChunkStore`] is **not** mutated — it stays purely virtual for the lifetime of
    /// this store. The caller owns the returned `Vec<Arc<Chunk>>`; dropping it frees the memory.
    /// Returns an error if any chunk ID is not in the manifest.
    pub fn load_chunks(&self, chunk_ids: &[ChunkId]) -> ChunkStoreResult<Vec<Arc<Chunk>>> {
        Ok(self.provider.load_chunks(chunk_ids)?)
    }

    /// Load every chunk in the manifest and return them in a single [`Vec`].
    ///
    /// Memory cost scales with the full RRD — consider streaming for large stores.
    pub fn load_all_chunks(&self) -> ChunkStoreResult<Vec<Arc<Chunk>>> {
        self.load_chunks(self.manifest().col_chunk_ids())
    }

    /// The store's schema, populated from the manifest (available without loading chunks).
    #[inline]
    pub fn schema(&self) -> StoreSchema {
        self.store.read().schema().clone()
    }

    /// The entity tree, populated from the manifest (available without loading chunks).
    pub fn entity_tree(&self) -> EntityTree {
        self.store.read().entity_tree().clone()
    }

    /// The number of chunks described by the manifest (physical + virtual).
    pub fn num_chunks(&self) -> usize {
        self.manifest().num_chunks()
    }

    /// The parsed manifest for this store.
    pub fn manifest(&self) -> &Arc<RrdManifest> {
        self.provider.manifest()
    }

    /// The raw manifest as-parsed from the RRD footer, before validation/extraction.
    ///
    /// Kept around so the server can synthesize `GetRrdManifest` responses without materializing
    /// chunks: the footer already contains everything a client needs to pick which chunks to fetch.
    pub fn raw_manifest(&self) -> &Arc<RawRrdManifest> {
        self.provider.raw_manifest()
    }

    /// The underlying chunk provider.
    pub fn provider(&self) -> &Arc<dyn ChunkProvider> {
        &self.provider
    }

    /// Look up the manifest row index for a given chunk ID.
    pub fn chunk_row_index(&self, chunk_id: &ChunkId) -> Option<usize> {
        self.chunk_id_to_index.get(chunk_id).copied()
    }

    /// Per-chunk timeline ranges.
    pub fn timeline_ranges(&self) -> &HashMap<ChunkId, IntMap<Timeline, AbsoluteTimeRange>> {
        &self.timeline_ranges
    }

    /// The store ID (from the manifest, no store lock needed).
    pub fn store_id(&self) -> &StoreId {
        self.manifest().store_id()
    }

    /// All entity paths known to this store (populated from the virtual index).
    pub fn all_entities(&self) -> IntSet<EntityPath> {
        self.store.read().all_entities()
    }

    /// Extract properties in a single pass: query the virtual index for required property
    /// chunks, load them from the provider, and run the extraction.
    pub fn extract_properties(&self) -> Result<arrow::array::RecordBatch, ExtractPropertiesError> {
        let per_entity = self.store.read().property_entities_query_results();

        let ids: Vec<ChunkId> = per_entity
            .iter()
            .flat_map(|(_, qr)| {
                qr.chunks
                    .iter()
                    .map(|c| c.id())
                    .chain(qr.missing_virtual.iter().copied())
            })
            .collect();

        let chunks = self
            .load_chunks(&ids)
            .map_err(|err| ExtractPropertiesError::Internal(err.to_string()))?;

        extract_properties_from_chunks(&per_entity, &chunks)
    }

    /// Run a latest-at query against the virtual index.
    ///
    /// Returns [`QueryResults`] with physical chunks in `chunks` and
    /// not-yet-loaded chunk IDs in `missing_virtual`.
    pub fn latest_at_relevant_chunks_for_all_components(
        &self,
        report_mode: ChunkTrackingMode,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        include_static: bool,
    ) -> QueryResults {
        self.store
            .read()
            .latest_at_relevant_chunks_for_all_components(
                report_mode,
                query,
                entity_path,
                include_static,
            )
    }

    /// Run a range query against the virtual index.
    ///
    /// Returns [`QueryResults`] with physical chunks in `chunks` and
    /// not-yet-loaded chunk IDs in `missing_virtual`.
    pub fn range_relevant_chunks_for_all_components(
        &self,
        report_mode: ChunkTrackingMode,
        query: &RangeQuery,
        entity_path: &EntityPath,
        include_static: bool,
    ) -> QueryResults {
        self.store.read().range_relevant_chunks_for_all_components(
            report_mode,
            query,
            entity_path,
            include_static,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::path::Path;

    use super::*;

    use re_chunk::{RowId, TimePoint, Timeline};
    use re_log_encoding::EncodingOptions;
    use re_log_types::{
        EntityPath, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource,
        example_components::{MyPoint, MyPoints},
    };

    /// Helper: create test chunks and encode to RRD file at `path`.
    /// Returns `(open file handle, store_id, chunks)`.
    fn create_test_rrd(
        path: &Path,
        num_entities: usize,
        num_frames: usize,
    ) -> (File, StoreId, Vec<Arc<Chunk>>) {
        let store_id = StoreId::random(StoreKind::Recording, "test");
        let store_info = StoreInfo::new(store_id.clone(), StoreSource::Unknown);
        let timeline = Timeline::new_sequence("frame");

        let mut chunks = Vec::new();
        for entity_idx in 0..num_entities {
            for frame_idx in 0..num_frames {
                let entity_path = EntityPath::from(format!("/entity_{entity_idx}"));
                let row_id = RowId::new();
                let points = MyPoint::from_iter(frame_idx as u32..frame_idx as u32 + 1);
                let chunk = Chunk::builder(entity_path)
                    .with_sparse_component_batches(
                        row_id,
                        #[expect(clippy::cast_possible_wrap)]
                        TimePoint::default().with(timeline, frame_idx as i64),
                        [(MyPoints::descriptor_points(), Some(&points as _))],
                    )
                    .build()
                    .unwrap();
                chunks.push(Arc::new(chunk));
            }
        }

        // Encode to file.
        let set_store_info = LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *RowId::ZERO,
            info: store_info,
        });
        let mut file = std::fs::File::create(path).unwrap();
        let mut encoder = re_log_encoding::Encoder::new_eager(
            re_log_encoding::CrateVersion::LOCAL,
            EncodingOptions::PROTOBUF_COMPRESSED,
            &mut file,
        )
        .unwrap();
        encoder.append(&set_store_info).unwrap();
        for chunk in &chunks {
            let arrow_msg = chunk.to_arrow_msg().unwrap();
            let msg = LogMsg::ArrowMsg(store_id.clone(), arrow_msg);
            encoder.append(&msg).unwrap();
        }
        encoder.finish().unwrap();

        // Re-open for reading.
        let file = File::open(path).unwrap();
        (file, store_id, chunks)
    }

    fn read_raw_manifest(file: &mut File, store_id: &StoreId) -> Arc<RawRrdManifest> {
        let footer = re_log_encoding::read_rrd_footer(file).unwrap().unwrap();
        Arc::new(footer.manifests[store_id].clone())
    }

    /// Construct a `LazyStore` from an open RRD file, via `RrdChunkProvider`.
    fn build_test_lazy_store(file: File, raw_manifest: Arc<RawRrdManifest>) -> LazyStore {
        let provider = Arc::new(
            re_log_encoding::RrdChunkProvider::try_new(file, raw_manifest)
                .expect("test rrd provider"),
        );
        LazyStore::new(provider)
    }

    #[test]
    fn test_lazy_store_no_physical_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let (mut file, store_id, chunks) = create_test_rrd(&dir.path().join("test.rrd"), 2, 3);
        let raw = read_raw_manifest(&mut file, &store_id);

        let lazy = build_test_lazy_store(file, raw);

        assert_eq!(lazy.store.read().num_physical_chunks(), 0);
        assert_eq!(
            lazy.manifest().col_chunk_ids().len(),
            chunks.len(),
            "All chunk IDs should be in manifest"
        );
    }

    #[test]
    fn test_lazy_store_entities_visible() {
        let dir = tempfile::tempdir().unwrap();
        let (mut file, store_id, _) = create_test_rrd(&dir.path().join("test.rrd"), 3, 2);
        let raw = read_raw_manifest(&mut file, &store_id);

        let lazy = build_test_lazy_store(file, raw);
        let entity_tree = lazy.entity_tree();

        let mut entities = Vec::new();
        entity_tree.visit_children_recursively(|path| {
            if !path.is_root() {
                entities.push(path.clone());
            }
        });
        // 3 entities + intermediate paths
        assert!(entities.len() >= 3, "Should have at least 3 leaf entities");
    }

    #[test]
    fn test_lazy_store_load_all() {
        let dir = tempfile::tempdir().unwrap();
        let (mut file, store_id, chunks) = create_test_rrd(&dir.path().join("test.rrd"), 2, 3);
        let raw = read_raw_manifest(&mut file, &store_id);

        let lazy = build_test_lazy_store(file, raw);
        let loaded = lazy.load_all_chunks().unwrap();
        assert_eq!(loaded.len(), chunks.len());
    }

    #[test]
    fn test_lazy_store_load_single_chunk() {
        let dir = tempfile::tempdir().unwrap();
        let (mut file, store_id, chunks) = create_test_rrd(&dir.path().join("test.rrd"), 2, 3);
        let raw = read_raw_manifest(&mut file, &store_id);

        let lazy = build_test_lazy_store(file, raw);
        let first_chunk_id = lazy.manifest().col_chunk_ids()[0];
        let loaded = lazy.load_chunks(&[first_chunk_id]).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id(), first_chunk_id);

        // Other chunks are still virtual.
        let total_chunks = chunks.len();
        assert!(total_chunks > 1);
    }

    #[test]
    fn test_lazy_store_load_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let (mut file, store_id, _) = create_test_rrd(&dir.path().join("test.rrd"), 1, 3);
        let raw = read_raw_manifest(&mut file, &store_id);

        let lazy = build_test_lazy_store(file, raw);

        // Calling `load_chunks` twice with the same IDs yields equivalent results, and neither
        // call retains chunks in the inner store — guard against anyone sneaking a cache back in.
        let ids = lazy.manifest().col_chunk_ids();
        let first = lazy.load_chunks(ids).unwrap();
        let second = lazy.load_chunks(ids).unwrap();

        assert_eq!(first.len(), second.len());
        let mut first_ids: Vec<_> = first.iter().map(|c| c.id()).collect();
        let mut second_ids: Vec<_> = second.iter().map(|c| c.id()).collect();
        first_ids.sort();
        second_ids.sort();
        assert_eq!(first_ids, second_ids);

        assert_eq!(
            lazy.store.read().num_physical_chunks(),
            0,
            "no-cache: inner store must stay empty across loads"
        );
    }

    #[test]
    fn test_lazy_store_load_does_not_retain() {
        let dir = tempfile::tempdir().unwrap();
        let (mut file, store_id, _) = create_test_rrd(&dir.path().join("test.rrd"), 2, 3);
        let raw = read_raw_manifest(&mut file, &store_id);

        let lazy = build_test_lazy_store(file, raw);
        let first_chunk_id = lazy.manifest().col_chunk_ids()[0];
        let loaded = lazy.load_chunks(&[first_chunk_id]).unwrap();
        assert_eq!(loaded.len(), 1);

        drop(loaded);

        assert_eq!(
            lazy.store.read().num_physical_chunks(),
            0,
            "dropping the returned Vec must free the chunk; inner store is not a cache"
        );
    }

    #[test]
    fn test_lazy_store_extract_properties() {
        // Build an RRD with a single property entity, extract properties, and assert that the
        // inner store remains empty afterwards.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("props.rrd");
        let store_id = StoreId::random(StoreKind::Recording, "props");
        let store_info = StoreInfo::new(store_id.clone(), StoreSource::Unknown);

        let property_entity = EntityPath::from("/__properties/my_prop");
        let row_id = RowId::new();
        let points = MyPoint::from_iter(0..1);
        let chunk = Chunk::builder(property_entity)
            .with_sparse_component_batches(
                row_id,
                TimePoint::default(),
                [(MyPoints::descriptor_points(), Some(&points as _))],
            )
            .build()
            .unwrap();
        let chunk = Arc::new(chunk);

        let mut file = std::fs::File::create(&path).unwrap();
        let mut encoder = re_log_encoding::Encoder::new_eager(
            re_log_encoding::CrateVersion::LOCAL,
            EncodingOptions::PROTOBUF_COMPRESSED,
            &mut file,
        )
        .unwrap();
        encoder
            .append(&LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: *RowId::ZERO,
                info: store_info,
            }))
            .unwrap();
        let arrow_msg = chunk.to_arrow_msg().unwrap();
        encoder
            .append(&LogMsg::ArrowMsg(store_id.clone(), arrow_msg))
            .unwrap();
        encoder.finish().unwrap();

        let mut file = File::open(&path).unwrap();
        let raw = read_raw_manifest(&mut file, &store_id);

        let lazy = build_test_lazy_store(file, raw);
        let batch = lazy.extract_properties().unwrap();
        assert!(
            batch.num_columns() > 0,
            "properties record batch should contain the property column"
        );
        assert_eq!(
            lazy.store.read().num_physical_chunks(),
            0,
            "extract_properties must not retain any chunks"
        );
    }

    #[test]
    fn test_lazy_store_schema() {
        let dir = tempfile::tempdir().unwrap();
        let (mut file, store_id, _) = create_test_rrd(&dir.path().join("test.rrd"), 2, 3);
        let raw = read_raw_manifest(&mut file, &store_id);

        let lazy = build_test_lazy_store(file, raw);
        let schema = lazy.schema();

        // Schema should be non-empty even without physical chunks.
        let columns = schema.chunk_column_descriptors();
        assert!(
            !columns.components.is_empty() || !columns.indices.is_empty(),
            "Schema should be populated from manifest"
        );
    }

    #[test]
    fn test_lazy_vs_eager_equivalence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.rrd");
        let (mut file, store_id, _) = create_test_rrd(&path, 2, 3);
        let raw = read_raw_manifest(&mut file, &store_id);

        // Lazy path: create lazy store, load all chunks via the no-cache API.
        let lazy = build_test_lazy_store(file, raw);
        let lazy_chunks = lazy.load_all_chunks().unwrap();

        // Eager path: load the same file fully.
        let eager_stores =
            ChunkStore::from_rrd_filepath(&ChunkStoreConfig::ALL_DISABLED, &path).unwrap();
        let eager_store = eager_stores.into_values().next().unwrap();

        let collect_entities = |tree: &crate::EntityTree| {
            let mut entities = Vec::new();
            tree.visit_children_recursively(|path| {
                if !path.is_root() {
                    entities.push(path.clone());
                }
            });
            entities.sort();
            entities
        };
        let lazy_entities = collect_entities(&lazy.entity_tree());
        let eager_entities = collect_entities(eager_store.entity_tree());

        assert_eq!(lazy_entities, eager_entities, "Same entities");

        let mut lazy_ids: Vec<_> = lazy_chunks.iter().map(|c| c.id()).collect();
        let mut eager_ids: Vec<_> = eager_store.iter_physical_chunks().map(|c| c.id()).collect();
        lazy_ids.sort();
        eager_ids.sort();
        assert_eq!(lazy_ids, eager_ids, "Same set of chunks");
    }
}
