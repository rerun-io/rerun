use std::path::PathBuf;
use std::sync::Arc;

use pyo3::exceptions::{PyFileNotFoundError, PyValueError};
use pyo3::prelude::*;
use re_chunk::{Chunk, EntityPath, TimelineName};
use re_lerobot::{
    EpisodeIndex, LeRobotConfig, LeRobotDataset, LeRobotDatasetVersion, LeRobotError, VideoMode,
};

use super::error::ChunkPipelineError;
use super::py_stream::PyLazyChunkStreamInternal;
use super::stream::LazyChunkStream;
use super::{ChunkStream, ChunkStreamFactory};

/// Internal LeRobot reader binding.
#[pyclass(
    frozen,
    name = "LeRobotReaderInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyLeRobotReaderInternal {
    dataset: Arc<LeRobotDataset>,
}

/// The re-creatable source behind a single `stream()` call: dataset + episode + the
/// fully resolved config for that stream.
struct LeRobotStreamFactory {
    dataset: Arc<LeRobotDataset>,
    episode: EpisodeIndex,
    config: LeRobotConfig,
}

#[pymethods]
impl PyLeRobotReaderInternal {
    #[new]
    #[pyo3(text_signature = "(self, path)")]
    fn new(path: &str) -> PyResult<Self> {
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(PyFileNotFoundError::new_err(format!(
                "LeRobot dataset not found: {}",
                path.display()
            )));
        }
        // `open()` detects the version and validates the layout; a directory that is
        // not a (v2/v3) LeRobot dataset is a `ValueError`.
        let dataset = LeRobotDataset::open(&path)
            .map_err(|err| PyValueError::new_err(format!("{err}\nPath: {}", path.display())))?;
        Ok(Self {
            dataset: Arc::new(dataset),
        })
    }

    /// The episode indices available in this dataset, ascending.
    fn episodes(&self) -> Vec<usize> {
        self.dataset.episodes().map(|episode| episode.0).collect()
    }

    /// The detected dataset format version: `"v2"` or `"v3"`.
    #[getter]
    fn version(&self) -> &'static str {
        match self.dataset.version() {
            LeRobotDatasetVersion::V1 => {
                re_log::debug_assert!(
                    false,
                    "`open()` rejects v1 datasets, a constructed reader is always v2 or v3"
                );
                "v1"
            }
            LeRobotDatasetVersion::V2 => "v2",
            LeRobotDatasetVersion::V3 => "v3",
        }
    }

    /// The directory this reader was constructed with.
    #[getter]
    fn path(&self) -> PathBuf {
        self.dataset.path().to_path_buf()
    }

    /// Return a new lazy stream over one episode's chunks.
    ///
    /// Only an unknown episode index fails here; data problems surface while the stream
    /// is drained — nothing is read before then.
    fn stream(
        &self,
        episode: usize,
        entity_path_prefix: Option<String>,
        timeline: Option<String>,
        video_mode: &str,
    ) -> PyResult<PyLazyChunkStreamInternal> {
        let video = match video_mode {
            "native" => VideoMode::Native,
            "skip" => VideoMode::Skip,
            other => {
                return Err(PyValueError::new_err(format!(
                    "Unknown video mode: '{other}'. Expected 'native' or 'skip'."
                )));
            }
        };

        let config = LeRobotConfig {
            entity_path_prefix: entity_path_prefix
                .map(|prefix| EntityPath::from(prefix.as_str()))
                .unwrap_or_else(EntityPath::root),
            timeline_name: timeline
                .map(|timeline| {
                    TimelineName::try_new(&timeline)
                        .map_err(|err| PyValueError::new_err(err.to_string()))
                })
                .transpose()?,
            video,
        };

        let episode = EpisodeIndex(episode);
        // The episode index is dataset metadata, so an unknown one fails here; everything
        // data-scoped surfaces while the stream is drained.
        if !self.dataset.episodes().any(|known| known == episode) {
            return Err(PyValueError::new_err(format!(
                "Invalid episode index: {}",
                episode.0
            )));
        }

        Ok(PyLazyChunkStreamInternal::new(
            LazyChunkStream::from_factory(LeRobotStreamFactory {
                dataset: Arc::clone(&self.dataset),
                episode,
                config,
            }),
        ))
    }
}

// TODO(RR-4850): this spawn-thread + bounded-channel block is hand-copied across
// mp4/mcap/parquet/hdf5/lerobot. Factor it into a shared `spawn_threaded_stream` adapter.
// The stream iterator is created and consumed entirely on the worker thread, so nothing
// here requires `re_lerobot`'s iterator to be `Send`.
impl ChunkStreamFactory for LeRobotStreamFactory {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        let (tx, rx) = crossbeam::channel::bounded::<Result<Arc<Chunk>, ChunkPipelineError>>(
            super::CHUNK_CHANNEL_CAPACITY,
        );

        let dataset = Arc::clone(&self.dataset);
        let episode = self.episode;
        let config = self.config.clone();

        std::thread::Builder::new()
            .name("lerobot-chunk-source".into())
            .spawn(move || {
                match dataset.stream(episode, &config) {
                    Ok(iter) => {
                        for chunk_result in iter {
                            let msg = match chunk_result {
                                Ok(chunk) => Ok(Arc::new(chunk)),
                                Err(err) => Err(lerobot_err(&err)),
                            };
                            if re_quota_channel::send_crossbeam(&tx, msg).is_err() {
                                break; // receiver dropped
                            }
                        }
                    }
                    Err(err) => {
                        re_quota_channel::send_crossbeam(&tx, Err(lerobot_err(&err))).ok();
                    }
                }
                // tx drops here → channel closes → LeRobotStream::next() returns Ok(None)
            })
            .expect("Failed to spawn lerobot decode thread");

        Ok(Box::new(LeRobotStream { rx }))
    }
}

fn lerobot_err(err: &LeRobotError) -> ChunkPipelineError {
    ChunkPipelineError::LeRobot {
        reason: err.to_string(),
    }
}

/// Chunk stream that receives decoded chunks from a background thread.
struct LeRobotStream {
    rx: crossbeam::channel::Receiver<Result<Arc<Chunk>, ChunkPipelineError>>,
}

impl ChunkStream for LeRobotStream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        match self.rx.recv() {
            Ok(Ok(chunk)) => Ok(Some(chunk)),
            Ok(Err(err)) => Err(err),
            Err(crossbeam::channel::RecvError) => Ok(None), // channel closed — loading finished
        }
    }
}
