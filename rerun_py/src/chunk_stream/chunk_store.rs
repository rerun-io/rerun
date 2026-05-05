use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use re_chunk::Chunk;
use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle};
use re_log_types::{StoreId, StoreKind};

use super::error::ChunkPipelineError;
use super::py_stream::PyLazyChunkStreamInternal;
use super::stream::LazyChunkStream;
use super::summary::{SummaryRow, format_summary};
use super::{ChunkStream, ChunkStreamFactory};
use crate::catalog::PySchemaInternal;
use crate::chunk::PyChunkInternal;

/// A fully-materialized, in-memory chunk store.
///
/// Implements [`ChunkStreamFactory`] so `stream()` can hand `self.clone()`
/// straight to [`LazyChunkStream::from_factory`] -- no intermediate wrapper.
#[pyclass(
    frozen,
    name = "ChunkStoreInternal",
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone)]
pub struct PyChunkStoreInternal {
    handle: ChunkStoreHandle,
}

impl PyChunkStoreInternal {
    pub fn new(store: ChunkStore) -> Self {
        Self {
            handle: ChunkStoreHandle::new(store),
        }
    }
}

#[pymethods]
impl PyChunkStoreInternal {
    /// Build a ChunkStore from a list of chunks.
    #[staticmethod]
    #[expect(clippy::needless_pass_by_value)] // PyO3 requires owned Vec for #[staticmethod]
    fn from_chunks(chunks: Vec<PyRef<'_, PyChunkInternal>>) -> PyResult<Self> {
        let store_id = StoreId::random(StoreKind::Recording, "chunk-store");
        let mut store = ChunkStore::new(store_id, ChunkStoreConfig::ALL_DISABLED);
        for py_chunk in &chunks {
            store
                .insert_chunk(py_chunk.inner())
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
        }
        Ok(Self::new(store))
    }

    /// The schema describing all columns in this store.
    fn schema(&self) -> PySchemaInternal {
        PySchemaInternal {
            columns: self
                .handle
                .read()
                .schema()
                .chunk_column_descriptors()
                .into(),
            metadata: Default::default(),
        }
    }

    /// The total number of chunks in this store (virtual and physical).
    fn num_chunks(&self) -> usize {
        self.handle.read().num_physical_chunks()
    }

    /// Compact, deterministic summary of every chunk in the store for snapshot testing.
    ///
    /// Each line describes one chunk:
    /// `{entity_path} rows={n} static={bool} timelines=[…] cols=[…]`
    ///
    /// Chunks are sorted by `(entity_path, !is_static)`. The `cols` list
    /// combines timeline and component column names (sorted).
    fn summary(&self) -> String {
        let store = self.handle.read();
        let chunks: Vec<Arc<Chunk>> = store.iter_physical_chunks().cloned().collect();
        let rows = chunks.iter().map(|chunk| {
            let mut timelines: Vec<String> = chunk
                .timelines()
                .keys()
                .map(|t| t.as_str().to_owned())
                .collect();
            timelines.sort();

            let mut cols: Vec<String> = chunk
                .timelines()
                .keys()
                .map(|t| t.as_str().to_owned())
                .chain(
                    chunk
                        .components()
                        .component_descriptors()
                        .map(|d| d.display_name().to_owned()),
                )
                .collect();
            cols.sort();

            SummaryRow {
                entity_path: chunk.entity_path().to_string(),
                num_rows: chunk.num_rows() as u64,
                is_static: chunk.is_static(),
                timelines,
                cols,
            }
        });
        format_summary(rows)
    }

    /// Return a lazy stream over all chunks in this store.
    fn stream(&self) -> PyLazyChunkStreamInternal {
        // Each compile() snapshots the store's current physical chunks.
        PyLazyChunkStreamInternal::new(LazyChunkStream::from_factory(self.clone()))
    }
}

impl ChunkStreamFactory for PyChunkStoreInternal {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        let chunks = self.handle.read().iter_physical_chunks().cloned().collect();
        Ok(Box::new(VecChunkStream { chunks, pos: 0 }))
    }
}

// --- Streaming ---

/// Pull-based stream over a pre-collected `Vec` of chunks.
struct VecChunkStream {
    chunks: Vec<Arc<Chunk>>,
    pos: usize,
}

impl ChunkStream for VecChunkStream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        if self.pos < self.chunks.len() {
            let chunk = self.chunks[self.pos].clone();
            self.pos += 1;
            Ok(Some(chunk))
        } else {
            Ok(None)
        }
    }
}
