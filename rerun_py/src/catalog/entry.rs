use std::str::FromStr as _;

use pyo3::{exceptions::PyTypeError, pyclass, pymethods, Py, PyErr, PyResult, Python};

use re_protos::catalog::v1alpha1::{EntryDetails, EntryType};
use re_tuid::Tuid;

use crate::catalog::PyCatalogClient;

/// A unique identifier for an entry in the catalog.
#[pyclass(name = "EntryId")]
#[derive(Clone)]
pub struct PyEntryId {
    pub id: Tuid,
}

//TODO(ab): __str__ + properties

#[pymethods]
impl PyEntryId {
    #[new]
    pub fn new(id: String) -> PyResult<Self> {
        Ok(Self {
            id: Tuid::from_str(id.as_str())
                .map_err(|err| PyTypeError::new_err(format!("invalid Tuid: {err}")))?,
        })
    }

    pub fn __str__(&self) -> String {
        self.id.to_string()
    }
}

impl From<Tuid> for PyEntryId {
    fn from(id: Tuid) -> Self {
        Self { id }
    }
}

impl From<re_protos::common::v1alpha1::Tuid> for PyEntryId {
    fn from(id: re_protos::common::v1alpha1::Tuid) -> Self {
        Self { id: id.into() }
    }
}

// ---

#[pyclass(name = "EntryType", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyEntryType {
    #[pyo3(name = "DATASET")]
    Dataset = 1,
    #[pyo3(name = "DATASET_VIEW")]
    DatasetView = 2,
    #[pyo3(name = "TABLE")]
    Table = 3,
    #[pyo3(name = "TABLE_VIEW")]
    TableView = 4,
}

#[pymethods]
impl PyEntryType {
    // This allows for EntryType.DATASET syntax in Python
    #[classattr]
    pub const DATASET: Self = Self::Dataset;
    #[classattr]
    pub const DATASET_VIEW: Self = Self::DatasetView;
    #[classattr]
    pub const TABLE: Self = Self::Table;
    #[classattr]
    pub const TABLE_VIEW: Self = Self::TableView;
}

impl TryFrom<EntryType> for PyEntryType {
    type Error = PyErr;

    fn try_from(value: EntryType) -> Result<Self, Self::Error> {
        match value {
            EntryType::Unspecified => Err(PyTypeError::new_err("EntryType is unspecified")),
            EntryType::Dataset => Ok(Self::Dataset),
            EntryType::DatasetView => Ok(Self::DatasetView),
            EntryType::Table => Ok(Self::Table),
            EntryType::TableView => Ok(Self::TableView),
        }
    }
}

// ---

#[pyclass(name = "Entry", subclass)]
pub struct PyEntry {
    pub client: Py<PyCatalogClient>,

    pub id: Py<PyEntryId>,

    pub details: EntryDetails,
}

#[pymethods]
impl PyEntry {
    #[getter]
    pub fn id(&self, py: Python<'_>) -> Py<PyEntryId> {
        self.id.clone_ref(py)
    }

    #[getter]
    pub fn name(&self) -> Option<String> {
        self.details.name.clone()
    }

    #[getter]
    pub fn catalog(&self, py: Python<'_>) -> Py<PyCatalogClient> {
        self.client.clone_ref(py)
    }

    #[getter]
    pub fn entry_type(&self) -> PyResult<PyEntryType> {
        EntryType::try_from(self.details.entry_type)
            .map_err(|err| PyTypeError::new_err(format!("cannot deserialize EntryType: {err}")))?
            .try_into()
    }

    #[getter]
    pub fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.details
            .created_at
            .and_then(|t| chrono::DateTime::from_timestamp(t.seconds, t.nanos as u32))
    }

    #[getter]
    pub fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.details
            .updated_at
            .and_then(|t| chrono::DateTime::from_timestamp(t.seconds, t.nanos as u32))
    }

    //TODO(ab): delete() (need delete entry grpc)
}
