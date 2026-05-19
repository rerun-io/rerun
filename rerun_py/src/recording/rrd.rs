use std::collections::BTreeMap;
use std::sync::Arc;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::{PyResult, pyclass, pyfunction, pymethods};

use re_chunk::Chunk;
use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle};
use re_log_types::{LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreSource};

use crate::catalog::PySchemaInternal;
use crate::chunk::PyChunkIterator;

/// An archive loaded from an RRD.
///
/// RRD archives may include 1 or more recordings or blueprints.
#[pyclass(
    frozen,
    name = "RRDArchiveInternal",
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone)]
pub struct PyRRDArchiveInternal {
    pub datasets: BTreeMap<StoreId, (ChunkStoreHandle, Option<StoreInfo>)>,
}

#[pymethods]
impl PyRRDArchiveInternal {
    /// The number of recordings in the archive.
    fn num_recordings(&self) -> usize {
        self.datasets
            .iter()
            .filter(|(id, _)| id.is_recording())
            .count()
    }

    /// All the recordings in the archive.
    // TODO(jleibs): This should return an iterator
    fn all_recordings(&self) -> Vec<PyRecordingInternal> {
        self.datasets
            .iter()
            .filter(|(id, _)| id.is_recording())
            .map(|(_, (store, store_info))| PyRecordingInternal {
                store: store.clone(),
                store_info: store_info.clone(),
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
#[pyclass(name = "RecordingInternal", module = "rerun_bindings.rerun_bindings")]
pub struct PyRecordingInternal {
    pub(crate) store: ChunkStoreHandle,
    pub(crate) store_info: Option<StoreInfo>,
}

#[pymethods]
impl PyRecordingInternal {
    /// The schema describing all the columns available in the recording.
    fn schema(&self) -> PySchemaInternal {
        PySchemaInternal {
            columns: self.store.read().schema().chunk_column_descriptors().into(),
            metadata: Default::default(),
        }
    }

    /// The recording ID of the recording.
    fn recording_id(&self) -> String {
        self.store.read().id().recording_id().to_string()
    }

    /// The application ID of the recording.
    fn application_id(&self) -> String {
        self.store.read().id().application_id().to_string()
    }

    /// Iterate over all physical chunks in this recording.
    fn chunks(&self) -> PyChunkIterator {
        // TODO(RR-4126): this should eventually become a streaming iterator which loads the chunk
        // as it is iterated.
        let chunks: Vec<_> = self.store.read().iter_physical_chunks().cloned().collect();
        PyChunkIterator::new(chunks)
    }

    /// Save this recording to an RRD file.
    #[expect(clippy::needless_pass_by_value)]
    fn save(&self, path: std::path::PathBuf) -> PyResult<()> {
        let store = self.store.read();
        let store_id = store.id().clone();

        let info = self.store_info.clone().unwrap_or_else(|| {
            StoreInfo::new(
                store_id.clone(),
                StoreSource::Other("rerun-sdk-python".into()),
            )
        });

        let file =
            std::fs::File::create(&path).map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        let mut encoder = re_log_encoding::Encoder::new_eager(
            re_build_info::CrateVersion::LOCAL,
            re_log_encoding::EncodingOptions::PROTOBUF_COMPRESSED,
            file,
        )
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        encoder
            .append(&LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: re_tuid::Tuid::new(),
                info,
            }))
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        for chunk in store.iter_physical_chunks() {
            let arrow_msg = chunk
                .to_arrow_msg()
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
            encoder
                .append(&LogMsg::ArrowMsg(store_id.clone(), arrow_msg))
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
        }

        encoder
            .finish()
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        Ok(())
    }
}

/// Load a single recording from an RRD file.
#[pyfunction]
pub fn load_recording(path_to_rrd: std::path::PathBuf) -> PyResult<PyRecordingInternal> {
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
#[pyfunction]
#[expect(clippy::needless_pass_by_value)]
pub fn load_archive(path_to_rrd: std::path::PathBuf) -> PyResult<PyRRDArchiveInternal> {
    let rrd_file = std::fs::File::open(&path_to_rrd)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
    let decoder = re_log_encoding::Decoder::decode_eager(std::io::BufReader::new(rrd_file))
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

    let mut stores: BTreeMap<StoreId, ChunkStore> = BTreeMap::new();
    let mut store_infos: BTreeMap<StoreId, StoreInfo> = BTreeMap::new();

    for msg_result in decoder {
        let msg = msg_result.map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
        match msg {
            LogMsg::SetStoreInfo(set_store_info) => {
                let info = set_store_info.info;
                stores.entry(info.store_id.clone()).or_insert_with(|| {
                    ChunkStore::new(info.store_id.clone(), ChunkStoreConfig::DEFAULT)
                });
                store_infos.insert(info.store_id.clone(), info);
            }
            LogMsg::ArrowMsg(store_id, arrow_msg) => {
                let chunk = Chunk::from_arrow_msg(&arrow_msg)
                    .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
                if let Some(store) = stores.get_mut(&store_id) {
                    store
                        .insert_chunk(&Arc::new(chunk))
                        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
                }
            }
            LogMsg::BlueprintActivationCommand(_) => {}
        }
    }

    let datasets = stores
        .into_iter()
        .map(|(store_id, store)| {
            let info = store_infos.remove(&store_id);
            (store_id, (ChunkStoreHandle::new(store), info))
        })
        .collect();

    Ok(PyRRDArchiveInternal { datasets })
}
