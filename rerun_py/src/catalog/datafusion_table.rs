use std::sync::Arc;

use arrow::array::RecordBatchReader;
use arrow::pyarrow::PyArrowType;
use datafusion::catalog::TableProvider;
use datafusion_ffi::table_provider::FFI_TableProvider;
use datafusion_python::context::PySessionContext;
use datafusion_python::dataframe::PyDataFrame;
use pyo3::prelude::PyAnyMethods as _;
use pyo3::types::PyCapsule;
use pyo3::{Bound, IntoPyObjectExt, Py, PyAny, PyRef, PyResult, Python, pyclass, pymethods};
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
    fn df(self_: PyRef<'_, Self>) -> PyResult<PyDataFrame> {
        let py = self_.py();

        let client = self_.client.borrow(py);

        let ctx = client.ctx(py)?;

        drop(client);

        let name = self_.name.clone();

        // Access the underlying PySessionContext
        let mut py_session_ctx = ctx.borrow_mut(py);

        // We're fine with this failing.
        let _ = PySessionContext::deregister_table(&mut py_session_ctx, &name);

        PySessionContext::register_table_provider(
            &mut py_session_ctx,
            &name,
            self_.into_bound_py_any(py)?,
        )?;

        // Get the table as a DataFusion DataFrame
        let df = py_session_ctx.table(&name, py)?;
        let df = Py::new(py, df)?;

        Ok(py)
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
