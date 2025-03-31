#![expect(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value

mod catalog_client;
mod connection_handle;
mod dataset;
mod entry;
mod errors;

use pyo3::{prelude::*, Bound, PyResult};

pub use catalog_client::PyCatalogClient;
pub use connection_handle::ConnectionHandle;
pub use dataset::PyDataset;
pub use entry::{PyEntry, PyEntryId, PyEntryKind};
pub use errors::to_py_err;

/// Register the `rerun.catalog` module.
pub(crate) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCatalogClient>()?;

    m.add_class::<PyEntryId>()?;
    m.add_class::<PyEntryKind>()?;
    m.add_class::<PyEntry>()?;

    m.add_class::<PyDataset>()?;

    m.add_class::<PyVectorDistanceMetric>()?;

    Ok(())
}

// TODO(ab): when the new query APIs are implemented, move these type next to it (they were salvaged
// from the legacy server API)

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
