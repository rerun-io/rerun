use pyo3::exceptions::PyRuntimeError;
use pyo3::{pyclass, pymethods, Py, PyResult, Python};

use re_protos::catalog::v1alpha1::{DatasetEntry, EntryDetails, EntryFilter};

use crate::catalog::{ConnectionHandle, PyDataset, PyEntry, PyEntryId};

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
    #[new]
    fn new(addr: String) -> PyResult<Self> {
        let origin = re_uri::Origin::try_from(addr.as_str())
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        let connection = ConnectionHandle::new(origin.clone(), runtime)?;

        Ok(Self { origin, connection })
    }

    fn entries(self_: Py<Self>, py: Python<'_>) -> PyResult<Vec<Py<PyEntry>>> {
        let mut connection = self_.borrow(py).connection.clone();

        let entry_details = py.allow_threads(|| {
            connection.find_entries(EntryFilter {
                id: None,
                name: None,
                entry_type: None,
            })
        })?;

        // Generate entry objects.
        entry_details
            .into_iter()
            .map(|entry| {
                let entry_id = Py::new(
                    py,
                    entry
                        .id
                        .ok_or(PyRuntimeError::new_err("No id in entry"))
                        .map(PyEntryId::from)?,
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

        let dataset_entry = py.allow_threads(|| connection.read_dataset(entry_id))?;

        let details = dataset_entry
            .details
            .ok_or(PyRuntimeError::new_err("No details in entry"))?;

        let dataset_handle = dataset_entry
            .dataset_handle
            .ok_or(PyRuntimeError::new_err("No dataset handle in entry"))?;

        let entry = PyEntry {
            client,
            id,
            details,
        };

        let dataset = PyDataset { dataset_handle };

        Py::new(py, (dataset, entry))
    }

    //TODO: `datasets` (needs FindDatasetsEntry)

    fn create_dataset(self_: Py<Self>, py: Python<'_>, name: &str) -> PyResult<Py<PyDataset>> {
        let mut connection = self_.borrow_mut(py).connection.clone();

        let response = py.allow_threads(|| {
            connection.create_dataset(DatasetEntry {
                details: Some(EntryDetails {
                    name: Some(name.to_owned()),
                    ..Default::default()
                }),
                dataset_handle: None,
            })
        })?;

        //TODO(ab): proper error management + wrapping in helper objects
        let entry_details = response
            .details
            .ok_or(PyRuntimeError::new_err("No details in response"))?;

        let dataset_handle = response
            .dataset_handle
            .ok_or(PyRuntimeError::new_err("No dataset handle in response"))?;

        Python::with_gil(|py| {
            let entry_id = Py::new(
                py,
                entry_details
                    .id
                    .ok_or(PyRuntimeError::new_err("No id in entry"))
                    .map(PyEntryId::from)?,
            )?;

            let entry = PyEntry {
                client: self_.clone_ref(py),
                id: entry_id,
                details: entry_details,
            };

            let dataset = PyDataset { dataset_handle };

            Py::new(py, (dataset, entry))
        })
    }

    //TODO: dataset from url
}
