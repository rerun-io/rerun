use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use arrow::array::RecordBatch as ArrowRecordBatch;
use arrow::pyarrow::{PyArrowType, ToPyArrow as _};
use re_chunk::Chunk;
use re_log_types::EntityPath;

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

    /// Interpret a PyArrow RecordBatch as Rerun chunk data, one chunk per entity path.
    ///
    /// `index_mode` is one of `"auto"`, `"static"`, or `"columns"`; when it is `"columns"`,
    /// `index_columns` names the columns to promote to timelines. `entity_path` is the default
    /// entity path for un-located component columns.
    ///
    /// All conversion errors (both `SorbetError` and `ChunkError`) are mapped to `ValueError`, so
    /// the documented `Raises` contract of `Chunk.from_record_batch` holds.
    #[staticmethod]
    #[pyo3(signature = (record_batch, index_mode, index_columns, entity_path))]
    #[expect(clippy::needless_pass_by_value)] // PyO3 requires owned arguments for #[staticmethod]
    fn from_record_batch(
        record_batch: PyArrowType<ArrowRecordBatch>,
        index_mode: &str,
        index_columns: Vec<String>,
        entity_path: Option<String>,
    ) -> PyResult<Vec<Self>> {
        use re_log_types::TimelineName;
        use re_sorbet::DataframeIndex;

        let index = match index_mode {
            "auto" => DataframeIndex::Auto,
            "static" => DataframeIndex::Static,
            "columns" => DataframeIndex::Columns(
                index_columns
                    .iter()
                    .map(|s| {
                        TimelineName::try_new(s.as_str())
                            .map_err(|err| PyValueError::new_err(err.to_string()))
                    })
                    .collect::<PyResult<Vec<_>>>()?,
            ),
            _ => {
                return Err(PyValueError::new_err(format!(
                    "Invalid index mode {index_mode:?}; expected \"auto\", \"static\", or \"columns\"."
                )));
            }
        };
        let entity_path = entity_path.map(|p| EntityPath::parse_forgiving(&p));

        let chunks =
            Chunk::from_dataframe_record_batch(&record_batch.0, &index, entity_path.as_ref())
                .map_err(|err| PyValueError::new_err(err.to_string()))?;

        Ok(chunks
            .into_iter()
            .map(|chunk| Self::new(Arc::new(chunk)))
            .collect())
    }

    /// Return a copy of this chunk with a new entity path.
    ///
    /// A fresh chunk ID is generated to avoid aliasing the original chunk in downstream
    /// caches and indices. Row IDs, timelines, and components are preserved as-is.
    fn with_entity_path(&self, entity_path: &str) -> Self {
        let entity_path = EntityPath::parse_forgiving(entity_path);
        let chunk = self.chunk.clone_with_new_entity_path(entity_path);
        Self::new(Arc::new(chunk))
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
        py: Python<'_>,
        lenses: Vec<crate::lenses::PyLens>,
    ) -> PyResult<Vec<Self>> {
        use re_lenses_core::ChunkExt as _;

        let lenses: Vec<_> = lenses
            .iter()
            .map(|l| l.build(py))
            .collect::<PyResult<Vec<_>>>()?;
        match self
            .chunk
            .apply_lenses(&lenses, &re_lenses::default_runtime())
        {
            Ok(chunks) => Ok(chunks
                .into_iter()
                .map(|chunk| Self {
                    chunk: Arc::new(chunk),
                })
                .collect()),
            Err(partial) => {
                let reason = partial
                    .errors()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; ");
                Err(PyValueError::new_err(reason))
            }
        }
    }

    /// Apply a selector to a single component, returning a new chunk with the component transformed.
    #[pyo3(signature = (source, selector))]
    fn apply_selector(
        &self,
        source: &str,
        selector: &crate::selector::PySelectorInternal,
    ) -> PyResult<Self> {
        use re_lenses_core::ChunkExt as _;
        use re_types_core::ComponentIdentifier;

        let source_id = ComponentIdentifier::from(source);

        let new_chunk = self
            .chunk
            .apply_selector(
                source_id,
                selector.selector(),
                &re_lenses::default_runtime(),
            )
            .map_err(|err| PyValueError::new_err(err.to_string()))?;

        Ok(Self::new(Arc::new(new_chunk)))
    }

    /// Format this chunk as a human-readable table string. Internal: the user-facing wrapper sets defaults.
    #[pyo3(signature = (*, width, redact, trim_metadata_keys))]
    #[expect(clippy::fn_params_excessive_bools)] // Named keyword args in Python.
    fn format(&self, width: usize, redact: bool, trim_metadata_keys: bool) -> PyResult<String> {
        let batch = self
            .chunk
            .to_record_batch()
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
        let opts = re_arrow_util::RecordBatchFormatOpts {
            width: Some(width),
            redact_non_deterministic: redact,
            trim_metadata_keys,
            ..Default::default()
        };
        Ok(re_arrow_util::format_record_batch_opts(&batch, &opts).to_string())
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
