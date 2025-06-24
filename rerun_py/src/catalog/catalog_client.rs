use pyo3::{
    Py, PyAny, PyResult, Python,
    exceptions::{PyLookupError, PyRuntimeError},
    pyclass, pymethods,
    types::PyAnyMethods as _,
};

use re_protos::catalog::v1alpha1::{EntryFilter, EntryKind};

use crate::catalog::{
    ConnectionHandle, PyDatasetEntry, PyEntry, PyEntryId, PyRerunHtmlTable, PyTableEntry, to_py_err,
};

/// Client for a remote Rerun catalog server.
#[pyclass(name = "CatalogClientInternal")]
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

        let connection_registry = re_grpc_client::ConnectionRegistry::new();

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
            .and_then(|datafusion| datafusion.getattr("html_formatter"))
            .and_then(|html_formatter| html_formatter.getattr("set_formatter"));

        if let Ok(format_fn) = format_fn {
            let _ = format_fn.call1((html_renderer,))?;
        }

        Ok(Self {
            origin,
            connection,
            datafusion_ctx,
        })
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

                let entry = PyEntry {
                    client: self_.clone_ref(py),
                    id,
                    details,
                };

                let table = PyTableEntry::default();

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

    //TODO(#9360): `dataset_from_url()`

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

        let entry = PyEntry {
            client,
            id,
            details: table_entry.details,
        };

        let table = PyTableEntry::default();

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
