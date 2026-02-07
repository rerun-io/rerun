use std::collections::BTreeMap;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::PyAnyMethods as _;
use pyo3::types::PyModule;
use pyo3::{PyObject, PyResult, Python, pyclass, pyfunction, pymethods};

use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle};
use re_log_types::StoreId;

use crate::catalog::PySchemaInternal;

/// An archive loaded from an RRD.
///
/// RRD archives may include 1 or more recordings or blueprints.
#[pyclass(frozen, name = "RRDArchive", module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq] non-trivial implementation
#[derive(Clone)]
pub struct PyRRDArchive {
    pub datasets: BTreeMap<StoreId, ChunkStoreHandle>,
}

#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PyRRDArchive {
    /// The number of recordings in the archive.
    fn num_recordings(&self) -> usize {
        self.datasets
            .iter()
            .filter(|(id, _)| id.is_recording())
            .count()
    }

    /// All the recordings in the archive.
    // TODO(jleibs): This should return an iterator
    fn all_recordings(&self) -> Vec<PyRecording> {
        self.datasets
            .iter()
            .filter(|(id, _)| id.is_recording())
            .map(|(_, store)| PyRecording {
                store: store.clone(),
            })
            .collect()
    }
}

/// A single Rerun recording.
///
/// This can be loaded from an RRD file using [`load_recording()`][rerun.recording.load_recording].
///
/// A recording is a collection of data that was logged to Rerun. This data is organized
/// as a column for each index (timeline) and each entity/component pair that was logged.
///
/// You can examine the [`.schema()`][rerun.recording.Recording.schema] of the recording to see
/// what data is available.
#[pyclass(name = "Recording", module = "rerun_bindings.rerun_bindings")] // NOLINT: ignore[py-cls-eq] non-trivial implementation
pub struct PyRecording {
    pub(crate) store: ChunkStoreHandle,
}

#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PyRecording {
    /// The schema describing all the columns available in the recording.
    fn schema(&self, py: Python<'_>) -> PyResult<PyObject> {
        let schema_internal = PySchemaInternal {
            columns: self.store.read().schema().into(),
            metadata: Default::default(),
        };

        // Import rerun.catalog.Schema and instantiate it with the internal schema
        let schema_class = PyModule::import(py, "rerun.catalog")?.getattr("Schema")?;
        let schema = schema_class.call1((schema_internal,))?;
        Ok(schema.into())
    }

    /// The recording ID of the recording.
    fn recording_id(&self) -> String {
        self.store.read().id().recording_id().to_string()
    }

    /// The application ID of the recording.
    fn application_id(&self) -> String {
        self.store.read().id().application_id().to_string()
    }
}

/// Load a single recording from an RRD file.
///
/// Will raise a `ValueError` if the file does not contain exactly one recording.
///
/// Parameters
/// ----------
/// path_to_rrd:
///     The path to the file to load.
///
/// Returns
/// -------
/// Recording
///     The loaded recording.
#[pyfunction]
pub fn load_recording(path_to_rrd: std::path::PathBuf) -> PyResult<PyRecording> {
    let archive = load_archive(path_to_rrd)?;

    let num_recordings = archive.num_recordings();

    if num_recordings != 1 {
        return Err(PyValueError::new_err(format!(
            "Expected exactly one recording in the archive, but found {num_recordings}",
        )));
    }

    if let Some(recording) = archive.all_recordings().into_iter().next() {
        Ok(recording)
    } else {
        Err(PyValueError::new_err(
            "Expected exactly one recording in the archive, but found none.",
        ))
    }
}

/// Load a rerun archive from an RRD file.
///
/// Parameters
/// ----------
/// path_to_rrd:
///     The path to the file to load.
///
/// Returns
/// -------
/// RRDArchive
///     The loaded archive.
#[pyfunction]
pub fn load_archive(path_to_rrd: std::path::PathBuf) -> PyResult<PyRRDArchive> {
    let stores = ChunkStore::from_rrd_filepath(&ChunkStoreConfig::DEFAULT, path_to_rrd)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
        .into_iter()
        .map(|(store_id, store)| (store_id, ChunkStoreHandle::new(store)))
        .collect();

    let archive = PyRRDArchive { datasets: stores };

    Ok(archive)
}
