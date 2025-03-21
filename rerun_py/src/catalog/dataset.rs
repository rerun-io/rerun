use crate::catalog::{CatalogConnectionHandle, PyEntry};
use pyo3::{pyclass, pymethods, PyRefMut, PyResult, Python};
use re_protos::common::v1alpha1::DatasetHandle;

#[pyclass(name = "Dataset", extends=PyEntry)]
pub struct PyDataset {
    pub connection: CatalogConnectionHandle,
    //TODO
    // Note: the `EntryDetail` is stored in the parent class
    //pub dataset_handle: DatasetHandle,
}

#[pymethods]
impl PyDataset {
    //TODO(ab): this should be a method on PyEntry
    fn delete(mut self_: PyRefMut<'_, Self>, py: Python<'_>) -> PyResult<()> {
        let entry_id = self_.as_super().id.borrow(py).id;

        self_.connection.delete_dataset(entry_id.into())
    }

    //TODO
    // #[getter]
    // fn manifest_url(&self) -> Option<String> {
    //     self.dataset_handle.dataset_url.clone()
    // }
}
