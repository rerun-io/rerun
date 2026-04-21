use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyStopIteration;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use arrow::array::RecordBatch as ArrowRecordBatch;
use arrow::pyarrow::{PyArrowType, ToPyArrow as _};
use re_chunk::Chunk;
use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle};
use re_log_types::{EntityPath, StoreId, StoreInfo, StoreSource};

use crate::recording::PyRecordingInternal;

/// A single chunk of data from a recording.
#[pyclass(
    frozen,
    name = "ChunkInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyChunkInternal {
    chunk: Arc<Chunk>,
}

impl PyChunkInternal {
    pub fn new(chunk: Arc<Chunk>) -> Self {
        Self { chunk }
    }

    pub fn inner(&self) -> &Arc<Chunk> {
        &self.chunk
    }
}

#[pymethods]
impl PyChunkInternal {
    /// The unique ID of this chunk.
    #[getter]
    fn id(&self) -> String {
        self.chunk.id().to_string()
    }

    /// The entity path this chunk belongs to.
    #[getter]
    fn entity_path(&self) -> String {
        self.chunk.entity_path().to_string()
    }

    /// The number of rows in this chunk.
    #[getter]
    fn num_rows(&self) -> usize {
        self.chunk.num_rows()
    }

    /// The number of columns in this chunk.
    #[getter]
    fn num_columns(&self) -> usize {
        self.chunk.num_columns()
    }

    /// Whether the chunk contains only static data (no timelines).
    #[getter]
    fn is_static(&self) -> bool {
        self.chunk.is_static()
    }

    /// Whether the chunk has zero rows.
    #[getter]
    fn is_empty(&self) -> bool {
        self.chunk.is_empty()
    }

    /// The names of all timelines in this chunk.
    #[getter]
    fn timeline_names(&self) -> Vec<String> {
        self.chunk
            .timelines()
            .keys()
            .map(|name| name.to_string())
            .collect()
    }

    /// Convert this chunk to an Arrow RecordBatch.
    fn to_record_batch(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let batch = self
            .chunk
            .to_record_batch()
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
        Ok(batch.to_pyarrow(py)?.unbind())
    }

    /// Create a Chunk from a PyArrow RecordBatch with Rerun schema metadata.
    ///
    /// The RecordBatch must have been produced by `to_record_batch()` or have
    /// equivalent Rerun metadata in its schema.
    #[staticmethod]
    #[expect(clippy::needless_pass_by_value)] // PyO3 requires owned PyArrowType for #[staticmethod]
    fn from_record_batch(record_batch: PyArrowType<ArrowRecordBatch>) -> PyResult<Self> {
        let chunk = Chunk::from_record_batch(&record_batch.0)
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
        Ok(Self::new(Arc::new(chunk)))
    }

    /// Create a Chunk from an entity path, timeline arrays, and component arrays.
    ///
    /// This is the low-level entry point called by `Chunk.from_columns()` in Python.
    #[staticmethod]
    fn from_columns(
        entity_path: &str,
        timelines: &Bound<'_, PyDict>,
        components: &Bound<'_, PyDict>,
    ) -> PyResult<Self> {
        let entity_path = EntityPath::parse_forgiving(entity_path);
        let chunk = crate::arrow::build_chunk_from_components(entity_path, timelines, components)?;
        Ok(Self::new(Arc::new(chunk)))
    }

    /// Apply one or more lenses to this chunk, returning transformed chunks.
    #[expect(clippy::needless_pass_by_value)] // PyO3 requires owned Vec
    #[pyo3(signature = (lenses))]
    fn apply_lenses(
        &self,
        lenses: Vec<PyRef<'_, crate::lenses::PyLensInternal>>,
    ) -> PyResult<Vec<Self>> {
        let mut collection =
            re_lenses_core::Lenses::new(re_lenses_core::OutputMode::ForwardUnmatched);
        for lens in &lenses {
            collection = collection.add_lens(lens.inner().clone());
        }

        let mut results = Vec::new();
        for result in collection.apply(&self.chunk) {
            match result {
                Ok(chunk) => results.push(Self {
                    chunk: Arc::new(chunk),
                }),
                Err(partial) => {
                    let reason = partial
                        .errors()
                        .map(|e| e.to_string())
                        .collect::<Vec<_>>()
                        .join("; ");
                    return Err(PyValueError::new_err(reason));
                }
            }
        }
        Ok(results)
    }

    /// Format this chunk as a human-readable table string.
    ///
    /// Args:
    ///     width: Fixed width for the table (default: 240).
    ///     redact: If true, redact non-deterministic values like RowIds (default: false).
    #[pyo3(signature = (*, width=240, redact=false))]
    fn format(&self, width: usize, redact: bool) -> PyResult<String> {
        let batch = self
            .chunk
            .to_record_batch()
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
        Ok(re_arrow_util::format_record_batch_with_width(&batch, Some(width), redact).to_string())
    }

    fn __repr__(&self) -> String {
        format!(
            "Chunk(id={}, entity_path={}, num_rows={}, num_columns={})",
            self.chunk.id(),
            self.chunk.entity_path(),
            self.chunk.num_rows(),
            self.chunk.num_columns(),
        )
    }

    fn __len__(&self) -> usize {
        self.chunk.num_rows()
    }
}

/// An iterator over chunks in a recording.
// TODO(RR-4126): currently, the stores we can iterate from are fully loaded in memory, so the
// `Vec<Arc<_>>` is an acceptable shortcut. In the future, this iterator should be streaming and
// only load chunks (from file/remote segment) to pipeline over larger-than-ram data.
#[pyclass(name = "ChunkIterator", module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq]
pub struct PyChunkIterator {
    chunks: Vec<Arc<Chunk>>,
    index: AtomicUsize,
}

impl PyChunkIterator {
    pub fn new(chunks: Vec<Arc<Chunk>>) -> Self {
        Self {
            chunks,
            index: AtomicUsize::new(0),
        }
    }
}

#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PyChunkIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self) -> PyResult<PyChunkInternal> {
        let idx = self.index.fetch_add(1, Ordering::Relaxed);
        if idx < self.chunks.len() {
            Ok(PyChunkInternal::new(self.chunks[idx].clone()))
        } else {
            Err(PyStopIteration::new_err(""))
        }
    }
}

/// Create a new recording from an iterable of chunks.
#[pyfunction]
#[expect(clippy::needless_pass_by_value)]
pub fn recording_from_chunks(
    py: Python<'_>,
    chunks: &Bound<'_, PyAny>,
    application_id: String,
    recording_id: String,
) -> PyResult<PyRecordingInternal> {
    let store_id = StoreId::recording(application_id.as_str(), recording_id.as_str());

    let mut store = ChunkStore::new(store_id.clone(), ChunkStoreConfig::DEFAULT);

    let iter = chunks.try_iter()?;
    for item in iter {
        let item: Bound<'_, PyAny> = item?;
        let chunk_internal: PyRef<'_, PyChunkInternal> = item.extract()?;
        store
            .insert_chunk(chunk_internal.inner())
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    }

    let info = StoreInfo::new(store_id, StoreSource::Other("rerun-sdk-python".into()));

    let _ = py;

    Ok(PyRecordingInternal {
        store: ChunkStoreHandle::new(store),
        store_info: Some(info),
    })
}
