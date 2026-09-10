use std::path::{Path, PathBuf};
use std::sync::Arc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use re_chunk::Chunk;
use re_chunk_store::LazyStore;
use re_log_encoding::{RawRrdManifest, RrdChunkProvider, RrdFooter};
use re_log_types::{LogMsg, StoreId, StoreKind};

use crate::utils::wait_for_future;

use super::error::ChunkPipelineError;
use super::lazy_store::PyLazyStoreInternal;
use super::py_stream::PyLazyChunkStreamInternal;
use super::stream::LazyChunkStream;
use super::{ChunkStream, ChunkStreamFactory};

/// Describes a single store found in an RRD file.
#[pyclass(
    frozen,
    from_py_object,
    name = "StoreEntryInternal",
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone)]
pub struct PyStoreEntryInternal {
    store_id: StoreId,
}

#[pymethods]
impl PyStoreEntryInternal {
    #[getter]
    fn kind(&self) -> &str {
        match self.store_id.kind() {
            StoreKind::Recording => "recording",
            StoreKind::Blueprint => "blueprint",
        }
    }

    #[getter]
    fn application_id(&self) -> &str {
        self.store_id.application_id().as_str()
    }

    #[getter]
    fn recording_id(&self) -> &str {
        self.store_id.recording_id().as_str()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.store_id == other.store_id
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash as _, Hasher as _};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.store_id.hash(&mut hasher);
        hasher.finish()
    }
}

/// Internal RRD reader binding.
///
/// Opens an RRD file and decodes its footer once; `store_entries()` and `store()` are served
/// from that cached footer. Each call to `stream()` produces an independent lazy chunk stream;
/// `store()` opens a specific store as a [`LazyStore`].
#[pyclass(
    frozen,
    name = "RrdReaderInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyRrdReaderInternal {
    path: PathBuf,

    /// Held open for the reader's lifetime so footer-backed reads survive the file being
    /// moved or deleted. Only used for positional reads, so it is safe to share.
    file: std::fs::File,

    /// `None` for legacy RRDs without a footer.
    footer: Option<RrdFooter>,

    /// Lazily populated on first `stores()` call.
    cached_stores: parking_lot::Mutex<Option<Vec<StoreId>>>,
}

#[pymethods]
impl PyRrdReaderInternal {
    #[new]
    #[pyo3(text_signature = "(self, path)")]
    fn new(py: Python<'_>, path: &str) -> PyResult<Self> {
        let path = PathBuf::from(path);

        if !path.exists() {
            return Err(PyValueError::new_err(format!(
                "RRD file not found: {}",
                path.display()
            )));
        }

        let file = std::fs::File::open(&path).map_err(|err| ChunkPipelineError::RrdRead {
            path: path.clone(),
            reason: err.to_string(),
        })?;

        // Reading the footer is cheap (3 seeks) and tells us whether this is a
        // legacy RRD that has no manifest. Without one, store enumeration falls
        // back to a whole-file frame scan and `store()` won't work at all,
        // so it's worth surfacing this up-front rather than at first use.
        let footer =
            wait_for_future(py, re_log_encoding::read_rrd_footer(&file)).map_err(|err| {
                ChunkPipelineError::RrdRead {
                    path: path.clone(),
                    reason: err.to_string(),
                }
            })?;
        if footer.is_none() {
            crate::utils::py_rerun_warn(&format!(
                "RRD file has no footer/manifest: {}. \
                 This is a legacy format; store enumeration will fall back to a \
                 whole-file scan, and `store()` is not supported (use `stream()` instead).",
                path.display()
            ))?;
        }

        Ok(Self {
            path,
            file,
            footer,
            cached_stores: parking_lot::Mutex::new(None),
        })
    }

    /// List all store entries in this RRD file.
    ///
    /// Lazily computed on first call, then cached.
    fn store_entries(&self, py: Python<'_>) -> PyResult<Vec<PyStoreEntryInternal>> {
        Ok(self
            .ensure_cached_stores(py)?
            .into_iter()
            .map(|store_id| PyStoreEntryInternal { store_id })
            .collect())
    }

    /// Return a new lazy stream over chunks from a specific store.
    ///
    /// If `store` is `None`, streams the first recording store found in this RRD. Errors
    /// if the file contains no recording stores.
    #[pyo3(signature = (store=None))]
    fn stream(
        &self,
        store: Option<&PyStoreEntryInternal>,
        py: Python<'_>,
    ) -> PyResult<PyLazyChunkStreamInternal> {
        let target = self.resolve_target(py, store)?;
        Ok(PyLazyChunkStreamInternal::new(
            LazyChunkStream::from_factory(RrdStreamFactory::new(self.path.clone(), target)),
        ))
    }

    /// Open a specific store as a [`LazyStore`]: read the manifest now, load chunks on demand.
    ///
    /// If `store` is `None`, opens the first recording store. Errors if the file contains
    /// no recording stores. Errors with `RrdNoManifest` for legacy RRDs that lack a
    /// footer/manifest — those must be materialized via `RrdReader.stream().collect()`.
    #[pyo3(signature = (store=None))]
    fn store(
        &self,
        store: Option<&PyStoreEntryInternal>,
        py: Python<'_>,
    ) -> PyResult<PyLazyStoreInternal> {
        let target_store_id = self.resolve_target(py, store)?;
        let footer = self
            .footer
            .as_ref()
            .ok_or_else(|| ChunkPipelineError::RrdNoManifest {
                path: self.path.clone(),
            })?;
        let raw = pick_manifest(footer, &self.path, &target_store_id)?;

        let reader = self
            .file
            .try_clone()
            .map_err(|err| ChunkPipelineError::RrdRead {
                path: self.path.clone(),
                reason: err.to_string(),
            })?;
        let provider = Arc::new(
            RrdChunkProvider::from_reader(reader, self.path.display().to_string(), Arc::new(raw))
                .map_err(|err| ChunkPipelineError::RrdRead {
                path: self.path.clone(),
                reason: format!("Invalid RRD manifest: {err}"),
            })?,
        );
        Ok(PyLazyStoreInternal::new(LazyStore::new(provider)))
    }

    /// The file path of the RRD file.
    #[getter]
    fn path(&self) -> PathBuf {
        self.path.clone()
    }
}

impl PyRrdReaderInternal {
    /// Populate the store cache on first call, then return a clone of the cached list.
    fn ensure_cached_stores(&self, py: Python<'_>) -> PyResult<Vec<StoreId>> {
        let mut cache = self.cached_stores.lock();
        if cache.is_none() {
            let stores = if let Some(footer) = &self.footer {
                let mut store_ids: Vec<StoreId> = footer.manifests.keys().cloned().collect();
                store_ids.sort();
                store_ids
            } else {
                wait_for_future(py, re_log_encoding::enumerate_rrd_stores(&self.file)).map_err(
                    |err| ChunkPipelineError::RrdRead {
                        path: self.path.clone(),
                        reason: err.to_string(),
                    },
                )?
            };
            *cache = Some(stores);
        }
        Ok(cache.as_ref().expect("just populated above").clone())
    }

    /// Resolve the `store` argument to a concrete [`StoreId`].
    ///
    /// If `store` is `Some`, returns its id after validating it belongs to this RRD.
    /// If `None`, picks the first recording store, erroring if there isn't one and
    /// warning if there are several (so the implicit pick doesn't go unnoticed).
    fn resolve_target(
        &self,
        py: Python<'_>,
        store: Option<&PyStoreEntryInternal>,
    ) -> PyResult<StoreId> {
        let cached = self.ensure_cached_stores(py)?;
        if let Some(s) = store {
            if !cached.contains(&s.store_id) {
                return Err(PyValueError::new_err(format!(
                    "Store {:?} not found in RRD file",
                    s.store_id
                )));
            }
            return Ok(s.store_id.clone());
        }
        let recordings: Vec<StoreId> = cached
            .into_iter()
            .filter(|id| id.kind() == StoreKind::Recording)
            .collect();
        let first = recordings
            .first()
            .cloned()
            .ok_or_else(|| PyValueError::new_err("No recording store found in RRD file"))?;
        if recordings.len() > 1 {
            crate::utils::py_rerun_warn(&format!(
                "RRD contains {} recording stores; implicitly using {:?}. \
                 Pass `store=…` to select explicitly (see `recordings()`).",
                recordings.len(),
                first
            ))?;
        }
        Ok(first)
    }
}

/// Factory for creating RRD chunk streams targeting a specific store.
pub struct RrdStreamFactory {
    path: PathBuf,
    target_store_id: StoreId,
}

impl RrdStreamFactory {
    pub fn new(path: PathBuf, target_store_id: StoreId) -> Self {
        Self {
            path,
            target_store_id,
        }
    }
}

impl ChunkStreamFactory for RrdStreamFactory {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        Ok(Box::new(RrdStream::new(
            &self.path,
            self.target_store_id.clone(),
        )))
    }
}

/// Chunk stream that lazily decodes an RRD file, yielding chunks from a single target store.
///
/// Construction is fallible: I/O errors (missing file, permission denied) are
/// captured and surfaced on the first `next()` call rather than panicking.
// TODO(RR-4850): this is the synchronous (no-thread) reference case for the shared
// adapter. Once `IterStream` lands, `RrdStream` could be replaced by it, keeping the
// per-item store-filtering as a `filter_map` on the decoder iterator.
enum RrdStream {
    /// Normal operation: lazily decode messages from the file.
    Live {
        path: PathBuf,
        decoder: Box<dyn Iterator<Item = Result<LogMsg, re_log_encoding::DecodeError>> + Send>,

        /// The `StoreId` of the store whose chunks we yield. Everything else is skipped.
        target_store_id: StoreId,
    },

    /// The file could not be opened. The error is yielded once, then the stream terminates.
    Failed(Option<ChunkPipelineError>),
}

impl RrdStream {
    fn new(path: &Path, target_store_id: StoreId) -> Self {
        match std::fs::File::open(path) {
            Ok(file) => {
                let reader = std::io::BufReader::new(file);
                let decoder = re_log_encoding::Decoder::<LogMsg>::decode_lazy(reader);
                Self::Live {
                    path: path.to_path_buf(),
                    decoder: Box::new(decoder),
                    target_store_id,
                }
            }

            Err(err) => Self::Failed(Some(ChunkPipelineError::RrdRead {
                path: path.to_path_buf(),
                reason: err.to_string(),
            })),
        }
    }
}

impl ChunkStream for RrdStream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        match self {
            Self::Failed(stored_err) => match stored_err.take() {
                Some(err) => Err(err),
                None => Ok(None),
            },

            Self::Live {
                path,
                decoder,
                target_store_id,
            } => loop {
                let Some(msg_result) = decoder.next() else {
                    return Ok(None);
                };

                let msg = msg_result.map_err(|err| ChunkPipelineError::RrdRead {
                    path: path.clone(),
                    reason: err.to_string(),
                })?;

                match msg {
                    LogMsg::SetStoreInfo(_) | LogMsg::BlueprintActivationCommand(_) => {}

                    LogMsg::ArrowMsg(ref store_id, ref arrow_msg) => {
                        if store_id == target_store_id {
                            let chunk = Chunk::from_arrow_msg(arrow_msg).map_err(|err| {
                                ChunkPipelineError::RrdChunkDecode {
                                    reason: err.to_string(),
                                }
                            })?;
                            return Ok(Some(Arc::new(chunk)));
                        }
                        // Chunk belongs to a different store — skip it.
                    }
                }
            },
        }
    }
}

/// Look up `target`'s manifest in an RRD footer.
fn pick_manifest(
    rrd_footer: &RrdFooter,
    path: &Path,
    target: &StoreId,
) -> Result<RawRrdManifest, ChunkPipelineError> {
    let raw_manifest =
        rrd_footer
            .manifests
            .get(target)
            .ok_or_else(|| ChunkPipelineError::RrdRead {
                path: path.to_path_buf(),
                reason: format!("Store {target:?} not found in RRD footer"),
            })?;
    Ok(raw_manifest.clone())
}
