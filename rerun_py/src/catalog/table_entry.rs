use std::sync::Arc;

use arrow::array::RecordBatchReader;
use arrow::pyarrow::PyArrowType;
use datafusion::catalog::TableProvider;
use datafusion_ffi::table_provider::FFI_TableProvider;
use datafusion_python::context::PySessionContext;
use datafusion_python::dataframe::PyDataFrame;
use pyo3::{
    Bound, IntoPyObjectExt as _, Py, PyRef, PyRefMut, PyResult, Python, exceptions::PyRuntimeError,
    pyclass, pymethods, types::PyCapsule,
};
use tracing::instrument;

use re_datafusion::TableEntryTableProvider;

use crate::arrow::datafusion_table_provider_to_arrow_reader;
use crate::catalog::to_py_err;
use crate::{
    catalog::PyEntry,
    utils::{get_tokio_runtime, wait_for_future},
};

/// A table entry in the catalog.
///
/// Note: this object acts as a table provider for DataFusion.
//TODO(ab): expose metadata about the table (e.g. stuff found in `provider_details`).
#[pyclass(name = "TableEntry", extends=PyEntry)]
#[derive(Default)]
pub struct PyTableEntry {
    lazy_provider: Option<Arc<dyn TableProvider + Send>>,
}

#[pymethods]
impl PyTableEntry {
    /// Returns a DataFusion table provider capsule.
    #[instrument(skip_all)]
    fn __datafusion_table_provider__<'py>(
        self_: PyRefMut<'py, Self>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let provider = Self::table_provider(self_)?;

        let capsule_name = cr"datafusion_table_provider".into();

        let runtime = get_tokio_runtime().handle().clone();
        let provider = FFI_TableProvider::new(provider, false, Some(runtime));

        PyCapsule::new(py, provider, Some(capsule_name))
    }

    /// Registers the table with the DataFusion context and return a DataFrame.
    // add `ctx=None, name=None`
    pub fn df(self_: PyRef<'_, Self>) -> PyResult<Py<PyDataFrame>> {
        let py = self_.py();

        let super_ = self_.as_super();
        let client = super_.client.borrow(py);
        let table_name = super_.name().clone();
        let ctx = client.ctx(py)?;

        drop(client);

        // Access the underlying PySessionContext
        let mut py_session_ctx = ctx.borrow_mut(py);

        // We're fine with this failing.
        PySessionContext::deregister_table(&mut py_session_ctx, &table_name)?;

        PySessionContext::register_table_provider(
            &mut py_session_ctx,
            &table_name,
            self_.into_bound_py_any(py)?,
        )?;

        // Get the table as a DataFusion DataFrame
        let df = py_session_ctx.table(&table_name, py)?;
        let df = Py::new(py, df)?;

        Ok(df)
    }

    /// Convert this table to a [`pyarrow.RecordBatchReader`][].
    #[instrument(skip_all)]
    fn to_arrow_reader<'py>(
        self_: PyRefMut<'py, Self>,
        py: Python<'py>,
    ) -> PyResult<PyArrowType<Box<dyn RecordBatchReader + Send>>> {
        let table_provider = Self::table_provider(self_)?;

        let reader = wait_for_future(
            py,
            datafusion_table_provider_to_arrow_reader(table_provider),
        )?;

        Ok(PyArrowType(reader))
    }
}

impl PyTableEntry {
    fn table_provider(mut self_: PyRefMut<'_, Self>) -> PyResult<Arc<dyn TableProvider + Send>> {
        let py = self_.py();
        if self_.lazy_provider.is_none() {
            let super_ = self_.as_mut();

            let id = super_.id.borrow(py).id;

            let connection = super_.client.borrow_mut(py).connection().clone();

            self_.lazy_provider = Some(
                wait_for_future(py, async {
                    TableEntryTableProvider::new(connection.client().await?, id)
                        .into_provider()
                        .await
                        .map_err(to_py_err)
                })
                .map_err(|err| {
                    PyRuntimeError::new_err(format!("Error creating TableProvider: {err}"))
                })?,
            );
        }

        let provider = self_
            .lazy_provider
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("Missing TableProvider".to_owned()))?
            .clone();

        Ok(provider)
    }
}
