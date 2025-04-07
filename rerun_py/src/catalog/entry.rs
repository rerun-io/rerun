use std::str::FromStr as _;

use pyo3::{exceptions::PyTypeError, pyclass, pymethods, Py, PyErr, PyResult, PyObject, Python, types::{PyModule, PyAnyMethods}};

use re_protos::catalog::v1alpha1::{ext::EntryDetails, EntryKind};
use re_protos::common::v1alpha1::ext::EntryId;

use crate::catalog::PyCatalogClient;

/// A unique identifier for an entry in the catalog.
#[pyclass(name = "EntryId")]
#[derive(Clone)]
pub struct PyEntryId {
    pub id: EntryId,
}

#[pymethods]
impl PyEntryId {
    #[new]
    pub fn new(id: String) -> PyResult<Self> {
        Ok(Self {
            id: re_tuid::Tuid::from_str(id.as_str())
                .map_err(|err| PyTypeError::new_err(format!("invalid Tuid: {err}")))?
                .into(),
        })
    }

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
    pub fn name(&self) -> String {
        self.details.name.clone()
    }

    #[getter]
    pub fn catalog(&self, py: Python<'_>) -> Py<PyCatalogClient> {
        self.client.clone_ref(py)
    }

    #[getter]
    pub fn kind(&self) -> PyResult<PyEntryKind> {
        self.details.kind.try_into()
    }

    #[getter]
    pub fn created_at(&self, py: Python<'_>) -> PyResult<PyObject> {
        let ts = self.details.created_at;
        // If the `prost::Timestamp` was legal, then this is also legal.
        #[allow(clippy::unwrap_used)]
        let timestamp = jiff::Timestamp::new(ts.as_second(), ts.subsec_nanosecond()).unwrap();

        // Get the Python timestamp value (seconds + fractional part)
        let unix_timestamp = timestamp.as_second() as f64
                           + timestamp.subsec_nanosecond() as f64 * 1e-9;

        // Create the UTC timezone object
        let datetime_module = PyModule::import(py, "datetime")?;
        let timezone = datetime_module.getattr("timezone")?;
        let utc = timezone.getattr("utc")?;

        // Create the datetime object with the UTC timezone
        let datetime_class = datetime_module.getattr("datetime")?;
        let datetime = datetime_class.call_method1("fromtimestamp", (unix_timestamp, utc))?;

        Ok(datetime.into())
    }

    #[getter]
    pub fn updated_at(&self, py: Python<'_>) -> PyResult<PyObject> {
        let ts = self.details.updated_at;
        // If the `prost::Timestamp` was legal, then this is also legal.
        #[allow(clippy::unwrap_used)]
        let timestamp = jiff::Timestamp::new(ts.as_second(), ts.subsec_nanosecond()).unwrap();

        // Get the Python timestamp value (seconds + fractional part)
        let unix_timestamp = timestamp.as_second() as f64
                           + timestamp.subsec_nanosecond() as f64 * 1e-9;

        // Create the UTC timezone object
        let datetime_module = PyModule::import(py, "datetime")?;
        let timezone = datetime_module.getattr("timezone")?;
        let utc = timezone.getattr("utc")?;

        // Create the datetime object with the UTC timezone
        let datetime_class = datetime_module.getattr("datetime")?;
        let datetime = datetime_class.call_method1("fromtimestamp", (unix_timestamp, utc))?;

        Ok(datetime.into())
    }

    // ---

    fn delete(&mut self, py: Python<'_>) -> PyResult<()> {
        let entry_id = self.id.borrow(py).id;
        let mut connection = self.client.borrow_mut(py).connection().clone();

        connection.delete_entry(py, entry_id)
    }
}

