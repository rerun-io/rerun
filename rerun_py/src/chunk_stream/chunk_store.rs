use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use re_chunk::Chunk;
use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle};
use re_log_types::{StoreId, StoreKind};

use super::error::ChunkPipelineError;
use super::py_stream::PyLazyChunkStreamInternal;
use super::stream::LazyChunkStream;
use super::{ChunkStream, ChunkStreamFactory};
use crate::catalog::PySchemaInternal;
use crate::chunk::PyChunkInternal;

/// Wraps a [`re_chunk_store::ChunkStore`].
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
    pub(crate) store: ChunkStoreHandle,
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
        Ok(Self {
            store: ChunkStoreHandle::new(store),
        })
    }

    /// The schema describing all columns in this store.
    fn schema(&self) -> PySchemaInternal {
        PySchemaInternal {
            columns: self.store.read().schema().chunk_column_descriptors().into(),
            metadata: Default::default(),
        }
    }

    /// Return a new store with chunks compacted for optimal storage.
    fn compact(&self, py: Python<'_>) -> PyResult<Self> {
        let store_handle = self.store.clone();
        py.detach(move || {
            store_handle
                .read()
                .compacted(&ChunkStoreConfig::CHANGELOG_DISABLED, None)
                .map(|store| Self {
                    store: ChunkStoreHandle::new(store),
                })
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))
        })
    }

    /// Return a lazy stream over all chunks in this store.
    fn stream(&self) -> PyLazyChunkStreamInternal {
        // Each compile() snapshots the store's current physical chunks.
        PyLazyChunkStreamInternal::new(LazyChunkStream::from_factory(self.clone()))
    }
}

impl ChunkStreamFactory for PyChunkStoreInternal {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        let guard = self.store.read();
        // TODO(RR-4321): collecting all chunks in a vec here is only acceptable so long as
        //   `ChunkStore` is fully materialized. This will have to be made lazy and index-based in
        //   the future.
        let chunks: Vec<Arc<Chunk>> = guard.iter_physical_chunks().cloned().collect();
        Ok(Box::new(VecChunkStream { chunks, pos: 0 }))
    }
}

/// Pull-based stream over a pre-collected `Vec` of chunks.
struct VecChunkStream {
    chunks: Vec<Arc<Chunk>>,
    pos: usize,
}

// Vec<Arc<Chunk>> + usize are Send.
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
