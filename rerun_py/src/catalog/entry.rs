use std::str::FromStr as _;

use pyo3::{Py, PyErr, PyResult, Python, exceptions::PyTypeError, pyclass, pymethods};

use re_log_types::EntryId;
use re_protos::catalog::v1alpha1::{EntryKind, ext::EntryDetails};

use crate::catalog::PyCatalogClient;

/// A unique identifier for an entry in the catalog.
#[pyclass(name = "EntryId")]
#[derive(Clone)]
pub struct PyEntryId {
    pub id: EntryId,
}

#[pymethods]
impl PyEntryId {
    /// Create a new `EntryId` from a string.
    #[new]
    pub fn new(id: String) -> PyResult<Self> {
        Ok(Self {
            id: re_tuid::Tuid::from_str(id.as_str())
                .map_err(|err| PyTypeError::new_err(format!("invalid Tuid: {err}")))?
                .into(),
        })
    }

    /// Entry id as a string.
    pub fn __str__(&self) -> String {
        self.id.to_string()
    }
}

impl From<EntryId> for PyEntryId {
    fn from(id: EntryId) -> Self {
        Self { id }
    }
}

// ---

/// The kinds of entries that can be stored in the catalog.
#[pyclass(name = "EntryKind", eq, eq_int)]
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

/// An entry in the catalog.
#[pyclass(name = "Entry", subclass)]
pub struct PyEntry {
    pub client: Py<PyCatalogClient>,

    pub id: Py<PyEntryId>,

    pub details: EntryDetails,
}

#[pymethods]
impl PyEntry {
    /// The entry's id.
    #[getter]
    pub fn id(&self, py: Python<'_>) -> Py<PyEntryId> {
        self.id.clone_ref(py)
    }

    /// The entry's name.
    #[getter]
    pub fn name(&self) -> String {
        self.details.name.clone()
    }

    /// The catalog client that this entry belongs to.
    #[getter]
    pub fn catalog(&self, py: Python<'_>) -> Py<PyCatalogClient> {
        self.client.clone_ref(py)
    }

    /// The entry's kind.
    #[getter]
    pub fn kind(&self) -> PyResult<PyEntryKind> {
        self.details.kind.try_into()
    }

    /// The entry's creation date and time.
    #[getter]
    //TODO(ab): use jiff when updating to pyo3 0.24.0
    pub fn created_at(&self) -> chrono::DateTime<chrono::Utc> {
        let ts = self.details.created_at;
        // If the `prost::Timestamp` was legal, then this is also legal.
        #[allow(clippy::unwrap_used)]
        chrono::DateTime::from_timestamp(ts.as_second(), ts.subsec_nanosecond() as u32).unwrap()
    }

    /// The entry's last updated date and time.
    #[getter]
    //TODO(ab): use jiff when updating to pyo3 0.24.0
    pub fn updated_at(&self) -> chrono::DateTime<chrono::Utc> {
        let ts = self.details.updated_at;
        // If the `prost::Timestamp` was legal, then this is also legal.
        #[allow(clippy::unwrap_used)]
        chrono::DateTime::from_timestamp(ts.as_second(), ts.subsec_nanosecond() as u32).unwrap()
    }

    // ---

    /// Delete this entry from the catalog.
    fn delete(&mut self, py: Python<'_>) -> PyResult<()> {
        let entry_id = self.id.borrow(py).id;
        let mut connection = self.client.borrow_mut(py).connection().clone();

        connection.delete_entry(py, entry_id)
    }
}
