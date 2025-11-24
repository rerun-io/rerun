use std::sync::Arc;

use datafusion::catalog::TableProvider;
use datafusion_ffi::table_provider::FFI_TableProvider;
use pyo3::{
    Bound, Py, PyAny, PyRef, PyResult, Python, prelude::PyAnyMethods as _, pyclass, pymethods,
    types::PyCapsule,
};
use tracing::instrument;

use crate::{catalog::PyCatalogClientInternal, utils::get_tokio_runtime};

#[pyclass( // NOLINT: ignore[py-cls-eq] non-trivial implementation
    frozen,
    name = "DataFusionTable",
    module = "rerun_bindings.rerun_bindings"
)]

pub struct PyDataFusionTable {
    pub provider: Arc<dyn TableProvider + Send>,
    pub name: String,
    pub client: Py<PyCatalogClientInternal>,
}

#[pymethods]
impl PyDataFusionTable {
    /// Returns a DataFusion table provider capsule.
    fn __datafusion_table_provider__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let capsule_name = cr"datafusion_table_provider".into();

        let runtime = get_tokio_runtime().handle().clone();
        let provider = FFI_TableProvider::new(Arc::clone(&self.provider), false, Some(runtime));

        PyCapsule::new(py, provider, Some(capsule_name))
    }

    /// Register this view to the global DataFusion context and return a DataFrame.
    fn df(self_: PyRef<'_, Self>) -> PyResult<Bound<'_, PyAny>> {
        let py = self_.py();

        let client = self_.client.borrow(py);

        let ctx = client.ctx(py)?;
        let ctx = ctx.bind(py);

        drop(client);

        let df = ctx.call_method1("read_table", (self_,))?;

        Ok(df)
    }

    /// Convert this table to a [`pyarrow.RecordBatchReader`][].
    #[instrument(skip_all)]
    fn to_arrow_reader<'py>(
        self_: PyRef<'py, Self>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let df = Self::df(self_)?;

        py.import("pyarrow")?
            .getattr("RecordBatchReader")?
            .call_method1("from_stream", (df,))
    }

    /// Name of this table.
    #[getter]
    fn name(&self) -> String {
        self.name.clone()
    }

    pub fn __str__(&self) -> String {
        format!("DataFusionTable(name='{}')", self.name)
    }
}
