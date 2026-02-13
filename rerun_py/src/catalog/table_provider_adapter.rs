use std::sync::Arc;

use datafusion::catalog::TableProvider;
use datafusion_ffi::table_provider::FFI_TableProvider;
use pyo3::types::PyCapsule;
use pyo3::{Bound, PyResult, Python, pyclass, pymethods};

use crate::utils::get_tokio_runtime;

/// Adapter to expose a [`TableProvider`] to the Python side via the DataFusion FFI capsule protocol.
#[pyclass( // NOLINT: ignore[py-cls-eq] non-trivial implementation
    frozen,
    name = "TableProviderAdapterInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyTableProviderAdapterInternal {
    provider: Arc<dyn TableProvider + Send>,
    streaming: bool,
}

impl PyTableProviderAdapterInternal {
    pub fn new(provider: Arc<dyn TableProvider + Send>, streaming: bool) -> Self {
        Self {
            provider,
            streaming,
        }
    }
}

#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PyTableProviderAdapterInternal {
    fn __datafusion_table_provider__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let capsule_name = cr"datafusion_table_provider".into();

        let runtime = get_tokio_runtime().handle().clone();
        let provider =
            FFI_TableProvider::new(Arc::clone(&self.provider), self.streaming, Some(runtime));

        PyCapsule::new(py, provider, Some(capsule_name))
    }
}
