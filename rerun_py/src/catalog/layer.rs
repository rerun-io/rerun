use pyo3::{Py, PyResult, Python, pyclass, pymethods};
use re_types_core::LayerName;

use super::{PyDatasetEntryInternal, to_py_err};
use crate::trace_context::read_trace_context_from_python;
use crate::utils::wait_for_future;

/// A handle to a named layer across a dataset's segments.
#[pyclass(name = "LayerInternal", module = "rerun_bindings.rerun_bindings")]
pub struct PyLayerInternal {
    dataset: Py<PyDatasetEntryInternal>,
    name: LayerName,
}

impl PyLayerInternal {
    pub fn new(dataset: Py<PyDatasetEntryInternal>, name: LayerName) -> Self {
        Self { dataset, name }
    }
}

#[pymethods]
impl PyLayerInternal {
    #[getter]
    fn name(&self) -> &str {
        self.name.as_str()
    }

    #[getter]
    fn dataset(&self, py: Python<'_>) -> Py<PyDatasetEntryInternal> {
        self.dataset.clone_ref(py)
    }

    fn __str__(&self) -> &str {
        self.name.as_str()
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "Layer(dataset_id={}, name={:?})",
            self.dataset.borrow(py).entry_id(),
            self.name.as_str()
        )
    }

    fn segment_ids(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        let _span = read_trace_context_from_python(py, "Layer.segment_ids").entered();
        let Self { dataset, name } = self;
        let (connection, dataset_id) = {
            let dataset = dataset.borrow(py);
            (
                dataset.client().borrow(py).connection().clone(),
                dataset.entry_id(),
            )
        };
        let ids = wait_for_future(py, async {
            connection
                .inner()
                .get_layer_segment_ids(dataset_id, name.clone())
                .await
                .map_err(to_py_err)
        })?;
        Ok(ids.into_iter().map(|id| id.to_string()).collect())
    }
}
