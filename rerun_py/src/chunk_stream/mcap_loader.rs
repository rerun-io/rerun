use std::path::{Path, PathBuf};
use std::sync::Arc;

use pyo3::exceptions::{PyFileNotFoundError, PyValueError};
use pyo3::prelude::*;

use re_chunk::Chunk;
use re_log_types::TimeType;
use re_mcap::{DecoderIdentifier, SelectedDecoders};

use super::error::ChunkPipelineError;
use super::py_stream::PyLazyChunkStreamInternal;
use super::stream::LazyChunkStream;
use super::{ChunkStream, ChunkStreamFactory};

/// Internal MCAP loader binding.
#[pyclass(
    frozen,
    name = "McapLoaderInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyMcapLoaderInternal {
    path: PathBuf,
    loader: re_data_loader::loader_mcap::McapLoader,
    timeline_type: TimeType,
    timestamp_offset_ns: Option<i64>,
}

#[pymethods]
impl PyMcapLoaderInternal {
    #[new]
    #[pyo3(text_signature = "(self, path, timeline_type, timestamp_offset_ns, decoders)")]
    fn new(
        path: &str,
        timeline_type: &str,
        timestamp_offset_ns: Option<i64>,
        decoders: Option<Vec<String>>,
    ) -> PyResult<Self> {
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(PyFileNotFoundError::new_err(format!(
                "MCAP file not found: {}",
                path.display()
            )));
        }

        let timeline_type = match timeline_type {
            "timestamp" => TimeType::TimestampNs,
            "duration" => TimeType::DurationNs,
            other => {
                return Err(PyValueError::new_err(format!(
                    "Invalid timeline_type: {other:?}. Expected \"timestamp\" or \"duration\""
                )));
            }
        };

        let selected_decoders = match decoders {
            None => SelectedDecoders::All,

            Some(ids) => {
                // Validate decoder names against the registry.
                let valid = re_data_loader::supported_mcap_decoder_identifiers(true);
                for id in &ids {
                    let as_id = DecoderIdentifier::from(id.clone());
                    if !valid.contains(&as_id) {
                        let valid_names: Vec<String> =
                            valid.iter().map(|d| d.to_string()).collect();
                        return Err(PyValueError::new_err(format!(
                            "Unknown decoder: {id:?}. Valid decoders: {valid_names:?}"
                        )));
                    }
                }

                SelectedDecoders::Subset(ids.into_iter().map(DecoderIdentifier::from).collect())
            }
        };

        let loader = re_data_loader::loader_mcap::McapLoader::new(&selected_decoders)
            .with_raw_fallback(true);

        Ok(Self {
            path,
            loader,
            timeline_type,
            timestamp_offset_ns,
        })
    }

    /// Return a new lazy stream over all chunks in the MCAP file.
    fn stream(&self) -> PyLazyChunkStreamInternal {
        PyLazyChunkStreamInternal::new(LazyChunkStream::from_factory(McapStreamFactory::new(
            self.path.clone(),
            self.loader.clone(),
            self.timeline_type,
            self.timestamp_offset_ns,
        )))
    }

    /// The file path this loader was constructed with.
    #[getter]
    fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// Return the list of all supported decoder identifiers.
    #[staticmethod]
    fn available_decoders() -> Vec<String> {
        re_data_loader::supported_mcap_decoder_identifiers(true)
            .into_iter()
            .map(|id| id.to_string())
            .collect()
    }
}

/// Factory for creating chunk streams from MCAP files.
///
/// Wraps a [`re_data_loader::loader_mcap::McapLoader`] (which holds decoder config
/// and pre-built lenses) plus the file path and timeline settings.
pub struct McapStreamFactory {
    path: PathBuf,
    loader: re_data_loader::loader_mcap::McapLoader,
    timeline_type: TimeType,
    timestamp_offset_ns: Option<i64>,
}

impl McapStreamFactory {
    pub fn new(
        path: PathBuf,
        loader: re_data_loader::loader_mcap::McapLoader,
        timeline_type: TimeType,
        timestamp_offset_ns: Option<i64>,
    ) -> Self {
        Self {
            path,
            loader,
            timeline_type,
            timestamp_offset_ns,
        }
    }
}

impl ChunkStreamFactory for McapStreamFactory {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        let (tx, rx) = crossbeam::channel::bounded::<Result<Arc<Chunk>, ChunkPipelineError>>(
            super::CHUNK_CHANNEL_CAPACITY,
        );

        let mmap = mmap_file(&self.path)?;
        let loader = self.loader.clone();
        let timeline_type = self.timeline_type;
        let timestamp_offset_ns = self.timestamp_offset_ns;

        std::thread::Builder::new()
            .name("mcap-chunk-source".into())
            .spawn(move || {
                let result =
                    loader.emit_chunks(&mmap, timeline_type, timestamp_offset_ns, &mut |chunk| {
                        // Stop producing if the receiver has been dropped.
                        re_quota_channel::send_crossbeam(&tx, Ok(Arc::new(chunk))).ok();
                    });
                if let Err(err) = result {
                    re_quota_channel::send_crossbeam(
                        &tx,
                        Err(ChunkPipelineError::Mcap {
                            reason: err.to_string(),
                        }),
                    )
                    .ok();
                }
                // tx drops here → channel closes → McapSource::next() returns Ok(None)
            })
            .expect("Failed to spawn MCAP decode thread");

        Ok(Box::new(McapStream { rx }))
    }
}

/// Chunk stream that receives decoded chunks from a background thread.
struct McapStream {
    rx: crossbeam::channel::Receiver<Result<Arc<Chunk>, ChunkPipelineError>>,
}

impl ChunkStream for McapStream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        match self.rx.recv() {
            Ok(Ok(chunk)) => Ok(Some(chunk)),
            Ok(Err(err)) => Err(err),
            Err(crossbeam::channel::RecvError) => Ok(None), // channel closed — decoding finished
        }
    }
}

fn mmap_file(path: &Path) -> Result<memmap2::Mmap, ChunkPipelineError> {
    let file = std::fs::File::open(path).map_err(|err| ChunkPipelineError::Mcap {
        reason: format!("{}: {err}", path.display()),
    })?;

    // SAFETY: file-backed mmap; we don't modify the file while mapped.
    #[expect(unsafe_code)]
    unsafe {
        memmap2::Mmap::map(&file).map_err(|err| ChunkPipelineError::Mcap {
            reason: format!("mmap failed for {}: {err}", path.display()),
        })
    }
}
