use std::path::{Path, PathBuf};
use std::sync::Arc;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use re_chunk::Chunk;
use re_log_types::{LogMsg, StoreId, StoreInfo, StoreKind};

use super::engine::ChunkStream;
use super::error::ChunkPipelineError;
use super::py_stream::PyLazyChunkStreamInternal;
use super::stream::LazyChunkStream;

/// Internal RRD loader binding.
///
/// Opens an RRD file and extracts header metadata.
/// Each call to `stream()` produces an independent lazy chunk stream.
#[pyclass(
    frozen,
    name = "RrdLoaderInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyRrdLoaderInternal {
    path: PathBuf,
    application_id: Option<String>,
    recording_id: Option<String>,
}

#[pymethods]
impl PyRrdLoaderInternal {
    #[new]
    #[pyo3(text_signature = "(self, path)")]
    fn new(path: &str) -> PyResult<Self> {
        let path = PathBuf::from(path);

        if !path.exists() {
            return Err(PyValueError::new_err(format!(
                "RRD file not found: {}",
                path.display()
            )));
        }

        // Read the header to extract StoreInfo
        let store_info = read_rrd_store_info(&path)
            .map_err(|err| PyRuntimeError::new_err(format!("Failed to read RRD header: {err}")))?;

        let (application_id, recording_id) = if let Some(info) = store_info {
            (
                Some(info.application_id().as_str().to_owned()),
                Some(info.recording_id().as_str().to_owned()),
            )
        } else {
            (None, None)
        };

        Ok(Self {
            path,
            application_id,
            recording_id,
        })
    }

    /// Return a new lazy stream over all chunks in the RRD file.
    fn stream(&self) -> PyLazyChunkStreamInternal {
        PyLazyChunkStreamInternal::new(LazyChunkStream::from_rrd(self.path.clone()))
    }

    /// Application ID from the RRD's StoreInfo, if present.
    #[getter]
    fn application_id(&self) -> Option<&str> {
        self.application_id.as_deref()
    }

    /// Recording ID from the RRD's StoreInfo, if present.
    #[getter]
    fn recording_id(&self) -> Option<&str> {
        self.recording_id.as_deref()
    }
}

// ---------------------------------------------------------------------------
// RrdSource — pull-based chunk stream from an RRD file
// ---------------------------------------------------------------------------

/// Chunk stream that lazily decodes an RRD file.
///
/// Streams only the **first recording store** found in the file. Blueprint stores are silently
/// skipped (`info!`), and additional recording stores are skipped with a `warn!`.
///
/// TODO(RR-4263): make this more flexible.
///
/// Construction is fallible: I/O errors (missing file, permission denied) are
/// captured and surfaced on the first `next()` call rather than panicking.
pub struct RrdSource {
    inner: RrdSourceInner,
}

enum RrdSourceInner {
    /// Normal operation: lazily decode messages from the file.
    Live {
        decoder: Box<dyn Iterator<Item = Result<LogMsg, re_log_encoding::DecodeError>> + Send>,

        /// The `StoreId` of the first recording store we encounter. Only chunks
        /// belonging to this store are yielded; everything else is skipped.
        target_store_id: Option<StoreId>,
    },

    /// The file could not be opened. The error is yielded once, then the stream terminates.
    Failed(Option<ChunkPipelineError>),
}

impl RrdSource {
    pub fn new(path: &Path) -> Self {
        match std::fs::File::open(path) {
            Ok(file) => {
                let reader = std::io::BufReader::new(file);
                let decoder = re_log_encoding::Decoder::<LogMsg>::decode_lazy(reader);
                Self {
                    inner: RrdSourceInner::Live {
                        decoder: Box::new(decoder),
                        target_store_id: None,
                    },
                }
            }

            Err(err) => Self {
                inner: RrdSourceInner::Failed(Some(ChunkPipelineError::RrdRead {
                    reason: format!("{}: {err}", path.display()),
                })),
            },
        }
    }
}

impl ChunkStream for RrdSource {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        match &mut self.inner {
            RrdSourceInner::Failed(stored_err) => match stored_err.take() {
                Some(err) => Err(err),
                None => Ok(None),
            },

            RrdSourceInner::Live {
                decoder,
                target_store_id,
            } => loop {
                let Some(msg_result) = decoder.next() else {
                    return Ok(None);
                };

                let msg = msg_result.map_err(|err| ChunkPipelineError::RrdRead {
                    reason: err.to_string(),
                })?;

                match msg {
                    LogMsg::SetStoreInfo(set_store_info) => {
                        let info = &set_store_info.info;
                        if info.store_id.kind() == StoreKind::Recording {
                            if target_store_id.is_none() {
                                *target_store_id = Some(info.store_id.clone());
                            } else if target_store_id.as_ref() != Some(&info.store_id) {
                                re_log::warn!(
                                    "RRD contains multiple recording stores; \
                                     ignoring store {:?}",
                                    info.store_id,
                                );
                            }
                        } else {
                            re_log::info!("Skipping blueprint store {:?} in RRD", info.store_id,);
                        }
                    }

                    LogMsg::ArrowMsg(ref store_id, ref arrow_msg) => {
                        // Only yield chunks from the active recording.
                        let is_target_store = target_store_id
                            .as_ref()
                            .is_some_and(|active| active == store_id);

                        if is_target_store {
                            let chunk = Chunk::from_arrow_msg(arrow_msg).map_err(|err| {
                                ChunkPipelineError::RrdChunkDecode {
                                    reason: err.to_string(),
                                }
                            })?;

                            return Ok(Some(Arc::new(chunk)));
                        }

                        // Chunk belongs to a different store — skip it.
                    }

                    LogMsg::BlueprintActivationCommand(_) => {}
                }
            },
        }
    }
}

// ---------------------------------------------------------------------------
// RRD I/O helpers
// ---------------------------------------------------------------------------

/// Open an RRD file and extract the [`StoreInfo`] from the first recording store.
///
/// Blueprint stores are skipped. Returns `None` if no recording store is found
/// before the first `ArrowMsg` or end of file.
fn read_rrd_store_info(
    path: &Path,
) -> Result<Option<StoreInfo>, Box<dyn std::error::Error + Send + Sync>> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let decoder = re_log_encoding::Decoder::<LogMsg>::decode_lazy(reader);

    for msg_result in decoder {
        match msg_result {
            Ok(LogMsg::SetStoreInfo(set_store_info))
                if set_store_info.info.store_id.kind() == StoreKind::Recording =>
            {
                return Ok(Some(set_store_info.info));
            }

            Ok(LogMsg::ArrowMsg(..)) => {
                return Ok(None);
            }
            _ => {}
        }
    }

    Ok(None)
}
