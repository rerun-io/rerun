use std::sync::Arc;

use arrow::array::{Float32Array, RecordBatch, RecordBatchOptions};
use arrow::datatypes::Field;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::{FromPyObject, PyErr, PyResult, pyclass, pymethods};
use re_protos::cloud::v1alpha1::ext::IndexProperties;
use re_sorbet::ComponentColumnSelector;

use crate::catalog::{PyComponentColumnSelector, PyIndexColumnSelector, to_py_err};

// ---

/// The result returned from an indexing operation.
#[pyclass(name = "IndexingResult", module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq] non-trivial implementation
pub struct PyIndexingResult {
    pub index: PyIndexConfig,
    pub statistics_json: bytes::Bytes,
    pub debug_info: Option<re_protos::cloud::v1alpha1::DebugInfo>,
}

#[pymethods]
impl PyIndexingResult {
    /// Returns configuration information and properties about the newly created index.
    #[getter]
    pub fn properties(&self) -> PyIndexConfig {
        self.index.clone()
    }

    /// Returns the component column that this index was created on.
    #[getter]
    pub fn column(&self) -> PyComponentColumnSelector {
        self.index.component_column()
    }

    /// Returns best-effort backend-specific statistics about the newly created index.
    //
    // TODO(RR-2824): should this deserialize and return a native dict?
    #[getter]
    pub fn statistics(&self) -> String {
        String::from_utf8_lossy(&self.statistics_json).to_string()
    }

    /// Get debug information about the indexing operation.
    ///
    /// The exact contents of debug information may vary depending on the indexing operation performed
    /// and the server implementation.
    ///
    /// Returns
    /// -------
    /// Optional[dict]
    ///     A dictionary containing debug information, or `None` if no debug information is available
    #[allow(clippy::allow_attributes, rustdoc::broken_intra_doc_links)]
    fn debug_info(&self, py: Python<'_>) -> PyResult<Option<Py<PyDict>>> {
        match &self.debug_info {
            Some(debug_info) => {
                let dict = PyDict::new(py);

                if let Some(memory_used) = debug_info.memory_used {
                    dict.set_item("memory_used", memory_used)?;
                }

                Ok(Some(dict.into()))
            }
            None => Ok(None),
        }
    }

    pub fn __repr__(&self) -> String {
        // Technically not a repr, but nice to printout when this is in a list.
        format!("IndexingResult(index={})", self.index)
    }
}

// ---

/// The complete description of a user-defined index.
#[pyclass(eq, name = "IndexConfig", module = "rerun_bindings.rerun_bindings")]
#[derive(Clone, PartialEq, Eq)]
pub struct PyIndexConfig {
    pub time_index: PyIndexColumnSelector,
    pub column: PyComponentColumnSelector,
    pub properties: PyIndexProperties,
}

impl std::fmt::Display for PyIndexConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            time_index,
            column,
            properties,
        } = self;

        f.write_fmt(format_args!("'{column}' on '{time_index}': {properties}"))
    }
}

// TODO(RR-2824): this should probably expose quite a bit more than that.
#[pymethods]
impl PyIndexConfig {
    pub fn __str__(&self) -> String {
        self.to_string()
    }

    pub fn __repr__(&self) -> String {
        format!("IndexConfig({self})")
    }

    /// Returns the time column that this index applies to.
    #[getter]
    pub fn time_column(&self) -> PyIndexColumnSelector {
        self.time_index.clone()
    }

    /// Returns the component column that this index applies to.
    #[getter]
    pub fn component_column(&self) -> PyComponentColumnSelector {
        self.column.clone()
    }

    /// Returns the properties/configuration of the index.
    #[getter]
    pub fn properties(&self) -> PyIndexProperties {
        self.properties.clone()
    }
}

impl From<re_protos::cloud::v1alpha1::ext::IndexConfig> for PyIndexConfig {
    fn from(value: re_protos::cloud::v1alpha1::ext::IndexConfig) -> Self {
        Self {
            time_index: PyIndexColumnSelector(value.time_index.into()),
            column: PyComponentColumnSelector(ComponentColumnSelector::from_descriptor(
                value.column.entity_path,
                &value.column.descriptor,
            )),
            properties: PyIndexProperties {
                props: value.properties,
            },
        }
    }
}

// ---

/// The properties and configuration of a user-defined index.
#[pyclass(eq, name = "IndexProperties", module = "rerun_bindings.rerun_bindings")]
#[derive(Clone, PartialEq, Eq)]
pub struct PyIndexProperties {
    pub props: IndexProperties,
}

impl std::fmt::Display for PyIndexProperties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { props } = self;

        f.write_fmt(format_args!("{props}"))
    }
}

// TODO(RR-2824): this should probably expose quite a bit more than that; for now this is only
// really useful for printing.
#[pymethods]
impl PyIndexProperties {
    pub fn __str__(&self) -> String {
        self.to_string()
    }

    pub fn __repr__(&self) -> String {
        format!("IndexProperties({self})")
    }
}

impl From<IndexProperties> for PyIndexProperties {
    fn from(props: IndexProperties) -> Self {
        Self { props }
    }
}

// ---

/// The type of distance metric to use for vector index and search.
#[pyclass(
    name = "VectorDistanceMetric",
    eq,
    eq_int,
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PyVectorDistanceMetric {
    L2,
    Cosine,
    Dot,
    Hamming,
}

impl From<PyVectorDistanceMetric> for re_protos::cloud::v1alpha1::VectorDistanceMetric {
    fn from(metric: PyVectorDistanceMetric) -> Self {
        match metric {
            PyVectorDistanceMetric::L2 => Self::L2,
            PyVectorDistanceMetric::Cosine => Self::Cosine,
            PyVectorDistanceMetric::Dot => Self::Dot,
            PyVectorDistanceMetric::Hamming => Self::Hamming,
        }
    }
}

/// A type alias for either a `VectorDistanceMetric` enum or a string literal.
#[derive(FromPyObject)]
pub enum VectorDistanceMetricLike {
    #[pyo3(transparent, annotation = "enum")]
    VectorDistanceMetric(PyVectorDistanceMetric),

    #[pyo3(transparent, annotation = "literal")]
    CatchAll(String),
}

impl TryFrom<VectorDistanceMetricLike> for re_protos::cloud::v1alpha1::VectorDistanceMetric {
    type Error = PyErr;

    fn try_from(metric: VectorDistanceMetricLike) -> Result<Self, PyErr> {
        match metric {
            VectorDistanceMetricLike::VectorDistanceMetric(metric) => Ok(metric.into()),
            VectorDistanceMetricLike::CatchAll(metric) => match metric.to_lowercase().as_str() {
                "l2" => Ok(PyVectorDistanceMetric::L2.into()),
                "cosine" => Ok(PyVectorDistanceMetric::Cosine.into()),
                "dot" => Ok(PyVectorDistanceMetric::Dot.into()),
                "hamming" => Ok(PyVectorDistanceMetric::Hamming.into()),
                _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Unknown vector distance metric: {metric}"
                ))),
            },
        }
    }
}

impl From<PyVectorDistanceMetric> for i32 {
    fn from(metric: PyVectorDistanceMetric) -> Self {
        let proto_typed = re_protos::cloud::v1alpha1::VectorDistanceMetric::from(metric);

        proto_typed as Self
    }
}

// ---

/// A type alias for a vector (vector search input data).
#[derive(FromPyObject)]
pub enum VectorLike<'py> {
    NumPy(numpy::PyArrayLike1<'py, f32>),
    Vector(Vec<f32>),
}

impl VectorLike<'_> {
    pub fn to_record_batch(&self) -> PyResult<RecordBatch> {
        let schema = arrow::datatypes::Schema::new_with_metadata(
            vec![Field::new(
                "items",
                arrow::datatypes::DataType::Float32,
                false,
            )],
            Default::default(),
        );

        match self {
            VectorLike::NumPy(array) => {
                let floats: Vec<f32> = array
                    .as_array()
                    .as_slice()
                    .ok_or_else(|| {
                        PyRuntimeError::new_err("Failed to convert numpy array to slice".to_owned())
                    })?
                    .to_vec();

                RecordBatch::try_new_with_options(
                    Arc::new(schema),
                    vec![Arc::new(Float32Array::from(floats))],
                    &RecordBatchOptions::default(),
                )
                .map_err(to_py_err)
            }
            VectorLike::Vector(floats) => RecordBatch::try_new_with_options(
                Arc::new(schema),
                vec![Arc::new(Float32Array::from(floats.clone()))],
                &RecordBatchOptions::default(),
            )
            .map_err(to_py_err),
        }
    }
}
