use pyo3::exceptions::PyRuntimeError;
use pyo3::{pyclass, pymethods, Py, PyResult, Python};

use re_protos::catalog::v1alpha1::{DatasetEntry, EntryDetails, EntryFilter};

use crate::catalog::{CatalogConnectionHandle, PyEntry, PyEntryId};

/// A connection to a remote storage node.
#[pyclass(name = "CatalogClient")]
pub struct PyCatalogClient {
    #[expect(dead_code)]
    origin: re_uri::Origin,

    connection: CatalogConnectionHandle,
}

#[pymethods]
impl PyCatalogClient {
    #[new]
    fn new(addr: String) -> PyResult<Self> {
        let origin = re_uri::Origin::try_from(addr.as_str())
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        Ok(Self {
            origin: origin.clone(),
            connection: CatalogConnectionHandle::new(origin)?,
        })
    }

    // TODO: Create and return entry objects
    fn list_entries(slf: Py<Self>, py: Python<'_>) -> PyResult<Vec<Py<PyEntry>>> {
        let mut slf_mut_guard = slf.borrow_mut(py);
        let entry_details = slf_mut_guard.connection.find_entries(EntryFilter {
            id: None,
            name: None,
            entry_type: None,
        })?;
        let connection = slf_mut_guard.connection.clone();

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
                        connection: connection.clone(),
                        catalog: slf.clone_ref(py),
                        id: entry_id,
                        details: entry,
                    },
                )
            })
            .collect()
    }

    fn create_dataset(&mut self, name: &str) -> PyResult<PyEntryId> {
        let response = self.connection.create_dataset(DatasetEntry {
            details: Some(EntryDetails {
                name: Some(name.to_owned()),
                ..Default::default()
            }),
            dataset_handle: None,
        })?;

        //TODO(ab): proper error management + wrapping in helper objects
        response
            .details
            .ok_or(PyRuntimeError::new_err("No details in response"))?
            .id
            .ok_or(PyRuntimeError::new_err("No id in details"))
            .map(|id| PyEntryId { id: id.into() })
    }

    // TODO: Create and return entry objects
    fn delete_dataset(&mut self, id: PyEntryId) -> PyResult<()> {
        self.connection.delete_dataset(id)
    }
}
