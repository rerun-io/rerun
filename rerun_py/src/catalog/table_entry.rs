use std::sync::Arc;

use datafusion::catalog::TableProvider;
use datafusion_ffi::table_provider::FFI_TableProvider;
use pyo3::{
    Bound, Py, PyAny, PyRef, PyRefMut, PyResult, Python,
    exceptions::PyRuntimeError,
    pyclass, pymethods,
    types::{PyAnyMethods as _, PyCapsule},
};
use tracing::instrument;

use re_datafusion::TableEntryTableProvider;
use re_protos::cloud::v1alpha1::ext::{EntryDetails, ProviderDetails, TableEntry, TableInsertMode};

use crate::{
    catalog::{PyCatalogClientInternal, PyEntryDetails, entry::update_entry, to_py_err},
    utils::{get_tokio_runtime, wait_for_future},
};

/// A table entry in the catalog.
///
/// Note: this object acts as a table provider for DataFusion.
//TODO(ab): expose metadata about the table (e.g. stuff found in `provider_details`).
#[pyclass(name = "TableEntryInternal", module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq] non-trivial implementation
pub struct PyTableEntryInternal {
    client: Py<PyCatalogClientInternal>,
    entry_details: EntryDetails,
    lazy_provider: Option<Arc<dyn TableProvider + Send>>,
    url: Option<String>,
}

#[pymethods]
impl PyTableEntryInternal {
    //
    // Entry methods
    //

    fn catalog(&self, py: Python<'_>) -> Py<PyCatalogClientInternal> {
        self.client.clone_ref(py)
    }

    fn entry_details(&self, py: Python<'_>) -> PyResult<Py<PyEntryDetails>> {
        Py::new(py, PyEntryDetails(self.entry_details.clone()))
    }

    /// Delete this entry from the catalog.
    fn delete(&mut self, py: Python<'_>) -> PyResult<()> {
        let connection = self.client.borrow_mut(py).connection().clone();
        connection.delete_entry(py, self.entry_details.id)
    }

    #[pyo3(signature = (*, name=None))]
    fn update(&mut self, py: Python<'_>, name: Option<String>) -> PyResult<()> {
        update_entry(py, name, &mut self.entry_details, &self.client)
    }

    //
    // Table entry methods
    //

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

        let client = self_.client.borrow(py);
        let table_name = self_.entry_details.name.clone();
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

    /// The table's storage URL.
    #[getter]
    pub fn storage_url(&self) -> String {
        self.url.clone().unwrap_or_default()
    }

    pub fn __str__(&self) -> String {
        format!("TableEntry(url='{}')", self.url.clone().unwrap_or_default())
    }
}

impl PyTableEntryInternal {
    pub fn new(client: Py<PyCatalogClientInternal>, table_entry: TableEntry) -> Self {
        let url = match &table_entry.provider_details {
            ProviderDetails::LanceTable(p) => Some(p.table_url.to_string()),
            ProviderDetails::SystemTable(_) => None,
        };

        Self {
            client,
            entry_details: table_entry.details,
            lazy_provider: None,
            url,
        }
    }

    fn table_provider(mut self_: PyRefMut<'_, Self>) -> PyResult<Arc<dyn TableProvider + Send>> {
        let py = self_.py();
        if self_.lazy_provider.is_none() {
            let table_id = self_.entry_details.id;
            let connection = self_.client.borrow_mut(py).connection().clone();

            self_.lazy_provider = Some(
                wait_for_future(py, async {
                    TableEntryTableProvider::new(
                        connection.client().await?,
                        table_id,
                        Some(get_tokio_runtime().handle().clone()),
                    )
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

#[pyclass(
    name = "TableInsertMode",
    eq,
    eq_int,
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum_macros::EnumIter)]
pub enum PyTableInsertMode {
    #[pyo3(name = "APPEND")]
    Append = 1,

    #[pyo3(name = "OVERWRITE")]
    Overwrite = 2,

    #[pyo3(name = "REPLACE")]
    Replace = 3,
}

impl From<PyTableInsertMode> for TableInsertMode {
    fn from(value: PyTableInsertMode) -> Self {
        match value {
            PyTableInsertMode::Append => Self::Append,
            PyTableInsertMode::Overwrite => Self::Overwrite,
            PyTableInsertMode::Replace => Self::Replace,
        }
    }
}
