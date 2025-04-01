use pyo3::{exceptions::PyLookupError, pyclass, pymethods, FromPyObject, Py, PyResult, Python};

use re_protos::catalog::v1alpha1::EntryFilter;

use crate::catalog::{to_py_err, ConnectionHandle, PyDataset, PyEntry, PyEntryId};

use super::table::PyTable;

/// A connection to a remote storage node.
#[pyclass(name = "CatalogClient")]
pub struct PyCatalogClient {
    #[expect(dead_code)]
    origin: re_uri::Origin,

    connection: ConnectionHandle,
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
        let origin = re_uri::Origin::try_from(addr.as_str()).map_err(to_py_err)?;

        let connection = ConnectionHandle::new(py, origin.clone())?;

        Ok(Self { origin, connection })
    }

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

    fn get_dataset(self_: Py<Self>, id: EntryIdLike, py: Python<'_>) -> PyResult<Py<PyDataset>> {
        let mut connection = self_.borrow(py).connection.clone();

        let id = id.resolve(&mut connection, py)?;

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

    fn get_table(self_: Py<Self>, id: EntryIdLike, py: Python<'_>) -> PyResult<Py<PyTable>> {
        let mut connection = self_.borrow(py).connection.clone();

        let id = id.resolve(&mut connection, py)?;

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
}

/// A type alias for a vector (vector search input data).
#[derive(FromPyObject)]
enum EntryIdLike {
    Str(String),
    Id(Py<PyEntryId>),
}

impl EntryIdLike {
    fn resolve(self, connection: &mut ConnectionHandle, py: Python<'_>) -> PyResult<Py<PyEntryId>> {
        match self {
            Self::Str(name) => {
                let entry_details = connection.find_entries(
                    py,
                    EntryFilter {
                        id: None,
                        name: Some(name),
                        entry_kind: None,
                    },
                )?;

                if entry_details.is_empty() {
                    return Err(PyLookupError::new_err("No entry found"));
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
