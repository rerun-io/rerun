use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ahash::{HashMap, HashMapExt as _};
use nohash_hasher::{IntMap, IntSet};
use parking_lot::Mutex;

use re_chunk::{Chunk, ChunkId};
use re_log_encoding::{CodecResult, RawRrdManifest, RrdManifest};
use re_log_types::{AbsoluteTimeRange, EntityPath, StoreId, Timeline};

use crate::{
    ChunkStore, ChunkStoreConfig, ChunkStoreHandle, ChunkStoreResult, ChunkTrackingMode,
    EntityTree, ExtractPropertiesError, LatestAtQuery, QueryResults, RangeQuery, StoreSchema,
};

/// A [`ChunkStore`] backed by an RRD file, with index loaded but chunks loaded on demand.
///
/// Constructed from a single store's [`RrdManifest`]. Store selection (which manifest to extract
/// from the `RrdFooter`) is the caller's responsibility.
///
/// On construction, the `ChunkStore`'s virtual index is populated via `insert_rrd_manifest()`.
/// No physical chunk data is loaded until [`Self::load_chunks`] or [`Self::load_all_chunks`]
/// is called.
///
/// Holds the RRD file open for the lifetime of the store, so that lazy chunk reads succeed
/// even if the file is deleted from the filesystem after construction.
//TODO(RR-4341): this abstraction is very primitive. We need a more general `ChunkProvider`-style
//  abstraction to cover for the many larger-than-RAM use cases.
pub struct LazyRrdStore {
    store: ChunkStoreHandle,
    file: Mutex<File>,
    rrd_path: PathBuf,
    raw_manifest: Arc<RawRrdManifest>,
    manifest: Arc<RrdManifest>,

    /// Precomputed map from `ChunkId` to manifest row index.
    chunk_id_to_index: HashMap<ChunkId, usize>,

    /// Precomputed per-chunk timeline ranges.
    timeline_ranges: HashMap<ChunkId, IntMap<Timeline, AbsoluteTimeRange>>,
}

impl LazyRrdStore {
    /// Create a new lazy store from a manifest and an open file handle.
    /// Populates the virtual index (no data loaded).
    ///
    /// The caller is responsible for reading the `RrdFooter` from the file and selecting
    /// the appropriate manifest (e.g. filtering by `StoreKind::Recording`). This keeps
    /// store-selection policy out of `re_chunk_store`. The manifest **must** come from
    /// the same file — byte offsets in the manifest are meaningless otherwise.
    ///
    /// `rrd_path` is kept for diagnostic messages only; all I/O goes through `file`.
    pub fn try_new(
        file: File,
        rrd_path: PathBuf,
        raw_manifest: Arc<RawRrdManifest>,
    ) -> CodecResult<Self> {
        let manifest = Arc::new(RrdManifest::try_new(&raw_manifest)?);

        // IMPORTANT: `ALL_DISABLED` here is load-bearing, since the `ChunkStore` is essentially
        // acting as a cache for the underlying RRD. Any compaction, etc. would lead to unexpected
        // consequences.
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

        Ok(Self {
            store: ChunkStoreHandle::new(store),
            file: Mutex::new(file),
            rrd_path,
            raw_manifest,
            manifest,
            chunk_id_to_index,
            timeline_ranges,
        })
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

    /// Load specific chunks from disk into the store.
    ///
    /// Chunks that are already physically loaded are skipped.
    /// Returns an error if any chunk ID is not in the manifest.
    /// All I/O happens without holding any store lock.
    pub fn load_chunks(&self, chunk_ids: &[ChunkId]) -> ChunkStoreResult<Vec<Arc<Chunk>>> {
        // 1. Filter out chunks that are already physical.
        let to_load: Vec<ChunkId> = {
            let guard = self.store.read();
            chunk_ids
                .iter()
                .filter(|id| guard.physical_chunk(id).is_none())
                .copied()
                .collect()
        };

        if to_load.is_empty() {
            return Ok(Vec::new());
        }

        // 2. Read from disk — NO store lock held.
        //    Returns `CodecError::ChunkNotInManifest` if any ID is unknown.
        let loaded = {
            let mut file = self.file.lock();
            re_log_encoding::read_chunks(&mut file, &self.manifest, &to_load)?
        };

        // 3. Insert into store.
        let mut store = self.store.write();
        for chunk in &loaded {
            // insert_chunk on an already-present ChunkId is a no-op.
            store.insert_chunk(chunk)?;
        }

        Ok(loaded)
    }

    /// Load all chunks from the RRD file into the store.
    pub fn load_all_chunks(&self) -> ChunkStoreResult<()> {
        self.load_chunks(self.manifest.col_chunk_ids())?;
        Ok(())
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
        self.manifest.num_chunks()
    }

    /// The number of chunks currently loaded in memory.
    pub fn num_physical_chunks(&self) -> usize {
        self.store.read().num_physical_chunks()
    }

    /// Whether a specific chunk is currently loaded in memory.
    pub fn has_physical_chunk(&self, chunk_id: &ChunkId) -> bool {
        self.store.read().physical_chunk(chunk_id).is_some()
    }

    /// Load all chunks, then return a compacted copy of the store.
    pub fn compacted(&self, options: &crate::CompactionOptions) -> ChunkStoreResult<ChunkStore> {
        self.load_all_chunks()?;
        self.store.read().compacted(options)
    }

    /// Load all chunks and return them.
    pub fn collect_physical_chunks(&self) -> ChunkStoreResult<Vec<Arc<Chunk>>> {
        self.load_all_chunks()?;
        Ok(self.store.read().iter_physical_chunks().cloned().collect())
    }

    /// Path to the source RRD file.
    pub fn rrd_path(&self) -> &Path {
        &self.rrd_path
    }

    /// The parsed manifest for this store.
    pub fn manifest(&self) -> &Arc<RrdManifest> {
        &self.manifest
    }

    /// The raw manifest as-parsed from the RRD footer, before validation/extraction.
    ///
    /// Kept around so the server can synthesize `GetRrdManifest` responses without materializing
    /// chunks: the footer already contains everything a client needs to pick which chunks to fetch.
    pub fn raw_manifest(&self) -> &Arc<RawRrdManifest> {
        &self.raw_manifest
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
        self.manifest.store_id()
    }

    /// All entity paths known to this store (populated from the virtual index).
    pub fn all_entities(&self) -> IntSet<EntityPath> {
        self.store.read().all_entities()
    }

    /// Get a physical chunk by ID if it's already loaded. Returns `None` for
    /// virtual-only chunks — use [`Self::load_chunks`] to materialize them first.
    pub fn physical_chunk(&self, id: &ChunkId) -> Option<Arc<Chunk>> {
        self.store.read().physical_chunk(id).cloned()
    }

    /// Extract properties, automatically loading the required property chunks
    /// on demand if they are still virtual.
    //TODO(RR-4458): currently takes one disk round-trip per property entity with virtual
    // chunks because `ChunkStore::extract_properties` short-circuits on the first missing
    // entity. Once it reports the full union of missing chunks, this will converge in a
    // single retry.
    pub fn extract_properties(&self) -> Result<arrow::array::RecordBatch, ExtractPropertiesError> {
        self.with_autoload(|store| store.extract_properties())
    }

    /// Run an operation against the inner [`ChunkStore`], auto-loading any chunks the
    /// operation reports as missing and retrying until it succeeds or returns a different
    /// error.
    ///
    /// The closure receives `&ChunkStore` rather than `&self`, which structurally prevents
    /// the read guard from escaping a single iteration — [`Self::load_chunks`] needs the
    /// write lock, and holding a read guard across that call would deadlock.
    ///
    /// A generous fixed attempt cap guards against a bug downstream (e.g. `load_chunks`
    /// silently no-ops while `MissingData` keeps being reported): exceeding it surfaces
    /// as an `Internal` error instead of spinning forever. In practice this loop converges
    /// in a handful of iterations; the cap is a paranoia valve, not a tight bound.
    fn with_autoload<T, F>(&self, mut op: F) -> Result<T, ExtractPropertiesError>
    where
        F: FnMut(&ChunkStore) -> Result<T, ExtractPropertiesError>,
    {
        const MAX_AUTOLOAD_ATTEMPTS: usize = 1024;
        for _ in 0..MAX_AUTOLOAD_ATTEMPTS {
            // IMPORTANT: bind to a local first so the read-guard temporary from
            // `self.store.read()` is dropped at this statement's semicolon. Matching on
            // `op(&self.store.read())` directly would extend the scrutinee's temporaries
            // through the arms and `self.load_chunks` (write lock) would deadlock.
            let result = op(&self.store.read());
            match result {
                Err(ExtractPropertiesError::MissingData(missing_ids)) => {
                    self.load_chunks(&missing_ids)
                        .map_err(|err| ExtractPropertiesError::Internal(err.to_string()))?;
                }
                other => return other,
            }
        }
        Err(ExtractPropertiesError::Internal(format!(
            "autoload did not converge after {MAX_AUTOLOAD_ATTEMPTS} attempts"
        )))
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
    use super::*;

    use re_chunk::{RowId, TimePoint, Timeline};
    use re_log_encoding::EncodingOptions;
    use re_log_types::{
        EntityPath, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource,
        example_components::{MyPoint, MyPoints},
    };

    /// Helper: create test chunks and encode to RRD file.
    /// Returns `(path, open file handle, store_id, chunks)`.
    fn create_test_rrd(
        dir: &Path,
        num_entities: usize,
        num_frames: usize,
    ) -> (PathBuf, File, StoreId, Vec<Arc<Chunk>>) {
        let path = dir.join("test.rrd");
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
        let mut file = std::fs::File::create(&path).unwrap();
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
        let file = File::open(&path).unwrap();
        (path, file, store_id, chunks)
    }

    fn read_raw_manifest(file: &mut File, store_id: &StoreId) -> Arc<RawRrdManifest> {
        let footer = re_log_encoding::read_rrd_footer(file).unwrap().unwrap();
        Arc::new(footer.manifests[store_id].clone())
    }

    #[test]
    fn test_lazy_store_no_physical_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let (path, mut file, store_id, chunks) = create_test_rrd(dir.path(), 2, 3);
        let raw = read_raw_manifest(&mut file, &store_id);

        let lazy = LazyRrdStore::try_new(file, path, raw).unwrap();

        assert_eq!(lazy.num_physical_chunks(), 0);
        assert_eq!(
            lazy.manifest().col_chunk_ids().len(),
            chunks.len(),
            "All chunk IDs should be in manifest"
        );
    }

    #[test]
    fn test_lazy_store_entities_visible() {
        let dir = tempfile::tempdir().unwrap();
        let (path, mut file, store_id, _) = create_test_rrd(dir.path(), 3, 2);
        let raw = read_raw_manifest(&mut file, &store_id);

        let lazy = LazyRrdStore::try_new(file, path, raw).unwrap();
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
        let (path, mut file, store_id, chunks) = create_test_rrd(dir.path(), 2, 3);
        let raw = read_raw_manifest(&mut file, &store_id);

        let lazy = LazyRrdStore::try_new(file, path, raw).unwrap();
        let loaded = lazy.collect_physical_chunks().unwrap();
        assert_eq!(loaded.len(), chunks.len());
    }

    #[test]
    fn test_lazy_store_load_single_chunk() {
        let dir = tempfile::tempdir().unwrap();
        let (path, mut file, store_id, chunks) = create_test_rrd(dir.path(), 2, 3);
        let raw = read_raw_manifest(&mut file, &store_id);

        let lazy = LazyRrdStore::try_new(file, path, raw).unwrap();
        let first_chunk_id = lazy.manifest().col_chunk_ids()[0];
        let loaded = lazy.load_chunks(&[first_chunk_id]).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(lazy.num_physical_chunks(), 1);
        assert!(lazy.has_physical_chunk(&first_chunk_id));

        // Other chunks are still virtual.
        let total_chunks = chunks.len();
        assert!(total_chunks > 1);
    }

    #[test]
    fn test_lazy_store_load_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let (path, mut file, store_id, _) = create_test_rrd(dir.path(), 1, 3);
        let raw = read_raw_manifest(&mut file, &store_id);

        let lazy = LazyRrdStore::try_new(file, path, raw).unwrap();
        lazy.load_all_chunks().unwrap();

        let count_before = lazy.num_physical_chunks();

        // Loading again should be a no-op.
        let loaded = lazy.load_chunks(lazy.manifest().col_chunk_ids()).unwrap();
        assert!(loaded.is_empty(), "Already-loaded chunks should be skipped");

        let count_after = lazy.num_physical_chunks();
        assert_eq!(count_before, count_after);
    }

    #[test]
    fn test_lazy_store_schema() {
        let dir = tempfile::tempdir().unwrap();
        let (path, mut file, store_id, _) = create_test_rrd(dir.path(), 2, 3);
        let raw = read_raw_manifest(&mut file, &store_id);

        let lazy = LazyRrdStore::try_new(file, path, raw).unwrap();
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
        let (path, mut file, store_id, _) = create_test_rrd(dir.path(), 2, 3);
        let raw = read_raw_manifest(&mut file, &store_id);

        // Lazy path: create lazy store, load all chunks.
        let lazy = LazyRrdStore::try_new(file, path.clone(), raw).unwrap();
        lazy.load_all_chunks().unwrap();

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
        assert_eq!(
            lazy.num_physical_chunks(),
            eager_store.num_physical_chunks(),
            "Same number of physical chunks"
        );
    }
}
