#![expect(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value

mod catalog_client;
mod connection_handle;
mod dataframe_query;
mod dataset_entry;
mod entry;
mod errors;
mod table_entry;
mod task;

use std::sync::Arc;

use arrow::{
    array::{Float32Array, RecordBatch},
    datatypes::Field,
};
use pyo3::{Bound, PyResult, exceptions::PyRuntimeError, prelude::*};

use crate::catalog::dataframe_query::PyDataframeQueryView;

pub(crate) use catalog_client::PyCatalogClientInternal;
pub(crate) use connection_handle::ConnectionHandle;
pub(crate) use dataset_entry::PyDatasetEntry;
pub(crate) use entry::{PyEntry, PyEntryId, PyEntryKind};
pub(crate) use errors::to_py_err;
pub(crate) use table_entry::PyTableEntry;
pub(crate) use task::{PyTask, PyTasks};

/// Register the `rerun.catalog` module.
pub(crate) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCatalogClientInternal>()?;

    m.add_class::<PyEntryId>()?;
    m.add_class::<PyEntryKind>()?;
    m.add_class::<PyEntry>()?;
    m.add_class::<PyDatasetEntry>()?;
    m.add_class::<PyTableEntry>()?;
    m.add_class::<PyTask>()?;
    m.add_class::<PyTasks>()?;

    m.add_class::<PyDataframeQueryView>()?;

    m.add_class::<PyVectorDistanceMetric>()?;

    Ok(())
}

// TODO(ab): when the new query APIs are implemented, move these type next to it (they were salvaged
// from the legacy server API)

/// The type of distance metric to use for vector index and search.
#[pyclass(name = "VectorDistanceMetric", eq, eq_int)]
#[derive(Clone, Debug, PartialEq)]
enum PyVectorDistanceMetric {
    L2,
    Cosine,
    Dot,
    Hamming,
}

impl From<PyVectorDistanceMetric> for re_protos::manifest_registry::v1alpha1::VectorDistanceMetric {
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
enum VectorDistanceMetricLike {
    #[pyo3(transparent, annotation = "enum")]
    VectorDistanceMetric(PyVectorDistanceMetric),

    #[pyo3(transparent, annotation = "literal")]
    CatchAll(String),
}

impl TryFrom<VectorDistanceMetricLike>
    for re_protos::manifest_registry::v1alpha1::VectorDistanceMetric
{
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
        let proto_typed =
            re_protos::manifest_registry::v1alpha1::VectorDistanceMetric::from(metric);

        proto_typed as Self
    }
}

/// A type alias for a vector (vector search input data).
#[derive(FromPyObject)]
enum VectorLike<'py> {
    NumPy(numpy::PyArrayLike1<'py, f32>),
    Vector(Vec<f32>),
}

impl VectorLike<'_> {
    fn to_record_batch(&self) -> PyResult<RecordBatch> {
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

                RecordBatch::try_new(Arc::new(schema), vec![Arc::new(Float32Array::from(floats))])
                    .map_err(to_py_err)
            }
            VectorLike::Vector(floats) => RecordBatch::try_new(
                Arc::new(schema),
                vec![Arc::new(Float32Array::from(floats.clone()))],
            )
            .map_err(to_py_err),
        }
    }
}
