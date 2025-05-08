use std::str::FromStr as _;

use pyo3::{
    exceptions::{PyLookupError, PyRuntimeError},
    pyclass, pymethods,
    types::PyAnyMethods as _,
    FromPyObject, Py, PyAny, PyResult, Python,
};
use re_log_types::EntryId;
use re_protos::catalog::v1alpha1::EntryFilter;

use crate::catalog::{to_py_err, ConnectionHandle, PyDataset, PyEntry, PyEntryId, PyTable};

/// Client for a remote Rerun catalog server.
#[pyclass(name = "CatalogClient")]
pub struct PyCatalogClient {
    #[expect(dead_code)]
    origin: re_uri::Origin,

    connection: ConnectionHandle,

    // If this isn't set, it means datafusion wasn't found
    datafusion_ctx: Option<Py<PyAny>>,
}

impl PyCatalogClient {
    pub fn connection(&self) -> &ConnectionHandle {
        &self.connection
    }
}

#[pymethods]
impl PyCatalogClient {
    /// Create a new catalog client object.
    #[new]
    fn new(py: Python<'_>, addr: String) -> PyResult<Self> {
        let origin = addr.as_str().parse::<re_uri::Origin>().map_err(to_py_err)?;

        let connection = ConnectionHandle::new(py, origin.clone())?;

        let datafusion_ctx = py
            .import("datafusion")
            .and_then(|datafusion| Ok(datafusion.getattr("SessionContext")?.call0()?.unbind()))
            .ok();

        Ok(Self {
            origin,
            connection,
            datafusion_ctx,
        })
    }

    /// Get a list of all entries in the catalog.
    fn entries(self_: Py<Self>, py: Python<'_>) -> PyResult<Vec<Py<PyEntry>>> {
        let mut connection = self_.borrow(py).connection.clone();

        let entry_details = connection.find_entries(
            py,
            EntryFilter {
                id: None,
                name: None,
                entry_kind: None,
            },
        )?;

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

    /// Get a dataset by name or id.
    fn get_dataset(
        self_: Py<Self>,
        name_or_id: EntryIdLike,
        py: Python<'_>,
    ) -> PyResult<Py<PyDataset>> {
        let mut connection = self_.borrow(py).connection.clone();

        let id = name_or_id.resolve(&mut connection, py)?;

        let entry_id = id.borrow(py).id;

        let client = self_.clone_ref(py);

        let dataset_entry = connection.read_dataset(py, entry_id)?;

        let entry = PyEntry {
            client,
            id,
            details: dataset_entry.details,
        };

        let dataset = PyDataset {
            dataset_handle: dataset_entry.handle,
        };

        Py::new(py, (dataset, entry))
    }

    //TODO(#9369): `datasets()` (needs FindDatasetsEntries rpc)

    /// Create a new dataset with the provided name.
    fn create_dataset(self_: Py<Self>, py: Python<'_>, name: &str) -> PyResult<Py<PyDataset>> {
        let mut connection = self_.borrow_mut(py).connection.clone();

        let dataset_entry = connection.create_dataset(py, name.to_owned())?;

        let entry_id = Py::new(py, PyEntryId::from(dataset_entry.details.id))?;

        let entry = PyEntry {
            client: self_.clone_ref(py),
            id: entry_id,
            details: dataset_entry.details,
        };

        let dataset = PyDataset {
            dataset_handle: dataset_entry.handle,
        };

        Py::new(py, (dataset, entry))
    }

    //TODO(#9360): `dataset_from_url()`

    /// Get a table by name or id.
    ///
    /// Note: the entry table is named `__entries`.
    fn get_table(
        self_: Py<Self>,
        name_or_id: EntryIdLike,
        py: Python<'_>,
    ) -> PyResult<Py<PyTable>> {
        let mut connection = self_.borrow(py).connection.clone();

        let id = name_or_id.resolve(&mut connection, py)?;

        let entry_id = id.borrow(py).id;

        let client = self_.clone_ref(py);

        let dataset_entry = connection.read_table(py, entry_id)?;

        let entry = PyEntry {
            client,
            id,
            details: dataset_entry.details,
        };

        let table = PyTable::default();

        Py::new(py, (table, entry))
    }

    /// Get the entries table.
    fn entries_table(self_: Py<Self>, py: Python<'_>) -> PyResult<Py<PyTable>> {
        Self::get_table(self_, EntryIdLike::Str("__entries".to_owned()), py)
    }

    /// The DataFusion context (if available).
    #[getter]
    pub fn ctx(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(datafusion_ctx) = &self.datafusion_ctx {
            Ok(datafusion_ctx.clone_ref(py))
        } else {
            Err(PyRuntimeError::new_err(
                "DataFusion context not available (the `datafusion` package may need to be installed)".to_owned(),
            ))
        }
    }
}

/// A type alias for a vector (vector search input data).
#[derive(FromPyObject)]
enum EntryIdLike {
    /// Name or id of the entry.
    Str(String),

    /// Id of the entry.
    Id(Py<PyEntryId>),
}

impl EntryIdLike {
    fn resolve(self, connection: &mut ConnectionHandle, py: Python<'_>) -> PyResult<Py<PyEntryId>> {
        match self {
            Self::Str(name_or_id) => {
                // First try to find by name
                let mut entry_details = connection.find_entries(
                    py,
                    EntryFilter {
                        id: None,
                        name: Some(name_or_id.clone()),
                        entry_kind: None,
                    },
                )?;

                // If that fails, try to find by id
                if entry_details.is_empty() {
                    if let Ok(entry_id) = EntryId::from_str(name_or_id.as_str()) {
                        entry_details = connection.find_entries(
                            py,
                            EntryFilter {
                                id: Some(entry_id.into()),
                                name: None,
                                entry_kind: None,
                            },
                        )?;
                    }
                }

                if entry_details.is_empty() {
                    return Err(PyLookupError::new_err(format!(
                        "No entry found with name or id {name_or_id:?}"
                    )));
                }

                Py::new(
                    py,
                    PyEntryId {
                        id: entry_details[0].id,
                    },
                )
            }
            Self::Id(id) => Ok(id),
        }
    }
}
