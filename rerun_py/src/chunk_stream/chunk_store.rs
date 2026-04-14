use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use re_chunk::Chunk;
use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle, LazyRrdStore};
use re_log_types::{StoreId, StoreKind};
use re_sorbet::ChunkColumnDescriptors;

use super::error::ChunkPipelineError;
use super::py_stream::PyLazyChunkStreamInternal;
use super::stream::LazyChunkStream;
use super::{ChunkStream, ChunkStreamFactory};
use crate::catalog::PySchemaInternal;
use crate::chunk::PyChunkInternal;

/// A chunk store, either fully materialized or lazily backed by an RRD file.
///
/// This is a newtype around [`ChunkStoreInternal`] because PyO3 cannot derive
/// `#[pyclass]` on enums whose variants hold non-PyO3 types.
///
/// Implements [`ChunkStreamFactory`] so `stream()` can hand `self.clone()`
/// straight to [`LazyChunkStream::from_factory`] -- no intermediate wrapper.
#[pyclass(
    frozen,
    name = "ChunkStoreInternal",
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone)]
pub struct PyChunkStoreInternal(ChunkStoreInternal);

/// Fully materialized or lazily-backed chunk store.
//TODO(RR-4341): this is a temporary thing until we have a more general `ChunkProvider` abstraction.
#[derive(Clone)]
enum ChunkStoreInternal {
    /// All chunks are in memory.
    InMemory(ChunkStoreHandle),

    /// Index loaded from RRD footer, chunks loaded on demand.
    IndexedRrd(Arc<LazyRrdStore>),
}

impl ChunkStoreInternal {
    fn schema(&self) -> ChunkColumnDescriptors {
        match self {
            Self::InMemory(handle) => handle.read().schema().chunk_column_descriptors(),
            Self::IndexedRrd(lazy) => lazy.schema().chunk_column_descriptors(),
        }
    }
}

impl PyChunkStoreInternal {
    pub fn in_memory(store: ChunkStore) -> Self {
        Self(ChunkStoreInternal::InMemory(ChunkStoreHandle::new(store)))
    }

    pub fn indexed_rrd(lazy: LazyRrdStore) -> Self {
        Self(ChunkStoreInternal::IndexedRrd(Arc::new(lazy)))
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
        Ok(Self::in_memory(store))
    }

    /// The schema describing all columns in this store.
    fn schema(&self) -> PySchemaInternal {
        PySchemaInternal {
            columns: self.0.schema().into(),
            metadata: Default::default(),
        }
    }

    /// Return a new store with chunks compacted for optimal storage.
    fn compact(&self, py: Python<'_>) -> PyResult<Self> {
        let inner = self.0.clone();
        py.detach(move || {
            let compacted = match &inner {
                ChunkStoreInternal::InMemory(handle) => handle
                    .read()
                    .compacted(&ChunkStoreConfig::CHANGELOG_DISABLED, None),
                ChunkStoreInternal::IndexedRrd(lazy) => {
                    lazy.compacted(&ChunkStoreConfig::CHANGELOG_DISABLED, None)
                }
            };
            compacted
                .map(Self::in_memory)
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))
        })
    }

    /// The total number of chunks in this store (virtual and physical).
    fn num_chunks(&self) -> usize {
        match &self.0 {
            ChunkStoreInternal::InMemory(handle) => handle.read().num_physical_chunks(),
            ChunkStoreInternal::IndexedRrd(lazy) => lazy.manifest().num_chunks(),
        }
    }

    /// Compact, deterministic summary of every chunk in the store for snapshot testing.
    ///
    /// Each line describes one chunk:
    /// `{entity_path} rows={n} static={bool} timelines=[…] cols=[…]`
    ///
    /// Chunks are sorted by `(entity_path, !is_static)`. The `cols` list
    /// combines timeline and component column names (sorted), excluding
    /// `rerun.controls` columns.
    ///
    /// For lazily-loaded stores, this forces loading all chunk data from disk.
    //TODO(ab): should that be implemented on `re_chunk_store::ChunkStore` directly?
    fn summary(&self) -> PyResult<String> {
        let chunks = self.collect_all_chunks()?;
        Ok(summary_from_chunks(&chunks))
    }

    /// Return a lazy stream over all chunks in this store.
    fn stream(&self) -> PyLazyChunkStreamInternal {
        // Each compile() snapshots the store's current physical chunks.
        PyLazyChunkStreamInternal::new(LazyChunkStream::from_factory(self.clone()))
    }
}

impl PyChunkStoreInternal {
    /// Collect all chunks from either variant, loading lazily if needed.
    fn collect_all_chunks(&self) -> PyResult<Vec<Arc<Chunk>>> {
        match &self.0 {
            ChunkStoreInternal::InMemory(handle) => {
                Ok(handle.read().iter_physical_chunks().cloned().collect())
            }

            ChunkStoreInternal::IndexedRrd(lazy) => lazy
                .collect_physical_chunks()
                .map_err(|err| PyRuntimeError::new_err(err.to_string())),
        }
    }
}

impl ChunkStreamFactory for PyChunkStoreInternal {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        let chunks = match &self.0 {
            ChunkStoreInternal::InMemory(handle) => {
                handle.read().iter_physical_chunks().cloned().collect()
            }
            ChunkStoreInternal::IndexedRrd(lazy) => {
                lazy.collect_physical_chunks()
                    .map_err(|err| ChunkPipelineError::RrdRead {
                        path: lazy.rrd_path().to_path_buf(),
                        reason: err.to_string(),
                    })?
            }
        };
        Ok(Box::new(VecChunkStream { chunks, pos: 0 }))
    }
}

/// Build a summary from a list of chunks.
fn summary_from_chunks(chunks: &[Arc<Chunk>]) -> String {
    let mut chunks: Vec<&Chunk> = chunks.iter().map(|c| c.as_ref()).collect();
    chunks.sort_by(|a, b| {
        a.entity_path()
            .cmp(b.entity_path())
            .then_with(|| a.is_static().cmp(&b.is_static()).reverse())
    });

    let mut lines = Vec::new();
    for chunk in &chunks {
        let mut timelines: Vec<&str> = chunk.timelines().keys().map(|t| t.as_str()).collect();
        timelines.sort();

        let mut cols: Vec<&str> = chunk
            .timelines()
            .keys()
            .map(|t| t.as_str())
            .chain(
                chunk
                    .components()
                    .component_descriptors()
                    .map(|d| d.display_name()),
            )
            .collect();
        cols.sort();

        let timelines_str = timelines
            .iter()
            .map(|t| format!("'{t}'"))
            .collect::<Vec<_>>()
            .join(", ");
        let cols_str = cols
            .iter()
            .map(|c| format!("'{c}'"))
            .collect::<Vec<_>>()
            .join(", ");
        let is_static = if chunk.is_static() { "True" } else { "False" };

        lines.push(format!(
            "{entity_path} rows={rows} static={is_static} timelines=[{timelines_str}] cols=[{cols_str}]",
            entity_path = chunk.entity_path(),
            rows = chunk.num_rows(),
        ));
    }

    lines.join("\n")
}

// --- Streaming ---

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
