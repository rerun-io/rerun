use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::Arc;

use pyo3::prelude::*;

use re_chunk::{Chunk, ChunkId};
use re_chunk_store::LazyStore;

use super::error::ChunkPipelineError;
use super::py_stream::PyLazyChunkStreamInternal;
use super::stream::LazyChunkStream;
use super::summary::{SummaryRow, format_summary};
use super::{ChunkStream, ChunkStreamFactory};
use crate::catalog::PySchemaInternal;

/// An index-based, lazily-loaded chunk store.
///
/// Constructed from a [`LazyStore`]; the manifest is held in memory but chunks are loaded on
/// demand. Implements [`ChunkStreamFactory`] so `stream()` produces an [`IndexedChunkStream`]
/// that pulls chunks in byte-budgeted batches.
#[pyclass(
    frozen,
    name = "LazyStoreInternal",
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone)]
pub struct PyLazyStoreInternal {
    inner: Arc<LazyStore>,
}

impl PyLazyStoreInternal {
    pub fn new(lazy: LazyStore) -> Self {
        Self {
            inner: Arc::new(lazy),
        }
    }
}

#[pymethods]
impl PyLazyStoreInternal {
    /// The schema describing all columns in this store.
    fn schema(&self) -> PySchemaInternal {
        PySchemaInternal {
            columns: self.inner.schema().chunk_column_descriptors().into(),
            metadata: Default::default(),
        }
    }

    /// The total number of chunks described by the manifest (virtual and physical).
    fn num_chunks(&self) -> usize {
        self.inner.manifest().num_chunks()
    }

    /// Compact, deterministic summary of every chunk in the store for snapshot testing.
    ///
    /// Each line describes one chunk:
    /// `{entity_path} rows={n} static={bool} timelines=[…] cols=[…]`
    ///
    /// Built from the manifest only — no chunk data is loaded.
    fn summary(&self) -> String {
        let manifest = self.inner.manifest();
        let chunk_ids = manifest.col_chunk_ids();
        let entity_paths = manifest.col_chunk_entity_path_raw();
        let is_static_iter: Vec<bool> = manifest.col_chunk_is_static().collect();
        let num_rows = manifest.col_chunk_num_rows();

        // Per-chunk (timelines, cols), using BTreeSet for sorted-by-construction order.
        let mut per_chunk: HashMap<ChunkId, (BTreeSet<&'static str>, BTreeSet<&'static str>)> =
            HashMap::new();

        for per_entity in manifest.temporal_map().values() {
            for (timeline, per_component) in per_entity {
                let timeline_name = timeline.name().as_str();
                for (component, per_chunk_map) in per_component {
                    let component_name = component.as_str();
                    for chunk_id in per_chunk_map.keys() {
                        let entry = per_chunk.entry(*chunk_id).or_default();
                        entry.0.insert(timeline_name);
                        entry.1.insert(timeline_name);
                        entry.1.insert(component_name);
                    }
                }
            }
        }
        for per_entity in manifest.static_map().values() {
            for (component, chunk_id) in per_entity {
                let entry = per_chunk.entry(*chunk_id).or_default();
                entry.1.insert(component.as_str());
            }
        }

        let rows = chunk_ids.iter().enumerate().map(|(i, id)| {
            let (timelines, cols) = per_chunk.remove(id).unwrap_or_default();
            SummaryRow {
                entity_path: entity_paths.value(i).to_owned(),
                num_rows: num_rows[i],
                is_static: is_static_iter[i],
                timelines: timelines.into_iter().map(str::to_owned).collect(),
                cols: cols.into_iter().map(str::to_owned).collect(),
            }
        });
        format_summary(rows)
    }

    /// Return a lazy stream over all chunks in this store.
    fn stream(&self) -> PyLazyChunkStreamInternal {
        PyLazyChunkStreamInternal::new(LazyChunkStream::from_factory(self.clone()))
    }
}

impl ChunkStreamFactory for PyLazyStoreInternal {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        Ok(Box::new(IndexedChunkStream::new(Arc::clone(&self.inner))))
    }
}

// --- Streaming ---

/// Streaming loader for an indexed (lazy) [`ChunkStore`].
///
/// Pulls chunks from the underlying [`ChunkProvider`][re_log_encoding::ChunkProvider] in
/// byte-budgeted batches so resident memory stays bounded regardless of total recording size.
//TODO(RR-4545): this is hardly an optimal strategy. We need the ChunkProvider to expose a streaming
// API so that specific optimizations can be applied (e.g. adjacency for RRD, parallelism for
// segments, etc.)
struct IndexedChunkStream {
    lazy: Arc<LazyStore>,
    chunk_ids: Vec<ChunkId>,
    next_id: usize,
    buffer: VecDeque<Arc<Chunk>>,
}

impl IndexedChunkStream {
    /// Target bytes per batch — bounds memory while still letting `read_chunks` coalesce.
    const BATCH_BYTE_BUDGET: u64 = 8 * 1024 * 1024;

    fn new(lazy: Arc<LazyStore>) -> Self {
        let chunk_ids = lazy.manifest().col_chunk_ids().to_vec();
        Self {
            lazy,
            chunk_ids,
            next_id: 0,
            buffer: VecDeque::new(),
        }
    }

    /// End index (exclusive) of the next batch starting at `self.next_id`,
    /// chosen so the cumulative byte size stays under [`Self::BATCH_BYTE_BUDGET`].
    /// Always advances by at least one chunk to guarantee progress on huge chunks.
    fn next_batch_end(&self) -> usize {
        let sizes = self.lazy.manifest().col_chunk_byte_size();
        let mut end = self.next_id;
        let mut accumulated: u64 = 0;
        while end < self.chunk_ids.len() {
            let size = self
                .lazy
                .chunk_row_index(&self.chunk_ids[end])
                .map(|row| sizes[row])
                .unwrap_or(0);
            if end > self.next_id && accumulated.saturating_add(size) > Self::BATCH_BYTE_BUDGET {
                break;
            }
            accumulated = accumulated.saturating_add(size);
            end += 1;
        }
        end
    }
}

impl ChunkStream for IndexedChunkStream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        loop {
            if let Some(chunk) = self.buffer.pop_front() {
                return Ok(Some(chunk));
            }
            if self.next_id >= self.chunk_ids.len() {
                return Ok(None);
            }

            let end = self.next_batch_end();
            let ids = &self.chunk_ids[self.next_id..end];
            let chunks =
                self.lazy
                    .load_chunks(ids)
                    .map_err(|err| ChunkPipelineError::IndexedLoad {
                        from: self.lazy.source(),
                        reason: err.to_string(),
                    })?;
            self.next_id = end;
            self.buffer = chunks.into();
        }
    }
}
