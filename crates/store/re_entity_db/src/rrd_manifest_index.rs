use ahash::{HashMap, HashSet};
use arrow::array::{AsArray, Int32Array, Int64Array, RecordBatch};
use arrow::compute::take_record_batch;
use arrow::datatypes::Int64Type;
use itertools::{Itertools as _, izip};
use re_arrow_util::ArrowArrayDowncastRef;
use re_chunk::{ChunkId, Timeline, TimelineName};
use re_chunk_store::ChunkStoreEvent;
use re_log_encoding::{CodecResult, RrdManifest};
use re_log_types::{AbsoluteTimeRange, StoreKind};

/// Is the following chunk loaded?
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoadState {
    /// The chunk is not loaded, nor being loaded.
    #[default]
    Unloaded,

    /// We have requested it.
    InTransit,

    /// We have the chole chunk in memory.
    Loaded,
}

/// Info about a single chunk that we know ahead of loading it.
#[derive(Clone, Debug, Default)]
pub struct ChunkInfo {
    pub state: LoadState,
}

/// A secondary index that keeps track of which chunks have been loaded into memory.
///
/// This is currently used to show a progress bar.
///
/// This is constructed from an [`RrdManifest`], which is what
/// the server sends to the client/viewer.
/// TODO(RR-2999): use this for larger-than-RAM.
#[derive(Default, Debug, Clone)]
pub struct RrdManifestIndex {
    /// The raw manifest.
    ///
    /// This is known ahead-of-time for _some_ data sources.
    manifest: Option<RrdManifest>,

    /// These are the chunks known to exist in the data source (e.g. remote server).
    ///
    /// The chunk store may split large chunks and merge (compact) small ones,
    /// so what's in the chunk store can differ significantally.
    remote_chunks: HashMap<ChunkId, ChunkInfo>,

    /// The chunk store may split large chunks and merge (compact) small ones.
    /// When we later drop a chunk, we need to know which other chunks to invalidate.
    parents: HashMap<ChunkId, HashSet<ChunkId>>,

    /// Have we ever deleted a chunk?
    ///
    /// If so, we have run some GC and should not show progress bar.
    has_deleted: bool,
}

impl RrdManifestIndex {
    pub fn append(&mut self, manifest: RrdManifest) -> CodecResult<()> {
        re_tracing::profile_function!();

        for chunk_id in manifest.col_chunk_id()? {
            self.remote_chunks.entry(chunk_id).or_default();
            // TODO(RR-2999): update chunk info?
        }
        self.manifest = Some(manifest);
        Ok(())
    }

    /// The full manifest, if known.
    pub fn manifest(&self) -> Option<&RrdManifest> {
        self.manifest.as_ref()
    }

    /// [0, 1], how many chunks have been loaded?
    ///
    /// Returns `None` if we have already started garbage-collecting some chunks.
    pub fn progress(&self) -> Option<f32> {
        #[expect(clippy::question_mark)]
        if self.manifest.is_none() {
            return None;
        }

        let num_remote_chunks = self.remote_chunks.len();

        if self.has_deleted {
            None
        } else if num_remote_chunks == 0 {
            Some(1.0)
        } else {
            let num_loaded = self
                .remote_chunks
                .values()
                .filter(|c| c.state == LoadState::Loaded)
                .count();
            Some(num_loaded as f32 / num_remote_chunks as f32)
        }
    }

    pub fn mark_as_loaded(&mut self, chunk_id: ChunkId) {
        let chunk_info = self.remote_chunks.entry(chunk_id).or_default();
        chunk_info.state = LoadState::Loaded;
    }

    pub fn on_events(&mut self, store_events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        if self.manifest.is_none() {
            return;
        }

        for event in store_events {
            let store_kind = event.store_id.kind();
            let chunk_id = event.chunk.id();
            match event.kind {
                re_chunk_store::ChunkStoreDiffKind::Addition => {
                    if let Some(chunk_info) = self.remote_chunks.get_mut(&chunk_id) {
                        chunk_info.state = LoadState::Loaded;
                    } else if let Some(source) = event.split_source {
                        // The added chunk was the result of splitting another chunk:
                        self.parents.entry(chunk_id).or_default().insert(source);
                    } else {
                        warn_when_editing_recording(
                            store_kind,
                            "Added chunk that was not part of the chunk index",
                        );
                    }
                }
                re_chunk_store::ChunkStoreDiffKind::Deletion => {
                    self.mark_deleted(store_kind, &chunk_id);
                }
            }
        }
    }

    fn mark_deleted(&mut self, store_kind: StoreKind, chunk_id: &ChunkId) {
        self.has_deleted = true;

        if let Some(chunk_info) = self.remote_chunks.get_mut(chunk_id) {
            chunk_info.state = LoadState::Unloaded;
        } else if let Some(parents) = self.parents.remove(chunk_id) {
            // Mark all ancestors as not being fully loaded:

            let mut ancestors = parents.into_iter().collect_vec();
            while let Some(chunk_id) = ancestors.pop() {
                if let Some(chunk_info) = self.remote_chunks.get_mut(&chunk_id) {
                    chunk_info.state = LoadState::Unloaded;
                } else if let Some(grandparents) = self.parents.get(&chunk_id) {
                    ancestors.extend(grandparents);
                } else {
                    warn_when_editing_recording(
                        store_kind,
                        "Removed ancestor chunk that was not part of the index",
                    );
                }
            }
        } else {
            warn_when_editing_recording(store_kind, "Removed chunk that was not part of the index");
        }
    }

    /// Returns the yet-to-be-loaded chunks
    #[must_use]
    pub fn time_range_missing_chunks(
        &mut self,
        timeline: Timeline,
        query_range: AbsoluteTimeRange,
    ) -> Option<RecordBatch> {
        // Find the indices of all chunks that overlaps the query, then select those rows of the record batch.

        let manifest = self.manifest.as_mut()?;

        let mut indices = vec![];

        let chunk_id = manifest.col_chunk_id().unwrap();
        let start_column = manifest
            .data
            .column_by_name(RrdManifest::field_index_start(&timeline, None).name())?
            .as_primitive_opt::<Int64Type>()?;
        let end_column = manifest
            .data
            .column_by_name(RrdManifest::field_index_end(&timeline, None).name())?
            .as_primitive_opt::<Int64Type>()?;

        for (row_idx, (chunk_id, start_time, end_time)) in
            izip!(chunk_id, start_column, end_column).enumerate()
        {
            let chunk_range = AbsoluteTimeRange::new(
                start_time.unwrap_or_default(),
                end_time.unwrap_or_default(),
            );
            if chunk_range.intersects(query_range) {
                let chunk_info = self.remote_chunks.entry(chunk_id).or_default();
                if chunk_info.state == LoadState::Unloaded {
                    chunk_info.state = LoadState::InTransit;
                    indices.push(row_idx as i32);
                }
            }
        }

        take_record_batch(&manifest.data, &Int32Array::from(indices)).ok()
    }
}

#[track_caller]
fn warn_when_editing_recording(store_kind: StoreKind, warning: &str) {
    match store_kind {
        StoreKind::Recording => {
            if cfg!(debug_assertions) {
                re_log::warn_once!("[DEBUG] {warning}");
            } else {
                re_log::debug_once!("{warning}");
            }
        }
        StoreKind::Blueprint => {
            // We edit blueprint by generating new chunks in the viewer.
        }
    }
}
