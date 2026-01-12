use std::sync::Arc;

use arrow::ffi_stream::ArrowArrayStreamReader;
use arrow::pyarrow::FromPyArrow as _;
use datafusion::catalog::TableProvider;
use datafusion_ffi::table_provider::FFI_TableProvider;
use pyo3::exceptions::PyRuntimeError;
use pyo3::types::{PyAnyMethods as _, PyCapsule};
use pyo3::{Bound, Py, PyAny, PyRef, PyRefMut, PyResult, Python, pyclass, pymethods};
use re_datafusion::TableEntryTableProvider;
use re_protos::cloud::v1alpha1::ext::{EntryDetails, ProviderDetails, TableEntry, TableInsertMode};
use tracing::instrument;

use crate::catalog::entry::set_entry_name;
use crate::catalog::{PyCatalogClientInternal, PyEntryDetails, to_py_err};
use crate::utils::{get_tokio_runtime, wait_for_future};

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

    fn set_name(&mut self, py: Python<'_>, name: String) -> PyResult<()> {
        set_entry_name(py, name, &mut self.entry_details, &self.client)
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
    pub fn reader(self_: PyRef<'_, Self>) -> PyResult<Bound<'_, PyAny>> {
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
        let df = Self::reader(self_)?;

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

    /// Write record batches to the table.
    #[instrument(skip_all)]
    fn write_batches(
        self_: Py<Self>,
        py: Python<'_>,
        batches: &Bound<'_, PyAny>,
        insert_mode: PyTableInsertModeInternal,
    ) -> PyResult<()> {
        let entry_id = self_.borrow(py).entry_details.id;
        let connection = self_
            .borrow_mut(py)
            .client
            .borrow_mut(py)
            .connection()
            .clone();
        let stream = ArrowArrayStreamReader::from_pyarrow_bound(batches)?;
        connection.write_table(py, entry_id, stream, insert_mode)?;
        Ok(())
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
    name = "TableInsertModeInternal",
    eq,
    eq_int,
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum_macros::EnumIter)]
pub enum PyTableInsertModeInternal {
    #[pyo3(name = "APPEND")]
    Append = 1,

    #[pyo3(name = "OVERWRITE")]
    Overwrite = 2,

    #[pyo3(name = "REPLACE")]
    Replace = 3,
}

impl From<PyTableInsertModeInternal> for TableInsertMode {
    fn from(value: PyTableInsertModeInternal) -> Self {
        match value {
            PyTableInsertModeInternal::Append => Self::Append,
            PyTableInsertModeInternal::Overwrite => Self::Overwrite,
            PyTableInsertModeInternal::Replace => Self::Replace,
        }
    }
}
