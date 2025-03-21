use pyo3::exceptions::PyRuntimeError;
use pyo3::types::PyList;
use pyo3::{pyclass, pymethods, Py, PyResult, Python};
use re_protos::catalog::v1alpha1::{DatasetEntry, EntryDetails, EntryFilter, EntryType};

use crate::catalog::{CatalogConnectionHandle, PyDataset, PyEntry, PyEntryId};

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

    fn entries(slf: Py<Self>, py: Python<'_>) -> PyResult<Vec<Py<PyEntry>>> {
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

    fn get_dataset(slf: Py<Self>, py: Python<'_>, id: Py<PyEntryId>) -> PyResult<Py<PyDataset>> {
        let mut slf_mut_guard = slf.borrow_mut(py);
        let connection = slf_mut_guard.connection.clone();
        let entry_details = slf_mut_guard.connection.find_entries(EntryFilter {
            id: Some(id.borrow(py).id.into()),
            name: None,
            entry_type: Some(EntryType::Dataset as i32),
        })?;

        //TODO(ab): check len == 1?

        let entry_details = entry_details
            .first()
            .ok_or(PyRuntimeError::new_err("No entry found"))?;

        let entry = PyEntry {
            connection: connection.clone(),
            catalog: slf.clone_ref(py),
            id,
            details: entry_details.clone(),
        };

        let dataset = PyDataset {
            connection: connection.clone(),
            // dataset_handle: entry_details
            //     .dataset_handle
            //     .ok_or(PyRuntimeError::new_err("No dataset handle in entry"))?,
        };

        Py::new(py, (dataset, entry))
    }

    // TODO(ab): this requires a `FindDatasetEntries` endpoint
    fn datasets(slf: Py<Self>, py: Python<'_>) -> PyResult<Py<PyList>> {
        let mut slf_mut_guard = slf.borrow_mut(py);
        let entry_details = slf_mut_guard.connection.find_entries(EntryFilter {
            id: None,
            name: None,
            entry_type: Some(EntryType::Dataset as i32),
        })?;
        let connection = slf_mut_guard.connection.clone();

        // Generate entry objects.
        let datasets = entry_details
            .into_iter()
            .map(|entry_details| {
                let entry_id = Py::new(
                    py,
                    entry_details
                        .id
                        .ok_or(PyRuntimeError::new_err("No id in entry"))
                        .map(PyEntryId::from)?,
                )?;

                let entry = PyEntry {
                    connection: connection.clone(),
                    catalog: slf.clone_ref(py),
                    id: entry_id,
                    details: entry_details,
                };

                let dataset = PyDataset {
                    connection: connection.clone(),
                    // dataset_handle: entry_details
                    //     .dataset_handle
                    //     .ok_or(PyRuntimeError::new_err("No dataset handle in entry"))?,
                };

                Py::new(py, (dataset, entry))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(PyList::new(py, datasets)?.into())
    }

    fn create_dataset(self_: Py<Self>, py: Python<'_>, name: &str) -> PyResult<Py<PyDataset>> {
        let mut self_mut = self_.borrow_mut(py);
        let response = self_mut.connection.create_dataset(DatasetEntry {
            details: Some(EntryDetails {
                name: Some(name.to_owned()),
                ..Default::default()
            }),
            dataset_handle: None,
        })?;
        let connection = self_mut.connection.clone();

        //TODO(ab): proper error management + wrapping in helper objects
        let entry_details = response
            .details
            .ok_or(PyRuntimeError::new_err("No details in response"))?;

        let entry_id = Py::new(
            py,
            entry_details
                .id
                .ok_or(PyRuntimeError::new_err("No id in entry"))
                .map(PyEntryId::from)?,
        )?;

        let entry = PyEntry {
            connection: connection.clone(),
            catalog: self_.clone_ref(py),
            id: entry_id,
            details: entry_details,
        };

        let dataset = PyDataset {
            connection,
            // dataset_handle: entry_details
            //     .dataset_handle
            //     .ok_or(PyRuntimeError::new_err("No dataset handle in entry"))?,
        };

        Ok(Py::new(py, (dataset, entry))?)
    }

    //TODO: dataset from url
}
