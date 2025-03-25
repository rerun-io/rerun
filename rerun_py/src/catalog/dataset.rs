use pyo3::{pyclass, pymethods, PyRefMut, PyResult, Python};

use re_protos::common::v1alpha1::DatasetHandle;

use crate::catalog::PyEntry;

#[pyclass(name = "Dataset", extends=PyEntry)]
pub struct PyDataset {
    pub dataset_handle: DatasetHandle,
}

#[pymethods]
impl PyDataset {
    //TODO(ab): this should be a method on PyEntry
    fn delete(mut self_: PyRefMut<'_, Self>, py: Python<'_>) -> PyResult<()> {
        let super_ = self_.as_super();
        let entry_id = super_.id.borrow(py).id;
        let mut connection = super_.client.borrow_mut(py).connection().clone();

        connection.delete_dataset(py, entry_id)
    }

    #[getter]
    fn manifest_url(&self) -> Option<String> {
        self.dataset_handle.dataset_url.clone()
    }
}
