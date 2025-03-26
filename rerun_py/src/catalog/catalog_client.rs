use pyo3::exceptions::PyRuntimeError;
use pyo3::{pyclass, pymethods, Py, PyResult, Python};

use re_protos::catalog::v1alpha1::{DatasetEntry, EntryDetails, EntryFilter};

use crate::catalog::{
    to_py_err, ConnectionHandle, MissingGrpcFieldError, PyDataset, PyEntry, PyEntryId,
};

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
                entry_type: None,
            },
        )?;

        // Generate entry objects.
        entry_details
            .into_iter()
            .map(|entry| {
                let entry_id = Py::new(
                    py,
                    entry
                        .id
                        .ok_or(PyRuntimeError::new_err("No id in entry"))
                        .and_then(PyEntryId::try_from)?,
                )?;

                Py::new(
                    py,
                    PyEntry {
                        client: self_.clone_ref(py),
                        id: entry_id,
                        details: entry,
                    },
                )
            })
            .collect()
    }

    fn get_dataset(self_: Py<Self>, py: Python<'_>, id: Py<PyEntryId>) -> PyResult<Py<PyDataset>> {
        let mut connection = self_.borrow(py).connection.clone();
        let entry_id = id.borrow(py).id;
        let client = self_.clone_ref(py);

        let dataset_entry = connection.read_dataset(py, entry_id)?;

        let details = dataset_entry
            .details
            .ok_or(MissingGrpcFieldError::new_err("No details in entry"))?;

        let dataset_handle = dataset_entry
            .dataset_handle
            .ok_or(MissingGrpcFieldError::new_err("No dataset handle in entry"))?;

        let entry = PyEntry {
            client,
            id,
            details,
        };

        let dataset = PyDataset { dataset_handle };

        Py::new(py, (dataset, entry))
    }

    //TODO(#9369): `datasets()` (needs FindDatasetsEntries rpc)

    fn create_dataset(self_: Py<Self>, py: Python<'_>, name: &str) -> PyResult<Py<PyDataset>> {
        let mut connection = self_.borrow_mut(py).connection.clone();

        let response = connection.create_dataset(
            py,
            DatasetEntry {
                details: Some(EntryDetails {
                    name: Some(name.to_owned()),
                    ..Default::default()
                }),
                dataset_handle: None,
            },
        )?;

        //TODO(ab): proper error management + wrapping in helper objects
        let entry_details = response
            .details
            .ok_or(MissingGrpcFieldError::new_err("No details in response"))?;

        let dataset_handle = response
            .dataset_handle
            .ok_or(MissingGrpcFieldError::new_err(
                "No dataset handle in response",
            ))?;

        let entry_id = Py::new(
            py,
            entry_details
                .id
                .ok_or(MissingGrpcFieldError::new_err("No id in entry"))
                .and_then(PyEntryId::try_from)?,
        )?;

        let entry = PyEntry {
            client: self_.clone_ref(py),
            id: entry_id,
            details: entry_details,
        };

        let dataset = PyDataset { dataset_handle };

        Py::new(py, (dataset, entry))
    }

    //TODO(#9360): `dataset_from_url()`
}
