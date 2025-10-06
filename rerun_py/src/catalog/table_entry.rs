use std::sync::Arc;

use datafusion::catalog::TableProvider;
use datafusion_ffi::table_provider::FFI_TableProvider;
use pyo3::{
    Bound, PyAny, PyRef, PyRefMut, PyResult, Python,
    exceptions::PyRuntimeError,
    pyclass, pymethods,
    types::{PyAnyMethods as _, PyCapsule},
};
use tracing::instrument;

use re_datafusion::TableEntryTableProvider;

use crate::catalog::to_py_err;
use crate::{
    catalog::PyEntry,
    utils::{get_tokio_runtime, wait_for_future},
};

/// A table entry in the catalog.
///
/// Note: this object acts as a table provider for DataFusion.
//TODO(ab): expose metadata about the table (e.g. stuff found in `provider_details`).
#[pyclass(name = "TableEntry", extends=PyEntry)] // NOLINT: skip pyclass_eq, non-trivial implementation
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
    pub fn df(self_: PyRef<'_, Self>) -> PyResult<Bound<'_, PyAny>> {
        let py = self_.py();

        let super_ = self_.as_super();
        let client = super_.client.borrow(py);
        let table_name = super_.name().clone();
        let ctx = client.ctx(py)?;
        let ctx = ctx.bind(py);

        // Any tables for which we have a TableEntry are already
        // registered with the CatalogProvider.

        let df = ctx.call_method1("table", (table_name,))?;

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
