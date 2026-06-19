use std::path::PathBuf;
use std::sync::Arc;

use pyo3::exceptions::{PyFileNotFoundError, PyValueError};
use pyo3::prelude::*;
use re_chunk::{Chunk, EntityPath};
use re_log_types::TimeType;
use re_mp4_reader::{Mode, Mp4Config};

use super::error::ChunkPipelineError;
use super::py_stream::PyLazyChunkStreamInternal;
use super::stream::LazyChunkStream;
use super::{ChunkStream, ChunkStreamFactory};

/// Internal MP4 reader binding.
#[pyclass(
    frozen,
    name = "Mp4ReaderInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyMp4ReaderInternal {
    path: PathBuf,
    config: Mp4Config,
    entity_path: EntityPath,
}

#[pymethods]
impl PyMp4ReaderInternal {
    #[new]
    #[pyo3(
        signature = (
            path,
            mode = "stream",
            chunk_by_gop = true,
            timeline_name = "video",
            timeline_type = "duration",
            allow_b_frames = false,
            entity_path = None,
        ),
        text_signature = "(self, path, mode='stream', chunk_by_gop=True, timeline_name='video', timeline_type='duration', allow_b_frames=False, entity_path=None)"
    )]
    #[expect(clippy::fn_params_excessive_bools)]
    fn new(
        path: &str,
        mode: &str,
        chunk_by_gop: bool,
        timeline_name: &str,
        timeline_type: &str,
        allow_b_frames: bool,
        entity_path: Option<String>,
    ) -> PyResult<Self> {
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(PyFileNotFoundError::new_err(format!(
                "MP4 file not found: {}",
                path.display()
            )));
        }

        let timeline_type = match timeline_type {
            "duration" => TimeType::DurationNs,
            "timestamp" => TimeType::TimestampNs,
            other => {
                return Err(PyValueError::new_err(format!(
                    "Invalid timeline_type: {other:?}. Expected \"duration\" or \"timestamp\""
                )));
            }
        };

        let mode = match mode {
            "asset" => {
                if !chunk_by_gop {
                    return Err(PyValueError::new_err(
                        "`chunk_by_gop=False` is only valid with `mode=\"stream\"`",
                    ));
                }
                if allow_b_frames {
                    return Err(PyValueError::new_err(
                        "`allow_b_frames=True` is only valid with `mode=\"stream\"`",
                    ));
                }
                Mode::Asset {
                    timepoint: re_chunk::TimePoint::default(),
                }
            }
            "stream" => Mode::Stream {
                chunk_by_gop,
                allow_b_frames,
            },
            other => {
                return Err(PyValueError::new_err(format!(
                    "Invalid mode: {other:?}. Expected \"asset\" or \"stream\""
                )));
            }
        };

        let config = Mp4Config {
            mode,
            timeline_name: timeline_name.into(),
            timeline_type,
        };

        let entity_path = match entity_path {
            Some(s) => EntityPath::from(s),
            None => EntityPath::from_file_path(&path),
        };

        Ok(Self {
            path,
            config,
            entity_path,
        })
    }

    /// Return a new lazy stream over all chunks in the MP4 file.
    fn stream(&self) -> PyLazyChunkStreamInternal {
        PyLazyChunkStreamInternal::new(LazyChunkStream::from_factory(Self {
            path: self.path.clone(),
            config: self.config.clone(),
            entity_path: self.entity_path.clone(),
        }))
    }

    /// The file path this reader was constructed with.
    #[getter]
    fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// The entity path under which chunks are emitted.
    #[getter]
    fn entity_path(&self) -> String {
        self.entity_path.to_string()
    }
}

// TODO(RR-4850): this spawn-thread + bounded-channel block is hand-copied across
// mp4/mcap/parquet. Factor it into a shared `spawn_threaded_stream` adapter (and a
// synchronous `IterStream` sibling), then benchmark whether mp4 wants threaded
// pipelining at all or should use the synchronous wrap. `load_mp4` yields a clean
// `'static + Send` iterator, so mp4 could use either variant.
impl ChunkStreamFactory for PyMp4ReaderInternal {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        let (tx, rx) = crossbeam::channel::bounded::<Result<Arc<Chunk>, ChunkPipelineError>>(
            super::CHUNK_CHANNEL_CAPACITY,
        );

        let path = self.path.clone();
        let config = self.config.clone();
        let entity_path = self.entity_path.clone();

        std::thread::Builder::new()
            .name("mp4-chunk-source".into())
            .spawn(move || {
                match re_mp4_reader::load_mp4(&path, &config, &entity_path) {
                    Ok(iter) => {
                        for chunk_result in iter {
                            let msg = match chunk_result {
                                Ok(chunk) => Ok(Arc::new(chunk)),
                                Err(err) => Err(ChunkPipelineError::Mp4 {
                                    reason: err.to_string(),
                                }),
                            };
                            if re_quota_channel::send_crossbeam(&tx, msg).is_err() {
                                break; // receiver dropped
                            }
                        }
                    }
                    Err(err) => {
                        re_quota_channel::send_crossbeam(
                            &tx,
                            Err(ChunkPipelineError::Mp4 {
                                reason: err.to_string(),
                            }),
                        )
                        .ok();
                    }
                }
                // tx drops here → channel closes → Mp4Stream::next() returns Ok(None)
            })
            .expect("Failed to spawn mp4 decode thread");

        Ok(Box::new(Mp4Stream { rx }))
    }
}

/// Chunk stream that receives decoded chunks from a background thread.
struct Mp4Stream {
    rx: crossbeam::channel::Receiver<Result<Arc<Chunk>, ChunkPipelineError>>,
}

impl ChunkStream for Mp4Stream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        match self.rx.recv() {
            Ok(Ok(chunk)) => Ok(Some(chunk)),
            Ok(Err(err)) => Err(err),
            Err(crossbeam::channel::RecvError) => Ok(None), // channel closed — loading finished
        }
    }
}
