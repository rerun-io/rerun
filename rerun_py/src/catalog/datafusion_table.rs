use std::sync::Arc;

use arrow::array::RecordBatchReader;
use arrow::pyarrow::PyArrowType;
use datafusion::catalog::TableProvider;
use datafusion_ffi::table_provider::FFI_TableProvider;
use pyo3::prelude::PyAnyMethods as _;
use pyo3::types::PyCapsule;
use pyo3::{Bound, Py, PyAny, PyRef, PyResult, Python, pyclass, pymethods};
use tracing::instrument;

use crate::arrow::datafusion_table_provider_to_arrow_reader;
use crate::catalog::PyCatalogClientInternal;
use crate::utils::{get_tokio_runtime, wait_for_future};

#[pyclass(frozen, name = "DataFusionTable")]
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

        let name = self_.name.clone();

        // We're fine with this failing.
        ctx.call_method1("deregister_table", (name.clone(),))?;

        ctx.call_method1("register_table_provider", (name.clone(), self_))?;

        let df = ctx.call_method1("table", (name.clone(),))?;

        Ok(df)
    }

    /// Convert this table to a [`pyarrow.RecordBatchReader`][].
    #[instrument(skip_all)]
    fn to_arrow_reader<'py>(
        self_: PyRef<'py, Self>,
        py: Python<'py>,
    ) -> PyResult<PyArrowType<Box<dyn RecordBatchReader + Send>>> {
        let table_provider = Arc::clone(&self_.provider);

        let reader = wait_for_future(
            py,
            datafusion_table_provider_to_arrow_reader(table_provider),
        )?;

        Ok(PyArrowType(reader))
    }

    /// Name of this table.
    #[getter]
    fn name(&self) -> String {
        self.name.clone()
    }
}
