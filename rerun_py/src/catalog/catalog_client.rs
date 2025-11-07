use arrow::datatypes::Schema;
use arrow::ffi_stream::ArrowArrayStreamReader;
use arrow::pyarrow::PyArrowType;
use pyo3::exceptions::PyValueError;
use pyo3::{
    Bound, Py, PyAny, PyResult, Python,
    exceptions::{PyLookupError, PyRuntimeError},
    pyclass, pymethods,
    types::PyAnyMethods as _,
};
use re_datafusion::{DEFAULT_CATALOG_NAME, get_all_catalog_names};
use re_protos::cloud::v1alpha1::{EntryFilter, EntryKind};
use std::sync::Arc;

use crate::catalog::datafusion_catalog::PyDataFusionCatalogProvider;
use crate::catalog::table_entry::PyTableInsertMode;
use crate::catalog::{
    ConnectionHandle, PyDatasetEntry, PyEntry, PyEntryId, PyRerunHtmlTable, PyTableEntry, to_py_err,
};
use crate::utils::{get_tokio_runtime, wait_for_future};
use arrow::pyarrow::FromPyArrow as _;

/// Client for a remote Rerun catalog server.
#[pyclass(  // NOLINT: ignore[py-cls-eq] non-trivial implementation
    name = "CatalogClientInternal",
    module = "rerun_bindings.rerun_bindings"
)]

pub struct PyCatalogClientInternal {
    origin: re_uri::Origin,

    connection: ConnectionHandle,

    // If this isn't set, it means datafusion wasn't found
    datafusion_ctx: Option<Py<PyAny>>,
}

impl PyCatalogClientInternal {
    pub fn connection(&self) -> &ConnectionHandle {
        &self.connection
    }
}

#[pymethods]
impl PyCatalogClientInternal {
    #[staticmethod]
    pub fn datafusion_major_version() -> u64 {
        datafusion_ffi::version()
    }

    /// Create a new catalog client object.
    #[new]
    #[pyo3(text_signature = "(self, addr, token=None)")]
    fn new(py: Python<'_>, addr: String, token: Option<String>) -> PyResult<Self> {
        // NOTE: The entire TLS stack expects this global variable to be set. It doesn't matter
        // what we set it to. But we have to set it, or we will crash at runtime, as soon as
        // anything tries to do anything TLS-related.
        // This used to be implicitly done by `object_store`, just by virtue of depending on it,
        // but we removed that unused dependency, so now we must do it ourselves.
        _ = rustls::crypto::ring::default_provider().install_default();

        let origin = addr.as_str().parse::<re_uri::Origin>().map_err(to_py_err)?;

        let connection_registry =
            re_redap_client::ConnectionRegistry::new_with_stored_credentials();

        let token = token
            .map(TryFrom::try_from)
            .transpose()
            .map_err(to_py_err)?;
        if let Some(token) = token {
            connection_registry.set_token(&origin, token);
        }

        let connection = ConnectionHandle::new(connection_registry, origin.clone());

        let datafusion_ctx = py
            .import("datafusion")
            .and_then(|datafusion| Ok(datafusion.getattr("SessionContext")?.call0()?.unbind()))
            .ok();

        // Set up our renderer by default since we've already established datafusion
        // is installed.
        let html_renderer = PyRerunHtmlTable::new(None, None);

        let format_fn = py
            .import("datafusion")
            .and_then(|datafusion| datafusion.getattr("dataframe_formatter"))
            .and_then(|df_formatter| df_formatter.getattr("set_formatter"));

        let client = wait_for_future(py, connection.client())?;
        let runtime = get_tokio_runtime().handle();
        let provider_names = get_all_catalog_names(&client, runtime).map_err(to_py_err)?;
        let mut providers = provider_names
            .iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>();
        if !providers.contains(&DEFAULT_CATALOG_NAME) {
            providers.push(DEFAULT_CATALOG_NAME);
        }

        if let Some(ctx) = datafusion_ctx.as_ref() {
            for provider_name in providers {
                let catalog_provider = PyDataFusionCatalogProvider::new(
                    Some(provider_name.to_owned()),
                    client.clone(),
                );

                ctx.call_method1(
                    py,
                    "register_catalog_provider",
                    (provider_name, catalog_provider),
                )?;
            }
        }

        if let Ok(format_fn) = format_fn {
            let _ = format_fn.call1((html_renderer,))?;
        }

        Ok(Self {
            origin,
            connection,
            datafusion_ctx,
        })
    }

    /// Get the URL of the catalog (a `rerun+http` URL).
    #[getter]
    pub fn url(&self) -> String {
        self.origin.to_string()
    }

    /// Get a list of all entries in the catalog.
    fn all_entries(self_: Py<Self>, py: Python<'_>) -> PyResult<Vec<Py<PyEntry>>> {
        let connection = self_.borrow(py).connection.clone();

        let entry_details = connection.find_entries(py, EntryFilter::new())?;

        // Generate entry objects.
        entry_details
            .into_iter()
            .map(|details| {
                let id = Py::new(py, PyEntryId::from(details.id))?;
                Py::new(
                    py,
                    PyEntry {
                        client: self_.clone_ref(py),
                        id,
                        details,
                    },
                )
            })
            .collect()
    }

    /// Get a list of all dataset entries in the catalog.
    fn dataset_entries(self_: Py<Self>, py: Python<'_>) -> PyResult<Vec<Py<PyDatasetEntry>>> {
        let connection = self_.borrow(py).connection.clone();

        let entry_details =
            connection.find_entries(py, EntryFilter::new().with_entry_kind(EntryKind::Dataset))?;

        entry_details
            .into_iter()
            .map(|details| {
                let id = Py::new(py, PyEntryId::from(details.id))?;
                let dataset_entry = connection.read_dataset(py, details.id)?;

                let entry = PyEntry {
                    client: self_.clone_ref(py),
                    id,
                    details,
                };

                let dataset = PyDatasetEntry {
                    dataset_details: dataset_entry.dataset_details,
                    dataset_handle: dataset_entry.handle,
                };

                Py::new(py, (dataset, entry))
            })
            .collect()
    }

    /// Get a list of all table entries in the catalog.
    fn table_entries(self_: Py<Self>, py: Python<'_>) -> PyResult<Vec<Py<PyTableEntry>>> {
        let connection = self_.borrow(py).connection.clone();

        let entry_details =
            connection.find_entries(py, EntryFilter::new().with_entry_kind(EntryKind::Table))?;

        entry_details
            .into_iter()
            .map(|details| {
                let id = Py::new(py, PyEntryId::from(details.id))?;

                let table_entry = connection.read_table(py, details.id)?;
                let table = PyTableEntry::new(&table_entry);

                let entry = PyEntry {
                    client: self_.clone_ref(py),
                    id,
                    details,
                };

                Py::new(py, (table, entry))
            })
            .collect()
    }

    // ---

    fn entry_names(self_: Py<Self>, py: Python<'_>) -> PyResult<Vec<String>> {
        let connection = self_.borrow(py).connection.clone();

        let entry_details = connection.find_entries(py, EntryFilter::new())?;

        Ok(entry_details
            .into_iter()
            .map(|details| details.name)
            .collect())
    }

    fn dataset_names(self_: Py<Self>, py: Python<'_>) -> PyResult<Vec<String>> {
        let connection = self_.borrow(py).connection.clone();

        let entry_details =
            connection.find_entries(py, EntryFilter::new().with_entry_kind(EntryKind::Dataset))?;

        Ok(entry_details
            .into_iter()
            .map(|details| details.name)
            .collect())
    }

    fn table_names(self_: Py<Self>, py: Python<'_>) -> PyResult<Vec<String>> {
        let connection = self_.borrow(py).connection.clone();

        let entry_details =
            connection.find_entries(py, EntryFilter::new().with_entry_kind(EntryKind::Table))?;

        Ok(entry_details
            .into_iter()
            .map(|details| details.name)
            .collect())
    }

    // ---

    /// Get a dataset by name or id.
    fn get_dataset_entry(
        self_: Py<Self>,
        id: Py<PyEntryId>,
        py: Python<'_>,
    ) -> PyResult<Py<PyDatasetEntry>> {
        let connection = self_.borrow(py).connection.clone();

        let client = self_.clone_ref(py);

        let dataset_entry = connection.read_dataset(py, id.borrow(py).id)?;

        let entry = PyEntry {
            client,
            id,
            details: dataset_entry.details,
        };

        let dataset = PyDatasetEntry {
            dataset_details: dataset_entry.dataset_details,
            dataset_handle: dataset_entry.handle,
        };

        Py::new(py, (dataset, entry))
    }

    /// Get a table by name or id.
    ///
    /// Note: the entry table is named `__entries`.
    fn get_table_entry(
        self_: Py<Self>,
        py: Python<'_>,
        id: Py<PyEntryId>,
    ) -> PyResult<Py<PyTableEntry>> {
        let connection = self_.borrow(py).connection.clone();

        let client = self_.clone_ref(py);

        let table_entry = connection.read_table(py, id.borrow(py).id)?;
        let table = PyTableEntry::new(&table_entry);

        let entry = PyEntry {
            client,
            id,
            details: table_entry.details,
        };

        Py::new(py, (table, entry))
    }

    // ---

    /// Create a new dataset with the provided name.
    fn create_dataset(self_: Py<Self>, py: Python<'_>, name: &str) -> PyResult<Py<PyDatasetEntry>> {
        let connection = self_.borrow_mut(py).connection.clone();

        let dataset_entry = connection.create_dataset(py, name.to_owned())?;

        let entry_id = Py::new(py, PyEntryId::from(dataset_entry.details.id))?;

        let entry = PyEntry {
            client: self_.clone_ref(py),
            id: entry_id,
            details: dataset_entry.details,
        };

        let dataset = PyDatasetEntry {
            dataset_details: dataset_entry.dataset_details,
            dataset_handle: dataset_entry.handle,
        };

        Py::new(py, (dataset, entry))
    }

    fn register_table(
        self_: Py<Self>,
        py: Python<'_>,
        name: String,
        url: String,
    ) -> PyResult<Py<PyTableEntry>> {
        let connection = self_.borrow_mut(py).connection.clone();

        let url = url
            .parse::<url::Url>()
            .map_err(|err| PyValueError::new_err(format!("Invalid URL: {err}")))?;

        let table_entry = connection.register_table(py, name, url)?;
        let table = PyTableEntry::new(&table_entry);

        let entry_id = Py::new(py, PyEntryId::from(table_entry.details.id))?;

        let entry = PyEntry {
            client: self_.clone_ref(py),
            id: entry_id,
            details: table_entry.details,
        };

        Py::new(py, (table, entry))
    }

    fn create_table_entry(
        self_: Py<Self>,
        py: Python<'_>,
        name: String,
        schema: PyArrowType<Schema>,
        url: String,
    ) -> PyResult<Py<PyTableEntry>> {
        let connection = self_.borrow_mut(py).connection.clone();

        let url = url
            .parse::<url::Url>()
            .map_err(|err| PyValueError::new_err(format!("Invalid URL: {err}")))?;

        let schema = Arc::new(schema.0);
        let table_entry = connection.create_table_entry(py, name, schema, &url)?;
        let table = PyTableEntry::new(&table_entry);

        let entry_id = Py::new(py, PyEntryId::from(table_entry.details.id))?;

        let entry = PyEntry {
            client: self_.clone_ref(py),
            id: entry_id,
            details: table_entry.details,
        };

        Py::new(py, (table, entry))
    }

    fn write_table(
        self_: Py<Self>,
        py: Python<'_>,
        name: String,
        batches: &Bound<'_, PyAny>,
        insert_mode: PyTableInsertMode,
    ) -> PyResult<()> {
        let connection = self_.borrow_mut(py).connection.clone();

        let stream = ArrowArrayStreamReader::from_pyarrow_bound(batches)?;

        connection.write_table(py, name, stream, insert_mode)?;

        Ok(())
    }

    // ---

    /// Perform global maintenance tasks on the server.
    fn do_global_maintenance(self_: Py<Self>, py: Python<'_>) -> PyResult<()> {
        let connection = self_.borrow_mut(py).connection.clone();

        connection.do_global_maintenance(py)
    }

    // ---

    /// The DataFusion context (if available).
    pub fn ctx(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(datafusion_ctx) = &self.datafusion_ctx {
            Ok(datafusion_ctx.clone_ref(py))
        } else {
            Err(PyRuntimeError::new_err(
                "DataFusion context not available (the `datafusion` package may need to be installed)".to_owned(),
            ))
        }
    }

    fn __repr__(&self) -> String {
        format!("CatalogClient({})", self.origin)
    }

    // ---

    fn _entry_id_from_entry_name(
        self_: Py<Self>,
        py: Python<'_>,
        name: String,
    ) -> PyResult<Py<PyEntryId>> {
        let connection = self_.borrow(py).connection.clone();

        let entry_details = connection.find_entries(py, EntryFilter::new().with_name(&name))?;

        if entry_details.is_empty() {
            return Err(PyLookupError::new_err(format!(
                "No entry found with name {name:?}"
            )));
        }

        Py::new(py, PyEntryId::from(entry_details[0].id))
    }
}
