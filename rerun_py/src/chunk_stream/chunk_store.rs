use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use re_chunk::Chunk;
use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, ChunkStoreHandle, QueryExpression, SparseFillStrategy,
    StaticColumnSelection, ViewContentsSelector,
};
use re_datafusion::LocalChunkStoreTableProvider;
use re_log_types::{EntityPathFilter, StoreId, StoreKind};

use super::error::ChunkPipelineError;
use super::py_stream::PyLazyChunkStreamInternal;
use super::stream::LazyChunkStream;
use super::summary::{SummaryRow, format_summary};
use super::{ChunkStream, ChunkStreamFactory};
use crate::catalog::{
    IndexValuesLike, PySchemaInternal, PyTableProviderAdapterInternal, to_py_err,
};
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

            let mut cols: Vec<String> = std::iter::chain(
                chunk.timelines().keys().map(|t| t.as_str().to_owned()),
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

    /// Build a `TableProvider` for an in-process DataFusion query over this store.
    ///
    /// All keyword arguments are required at this internal layer; defaults
    /// live in the public `ChunkStore.reader()` wrapper.
    #[expect(clippy::fn_params_excessive_bools)]
    #[expect(clippy::needless_pass_by_value)] // PyO3 extraction yields owned values
    #[pyo3(signature = (
        *,
        index,
        contents,
        include_semantically_empty_columns,
        include_tombstone_columns,
        fill_latest_at,
        using_index_values,
    ))]
    fn reader(
        &self,
        index: Option<String>,
        contents: Option<Vec<String>>,
        include_semantically_empty_columns: bool,
        include_tombstone_columns: bool,
        fill_latest_at: bool,
        using_index_values: Option<IndexValuesLike<'_>>,
    ) -> PyResult<PyTableProviderAdapterInternal> {
        // `LocalChunkStoreTableProvider::try_new` validates `index` against the
        // store schema and returns an error if it doesn't exist.
        let view_contents =
            build_view_contents_from_filters(&self.handle.read(), contents.as_deref());
        let using_index_values = using_index_values
            .map(|v| v.to_index_values())
            .transpose()?;

        let static_only = index.is_none();
        let query = QueryExpression {
            view_contents: Some(view_contents),
            include_semantically_empty_columns,
            include_tombstone_columns,
            include_static_columns: if static_only {
                StaticColumnSelection::StaticOnly
            } else {
                StaticColumnSelection::Both
            },
            filtered_index: index.map(Into::into),
            filtered_index_range: None,
            filtered_index_values: None,
            using_index_values,
            filtered_is_not_null: None,
            sparse_fill_strategy: if fill_latest_at {
                SparseFillStrategy::LatestAtGlobal
            } else {
                SparseFillStrategy::None
            },
            selection: None,
        };

        let provider =
            LocalChunkStoreTableProvider::try_new(self.handle.clone(), query).map_err(to_py_err)?;
        Ok(PyTableProviderAdapterInternal::new(
            Arc::new(provider),
            /* streaming= */ true,
        ))
    }
}

/// Build a `ViewContentsSelector` from `contents` expressions, applied
/// against the store's currently known entities.
///
/// Semantics mirror [`PyDatasetViewInternal::filter_contents`]:
/// * `None` → everything (`/**`)
/// * `Some([])` → nothing (`-/**`)
/// * `Some(exprs)` → join `exprs` with spaces and parse as a filter.
fn build_view_contents_from_filters(
    store: &ChunkStore,
    contents: Option<&[String]>,
) -> ViewContentsSelector {
    let filter = match contents {
        None => EntityPathFilter::parse_forgiving("/**").resolve_without_substitutions(),
        Some([]) => EntityPathFilter::parse_forgiving("-/**").resolve_without_substitutions(),
        Some(exprs) => {
            EntityPathFilter::parse_forgiving(exprs.join(" ")).resolve_without_substitutions()
        }
    };
    store
        .all_entities()
        .into_iter()
        .filter(|ep| filter.matches(ep))
        .map(|ep| (ep, None))
        .collect()
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
