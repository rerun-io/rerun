use pyo3::{pyclass, pymethods};

use re_protos::common::v1alpha1::ext::DatasetHandle;

use crate::catalog::PyEntry;

#[pyclass(name = "Dataset", extends=PyEntry)]
pub struct PyDataset {
    pub dataset_handle: DatasetHandle,
}

#[pymethods]
impl PyDataset {
    #[getter]
    fn manifest_url(&self) -> String {
        self.dataset_handle.url.clone()
    }
}
