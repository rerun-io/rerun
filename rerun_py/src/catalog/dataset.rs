use pyo3::{pyclass, pymethods};

use re_protos::common::v1alpha1::DatasetHandle;

use crate::catalog::PyEntry;

#[pyclass(name = "Dataset", extends=PyEntry)]
pub struct PyDataset {
    pub dataset_handle: DatasetHandle,
}

#[pymethods]
impl PyDataset {
    #[getter]
    fn manifest_url(&self) -> Option<String> {
        self.dataset_handle.dataset_url.clone()
    }
}
