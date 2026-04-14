use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;

use re_chunk::{Chunk, ChunkId};
use re_log_encoding::RrdManifest;

use crate::{
    ChunkStore, ChunkStoreConfig, ChunkStoreHandle, ChunkStoreResult, EntityTree, StoreSchema,
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
    manifest: Arc<RrdManifest>,
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
    pub fn new(file: File, rrd_path: PathBuf, manifest: Arc<RrdManifest>) -> Self {
        // IMPORTANT: `ALL_DISABLED` here is load-bearing, since the `ChunkStore` is essentially
        // acting as a cache for the underlying RRD. Any compaction, etc. would lead to unexpected
        // consequences.
        let mut store =
            ChunkStore::new(manifest.store_id().clone(), ChunkStoreConfig::ALL_DISABLED);

        #[expect(clippy::let_underscore_must_use)]
        let _ = store.insert_rrd_manifest(Arc::clone(&manifest));

        Self {
            store: ChunkStoreHandle::new(store),
            file: Mutex::new(file),
            rrd_path,
            manifest,
        }
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
    pub fn compacted(
        &self,
        compaction_config: &ChunkStoreConfig,
        num_extra_passes: Option<usize>,
    ) -> ChunkStoreResult<ChunkStore> {
        self.load_all_chunks()?;
        self.store
            .read()
            .compacted(compaction_config, num_extra_passes)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    use re_chunk::{RowId, TimePoint, Timeline};
    use re_log_encoding::{EncodingOptions, RrdManifest};
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

    fn read_manifest(file: &mut File, store_id: &StoreId) -> Arc<RrdManifest> {
        let footer = re_log_encoding::read_rrd_footer(file).unwrap().unwrap();
        let raw = footer.manifests[store_id].clone();
        Arc::new(RrdManifest::try_new(&raw).unwrap())
    }

    #[test]
    fn test_lazy_store_no_physical_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let (path, mut file, store_id, chunks) = create_test_rrd(dir.path(), 2, 3);
        let manifest = read_manifest(&mut file, &store_id);

        let lazy = LazyRrdStore::new(file, path, manifest.clone());

        assert_eq!(lazy.num_physical_chunks(), 0);
        assert_eq!(
            manifest.col_chunk_ids().len(),
            chunks.len(),
            "All chunk IDs should be in manifest"
        );
    }

    #[test]
    fn test_lazy_store_entities_visible() {
        let dir = tempfile::tempdir().unwrap();
        let (path, mut file, store_id, _) = create_test_rrd(dir.path(), 3, 2);
        let manifest = read_manifest(&mut file, &store_id);

        let lazy = LazyRrdStore::new(file, path, manifest);
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
        let manifest = read_manifest(&mut file, &store_id);

        let lazy = LazyRrdStore::new(file, path, manifest);
        let loaded = lazy.collect_physical_chunks().unwrap();
        assert_eq!(loaded.len(), chunks.len());
    }

    #[test]
    fn test_lazy_store_load_single_chunk() {
        let dir = tempfile::tempdir().unwrap();
        let (path, mut file, store_id, chunks) = create_test_rrd(dir.path(), 2, 3);
        let manifest = read_manifest(&mut file, &store_id);

        let first_chunk_id = manifest.col_chunk_ids()[0];
        let lazy = LazyRrdStore::new(file, path, manifest);
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
        let manifest = read_manifest(&mut file, &store_id);

        let lazy = LazyRrdStore::new(file, path, manifest);
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
        let manifest = read_manifest(&mut file, &store_id);

        let lazy = LazyRrdStore::new(file, path, manifest);
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
        let manifest = read_manifest(&mut file, &store_id);

        // Lazy path: create lazy store, load all chunks.
        let lazy = LazyRrdStore::new(file, path.clone(), manifest);
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
