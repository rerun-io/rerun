use std::collections::BTreeMap;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::{PyResult, pyclass, pyfunction, pymethods};

use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle};
use re_log_types::{StoreId, StoreKind};

use super::PyRecording;

/// An archive loaded from an RRD.
///
/// RRD archives may include 1 or more recordings or blueprints.
#[pyclass(frozen, name = "RRDArchive")]
#[derive(Clone)]
pub struct PyRRDArchive {
    pub datasets: BTreeMap<StoreId, ChunkStoreHandle>,
}

#[pymethods]
impl PyRRDArchive {
    /// The number of recordings in the archive.
    fn num_recordings(&self) -> usize {
        self.datasets
            .iter()
            .filter(|(id, _)| matches!(id.kind, StoreKind::Recording))
            .count()
    }

    /// All the recordings in the archive.
    // TODO(jleibs): This should return an iterator
    fn all_recordings(&self) -> Vec<PyRecording> {
        self.datasets
            .iter()
            .filter(|(id, _)| matches!(id.kind, StoreKind::Recording))
            .map(|(_, store)| {
                let cache = re_dataframe::QueryCacheHandle::new(re_dataframe::QueryCache::new(
                    store.clone(),
                ));
                PyRecording {
                    store: store.clone(),
                    cache,
                }
            })
            .collect()
    }
}

/// Load a single recording from an RRD file.
///
/// Will raise a `ValueError` if the file does not contain exactly one recording.
///
/// Parameters
/// ----------
/// path_to_rrd : str | os.PathLike[str]
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
/// path_to_rrd : str | os.PathLike[str]
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
