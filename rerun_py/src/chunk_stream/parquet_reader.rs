use std::path::PathBuf;
use std::sync::Arc;

use pyo3::exceptions::{PyFileNotFoundError, PyValueError};
use pyo3::prelude::*;
use re_chunk::{Chunk, EntityPath};
use re_parquet::{ColumnGrouping, IndexColumn, IndexType, ParquetConfig, TimeUnit};

use super::error::ChunkPipelineError;
use super::py_stream::PyLazyChunkStreamInternal;
use super::stream::LazyChunkStream;
use super::{ChunkStream, ChunkStreamFactory};

/// Internal Parquet reader binding.
#[pyclass(
    frozen,
    name = "ParquetReaderInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyParquetReaderInternal {
    path: PathBuf,
}

#[pymethods]
impl PyParquetReaderInternal {
    #[new]
    #[pyo3(text_signature = "(self, path)")]
    fn new(path: &str) -> PyResult<Self> {
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(PyFileNotFoundError::new_err(format!(
                "Parquet file not found: {}",
                path.display()
            )));
        }
        Ok(Self { path })
    }

    /// Return a new lazy stream over all chunks in the Parquet file.
    ///
    /// The config is validated against the file's schema up front (footer only), so
    /// bad configuration fails here rather than mid-stream.
    #[pyo3(signature = (
        entity_path_prefix = None,
        column_grouping = "prefix",
        delimiter = '_',
        prefixes = None,
        use_structs = true,
        static_columns = None,
        index_columns = None,
    ))]
    fn stream(
        &self,
        entity_path_prefix: Option<String>,
        column_grouping: &str,
        delimiter: char,
        prefixes: Option<Vec<String>>,
        use_structs: bool,
        static_columns: Option<Vec<String>>,
        index_columns: Option<Vec<(String, String, Option<String>)>>,
    ) -> PyResult<PyLazyChunkStreamInternal> {
        let grouping = match column_grouping {
            "individual" => {
                if prefixes.is_some() {
                    return Err(PyValueError::new_err(
                        "'prefixes' is only valid with column_grouping='explicit_prefixes'",
                    ));
                }
                ColumnGrouping::Individual
            }
            "prefix" => {
                if prefixes.is_some() {
                    return Err(PyValueError::new_err(
                        "'prefixes' is only valid with column_grouping='explicit_prefixes'",
                    ));
                }
                ColumnGrouping::Prefix {
                    delimiter,
                    use_structs,
                }
            }
            "explicit_prefixes" => {
                let pfx = prefixes.ok_or_else(|| {
                    PyValueError::new_err("'explicit_prefixes' requires the 'prefixes' parameter")
                })?;
                if pfx.is_empty() {
                    return Err(PyValueError::new_err(
                        "'prefixes' must not be empty for 'explicit_prefixes' grouping",
                    ));
                }
                ColumnGrouping::ExplicitPrefixes {
                    prefixes: pfx,
                    use_structs,
                }
            }
            other => {
                return Err(PyValueError::new_err(format!(
                    "Unknown column_grouping: '{other}'. \
                     Expected 'prefix', 'individual', or 'explicit_prefixes'."
                )));
            }
        };

        let index_cols: Vec<IndexColumn> = index_columns
            .unwrap_or_default()
            .into_iter()
            .map(|(name, type_str, unit_str)| {
                let unit = match unit_str.as_deref().unwrap_or("ns") {
                    "ns" => TimeUnit::Nanoseconds,
                    "us" => TimeUnit::Microseconds,
                    "ms" => TimeUnit::Milliseconds,
                    "s" => TimeUnit::Seconds,
                    other => {
                        return Err(PyValueError::new_err(format!(
                            "Unknown time unit: '{other}'. Expected 'ns', 'us', 'ms', or 's'."
                        )));
                    }
                };
                let index_type = match type_str.as_str() {
                    "timestamp" => IndexType::Timestamp(unit),
                    "duration" => IndexType::Duration(unit),
                    "sequence" => IndexType::Sequence,
                    other => {
                        return Err(PyValueError::new_err(format!(
                            "Unknown index type: '{other}'. Expected 'timestamp', 'duration', or 'sequence'."
                        )));
                    }
                };
                Ok(IndexColumn { name, index_type })
            })
            .collect::<PyResult<Vec<_>>>()?;

        let config = ParquetConfig {
            column_grouping: grouping,
            index_columns: index_cols,
            static_columns: static_columns.unwrap_or_default(),
        };

        let prefix = entity_path_prefix
            .map(EntityPath::from)
            .unwrap_or_else(ParquetConfig::default_entity_path_prefix);

        // Per-stream config validation (schema/footer only): fail fast before
        // spawning the worker. Misconfiguration and open/parse failures of a present
        // file become `ValueError`.
        re_parquet::validate_config(&self.path, &config).map_err(|err| {
            PyValueError::new_err(format!("{err}\nFile path: {}", self.path.display()))
        })?;

        Ok(PyLazyChunkStreamInternal::new(
            LazyChunkStream::from_factory(ParquetStreamFactory {
                path: self.path.clone(),
                config,
                entity_path_prefix: prefix,
            }),
        ))
    }

    /// The file path this reader was constructed with.
    #[getter]
    fn path(&self) -> PathBuf {
        self.path.clone()
    }
}

/// The source behind a single `stream()` call: path + the fully resolved config for that stream.
struct ParquetStreamFactory {
    path: PathBuf,
    config: ParquetConfig,
    entity_path_prefix: EntityPath,
}

// TODO(RR-4850): this spawn-thread + bounded-channel block is hand-copied across
// mp4/mcap/parquet/hdf5. Factor it into a shared `spawn_threaded_stream` adapter and
// benchmark. Note parquet's iterator is `!Send` (see below), so the threaded
// adapter must bound only the `produce` closure as `Send` — not `I` itself, since
// the iterator is created and consumed entirely on the worker thread. This reader
// therefore cannot use the synchronous `IterStream` variant.
impl ChunkStreamFactory for ParquetStreamFactory {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        let (tx, rx) = crossbeam::channel::bounded::<Result<Arc<Chunk>, ChunkPipelineError>>(
            super::CHUNK_CHANNEL_CAPACITY,
        );

        let path = self.path.clone();
        let config = self.config.clone();
        let prefix = self.entity_path_prefix.clone();

        // NOTE: `re_parquet::load_parquet` returns an iterator whose inner type
        // (`ParquetChunkIterator`) contains `Box<dyn Iterator<...>>` without a
        // `Send` bound. The iterator must therefore be created and consumed on
        // the same thread — we call `load_parquet` inside the spawned thread.
        std::thread::Builder::new()
            .name("parquet-chunk-source".into())
            .spawn(move || {
                match re_parquet::load_parquet(&path, &config, &prefix) {
                    Ok(iter) => {
                        for chunk_result in iter {
                            let msg = match chunk_result {
                                Ok(chunk) => Ok(Arc::new(chunk)),
                                Err(err) => Err(ChunkPipelineError::Parquet {
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
                            Err(ChunkPipelineError::Parquet {
                                reason: err.to_string(),
                            }),
                        )
                        .ok();
                    }
                }
                // tx drops here → channel closes → ParquetStream::next() returns Ok(None)
            })
            .expect("Failed to spawn parquet decode thread");

        Ok(Box::new(ParquetStream { rx }))
    }
}

/// Chunk stream that receives decoded chunks from a background thread.
struct ParquetStream {
    rx: crossbeam::channel::Receiver<Result<Arc<Chunk>, ChunkPipelineError>>,
}

impl ChunkStream for ParquetStream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        match self.rx.recv() {
            Ok(Ok(chunk)) => Ok(Some(chunk)),
            Ok(Err(err)) => Err(err),
            Err(crossbeam::channel::RecvError) => Ok(None), // channel closed — loading finished
        }
    }
}
