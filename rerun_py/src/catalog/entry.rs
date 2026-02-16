use std::str::FromStr as _;

use pyo3::exceptions::PyTypeError;
use pyo3::{Py, PyErr, PyResult, Python, pyclass, pymethods};
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::EntryKind;
use re_protos::cloud::v1alpha1::ext::EntryDetails;

use crate::catalog::PyCatalogClientInternal;

/// A unique identifier for an entry in the catalog.
#[pyclass(eq, name = "EntryId", module = "rerun_bindings.rerun_bindings")]
#[derive(Clone, PartialEq, Eq)]
pub struct PyEntryId {
    pub id: EntryId,
}

#[pymethods]
impl PyEntryId {
    /// Create a new `EntryId` from a string.
    #[new]
    #[pyo3(text_signature = "(self, id)")]
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

    /// Return the raw 16-byte representation.
    pub fn as_bytes<'py>(&self, py: Python<'py>) -> pyo3::Bound<'py, pyo3::types::PyBytes> {
        pyo3::types::PyBytes::new(py, &self.id.id.as_bytes())
    }
}

impl From<EntryId> for PyEntryId {
    fn from(id: EntryId) -> Self {
        Self { id }
    }
}

// ---

/// The kinds of entries that can be stored in the catalog.
#[pyclass(
    name = "EntryKind",
    eq,
    eq_int,
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum_macros::EnumIter)]
pub enum PyEntryKind {
    #[pyo3(name = "DATASET")]
    Dataset = 1,

    #[pyo3(name = "DATASET_VIEW")]
    DatasetView = 2,

    #[pyo3(name = "TABLE")]
    Table = 3,

    #[pyo3(name = "TABLE_VIEW")]
    TableView = 4,

    #[pyo3(name = "BLUEPRINT_DATASET")]
    BlueprintDataset = 5,
}

// Enums don't need str
#[pymethods] // NOLINT: ignore[py-mthd-str]
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
            EntryKind::BlueprintDataset => Ok(Self::BlueprintDataset),
        }
    }
}

impl From<PyEntryKind> for EntryKind {
    fn from(value: PyEntryKind) -> Self {
        match value {
            PyEntryKind::Dataset => Self::Dataset,
            PyEntryKind::DatasetView => Self::DatasetView,
            PyEntryKind::Table => Self::Table,
            PyEntryKind::TableView => Self::TableView,
            PyEntryKind::BlueprintDataset => Self::BlueprintDataset,
        }
    }
}

// ---

#[pyclass( // NOLINT: ignore[py-cls-eq] internal object, __eq__ not needed
    name = "EntryDetailsInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyEntryDetails(pub EntryDetails);

#[pymethods] // NOLINT: ignore[py-mthd-str] internal object, __str__ not needed
impl PyEntryDetails {
    #[getter]
    fn id(&self) -> PyEntryId {
        self.0.id.into()
    }

    #[getter]
    fn name(&self) -> &str {
        &self.0.name
    }

    /// The entry's kind.
    #[getter]
    pub fn kind(&self) -> PyResult<PyEntryKind> {
        self.0.kind.try_into()
    }

    /// The entry's creation date and time.
    #[getter]
    //TODO(ab): use jiff when updating to pyo3 0.24.0
    pub fn created_at(&self) -> chrono::DateTime<chrono::Utc> {
        let ts = self.0.created_at;
        // If the `prost::Timestamp` was legal, then this is also legal.
        #[expect(clippy::unwrap_used)]
        chrono::DateTime::from_timestamp(ts.as_second(), ts.subsec_nanosecond() as u32).unwrap()
    }

    /// The entry's last updated date and time.
    #[getter]
    //TODO(ab): use jiff when updating to pyo3 0.24.0
    pub fn updated_at(&self) -> chrono::DateTime<chrono::Utc> {
        let ts = self.0.updated_at;
        // If the `prost::Timestamp` was legal, then this is also legal.
        #[expect(clippy::unwrap_used)]
        chrono::DateTime::from_timestamp(ts.as_second(), ts.subsec_nanosecond() as u32).unwrap()
    }
}

// ---

pub fn set_entry_name(
    py: Python<'_>,
    name: String,
    entry_details: &mut EntryDetails,
    client: &Py<PyCatalogClientInternal>,
) -> PyResult<()> {
    let entry_id = entry_details.id;
    let connection = client.borrow_mut(py).connection().clone();

    let entry_details_update =
        re_protos::cloud::v1alpha1::ext::EntryDetailsUpdate { name: Some(name) };

    let updated_entry_details = connection.update_entry(py, entry_id, entry_details_update)?;
    *entry_details = updated_entry_details;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_entry_kind_and_py_entry_kind_have_same_representation() {
        use strum::IntoEnumIterator as _;

        for kind in PyEntryKind::iter() {
            let entry_kind: EntryKind = kind.into();
            assert_eq!(
                kind as i32, entry_kind as i32,
                "Mismatched numerical representation for kind: {kind:?}",
            );
        }
    }
}
