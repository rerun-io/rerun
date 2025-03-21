use crate::catalog::CatalogConnectionHandle;
use pyo3::pyclass;
use re_protos::catalog::v1alpha1::DatasetEntry;

#[pyclass(name = "Dataset")]
pub struct PyDataset {
    connection: CatalogConnectionHandle,

    entry: DatasetEntry,
}
