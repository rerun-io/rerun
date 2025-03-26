use std::str::FromStr as _;

use pyo3::{exceptions::PyTypeError, pyclass, pymethods, Py, PyErr, PyResult, Python};

use re_protos::catalog::v1alpha1::{EntryDetails, EntryKind};
use re_tuid::Tuid;

use crate::catalog::PyCatalogClient;

/// A unique identifier for an entry in the catalog.
#[pyclass(name = "EntryId")]
#[derive(Clone)]
pub struct PyEntryId {
    pub id: Tuid,
}

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

impl TryFrom<re_protos::common::v1alpha1::Tuid> for PyEntryId {
    type Error = pyo3::PyErr;

    fn try_from(value: re_protos::common::v1alpha1::Tuid) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value
                .try_into()
                .map_err(|err| PyTypeError::new_err(format!("invalid Tuid: {err}")))?,
        })
    }
}

// ---

#[pyclass(name = "EntryType", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyEntryKind {
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
impl PyEntryKind {
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

impl TryFrom<EntryKind> for PyEntryKind {
    type Error = PyErr;

    fn try_from(value: EntryKind) -> Result<Self, Self::Error> {
        match value {
            EntryKind::Unspecified => Err(PyTypeError::new_err("EntryType is unspecified")),
            EntryKind::Dataset => Ok(Self::Dataset),
            EntryKind::DatasetView => Ok(Self::DatasetView),
            EntryKind::Table => Ok(Self::Table),
            EntryKind::TableView => Ok(Self::TableView),
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
    pub fn kind(&self) -> PyResult<PyEntryKind> {
        //TODO(ab): make this less annoying thanks to a wrapper over grpc messages.
        EntryKind::try_from(self.details.entry_kind)
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

    // ---

    fn delete(&mut self, py: Python<'_>) -> PyResult<()> {
        let entry_id = self.id.borrow(py).id;
        let mut connection = self.client.borrow_mut(py).connection().clone();

        connection.delete_entry(py, entry_id)
    }
}
