use std::path::PathBuf;
use std::sync::Arc;

use pyo3::exceptions::{PyFileNotFoundError, PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use re_chunk::{Chunk, EntityPath};
use re_hdf5::{AttrValue, Hdf5Config, Hdf5Error, IndexColumn, IndexType, TimeUnit};

use super::error::ChunkPipelineError;
use super::py_stream::PyLazyChunkStreamInternal;
use super::stream::LazyChunkStream;
use super::{ChunkStream, ChunkStreamFactory};

/// Internal HDF5 reader binding.
#[pyclass(
    frozen,
    name = "Hdf5ReaderInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyHdf5ReaderInternal {
    path: PathBuf,
}

/// The re-creatable source behind a single `stream()` call: path + the fully
/// resolved config for that stream.
struct Hdf5StreamFactory {
    path: PathBuf,
    config: Hdf5Config,
}

/// Map a `stream()` validation error to the Python exception the `Hdf5Reader`
/// contract promises: bad configuration (misalignment, bad `index_column`)
/// and — deliberately, since `validate_layout` opens the file eagerly —
/// open/parse failures of a present file all become `ValueError`.
fn validate_err_to_py(err: &Hdf5Error, path: &std::path::Path) -> PyErr {
    PyValueError::new_err(format!("{err}\nFile path: {}", path.display()))
}

/// Map a metadata-accessor error: a missing object is a `KeyError`, anything
/// else (a genuine read failure) a `RuntimeError`.
fn accessor_err_to_py(err: &Hdf5Error) -> PyErr {
    if err.is_not_found() {
        PyKeyError::new_err(err.to_string())
    } else {
        PyRuntimeError::new_err(err.to_string())
    }
}

#[pymethods]
impl PyHdf5ReaderInternal {
    #[new]
    #[pyo3(text_signature = "(self, path)")]
    fn new(path: &str) -> PyResult<Self> {
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(PyFileNotFoundError::new_err(format!(
                "HDF5 file not found: {}",
                path.display()
            )));
        }
        Ok(Self { path })
    }

    /// Return a new lazy stream over all chunks in the HDF5 file.
    ///
    /// The layout is validated against `config` up front (metadata only), so bad
    /// configuration fails here rather than mid-stream.
    #[pyo3(signature = (
        entity_path_prefix = None,
        index_column = None,
        ignore_datasets = None,
        use_structs = true,
    ))]
    fn stream(
        &self,
        entity_path_prefix: Option<String>,
        index_column: Option<(String, String, Option<String>)>,
        ignore_datasets: Option<Vec<String>>,
        use_structs: bool,
    ) -> PyResult<PyLazyChunkStreamInternal> {
        let index_column = index_column
            .map(|(dataset_path, type_str, unit_str)| {
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
                Ok(IndexColumn {
                    path: dataset_path,
                    index_type,
                })
            })
            .transpose()?;

        let config = Hdf5Config {
            index_column,
            ignore_datasets: ignore_datasets.unwrap_or_default(),
            use_structs,
            entity_path_prefix: entity_path_prefix
                .map(EntityPath::from)
                .unwrap_or_else(|| Hdf5Config::default().entity_path_prefix),
        };

        // Per-stream structural validation: fail fast before spawning the worker.
        re_hdf5::validate_layout(&self.path, &config)
            .map_err(|err| validate_err_to_py(&err, &self.path))?;

        Ok(PyLazyChunkStreamInternal::new(
            LazyChunkStream::from_factory(Hdf5StreamFactory {
                path: self.path.clone(),
                config,
            }),
        ))
    }

    /// List the group paths under `path`, recursively.
    #[pyo3(signature = (path = "/"))]
    fn groups(&self, path: &str) -> PyResult<Vec<String>> {
        re_hdf5::list_groups(&self.path, path).map_err(|err| accessor_err_to_py(&err))
    }

    /// List the datasets under `path`, recursively, as `(path, shape, dtype)` tuples.
    #[pyo3(signature = (path = "/"))]
    fn datasets(&self, path: &str) -> PyResult<Vec<(String, Vec<u64>, String)>> {
        Ok(re_hdf5::list_datasets(&self.path, path)
            .map_err(|err| accessor_err_to_py(&err))?
            .into_iter()
            .map(|info| (info.path, info.shape, info.dtype.to_string()))
            .collect())
    }

    /// Read the attributes of the object at `path` as a typed Python dict.
    #[pyo3(signature = (path = "/"))]
    fn attributes<'py>(&self, py: Python<'py>, path: &str) -> PyResult<Bound<'py, PyDict>> {
        let attrs =
            re_hdf5::read_attributes(&self.path, path).map_err(|err| accessor_err_to_py(&err))?;

        let dict = PyDict::new(py);
        for (name, value) in attrs {
            match value {
                AttrValue::F64(value) => dict.set_item(name, value)?,
                AttrValue::I32(value) => dict.set_item(name, value)?,
                AttrValue::I64(value) => dict.set_item(name, value)?,
                AttrValue::U32(value) => dict.set_item(name, value)?,
                AttrValue::U64(value) => dict.set_item(name, value)?,
                AttrValue::String(value) | AttrValue::AsciiString(value) => {
                    dict.set_item(name, value)?;
                }
                AttrValue::F64Array(values) => dict.set_item(name, values)?,
                AttrValue::I64Array(values) => dict.set_item(name, values)?,
                AttrValue::StringArray(values)
                | AttrValue::AsciiStringArray(values)
                | AttrValue::VarLenAsciiArray(values) => dict.set_item(name, values)?,
            }
        }
        Ok(dict)
    }

    /// The file path this reader was constructed with.
    #[getter]
    fn path(&self) -> PathBuf {
        self.path.clone()
    }
}

// TODO(RR-4850): this spawn-thread + bounded-channel block is hand-copied across
// mp4/mcap/parquet/hdf5. Factor it into a shared `spawn_threaded_stream` adapter.
// The iterator is created and consumed entirely on the worker thread, so nothing
// here requires `re_hdf5`'s iterator (or `hdf5_pure::File`) to be `Send`.
impl ChunkStreamFactory for Hdf5StreamFactory {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        let (tx, rx) = crossbeam::channel::bounded::<Result<Arc<Chunk>, ChunkPipelineError>>(
            super::CHUNK_CHANNEL_CAPACITY,
        );

        let path = self.path.clone();
        let config = self.config.clone();

        std::thread::Builder::new()
            .name("hdf5-chunk-source".into())
            .spawn(move || {
                match re_hdf5::load_hdf5(&path, &config) {
                    Ok(iter) => {
                        for chunk_result in iter {
                            let msg = match chunk_result {
                                Ok(chunk) => Ok(Arc::new(chunk)),
                                Err(err) => Err(ChunkPipelineError::Hdf5 {
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
                            Err(ChunkPipelineError::Hdf5 {
                                reason: err.to_string(),
                            }),
                        )
                        .ok();
                    }
                }
                // tx drops here → channel closes → Hdf5Stream::next() returns Ok(None)
            })
            .expect("Failed to spawn hdf5 decode thread");

        Ok(Box::new(Hdf5Stream { rx }))
    }
}

/// Chunk stream that receives decoded chunks from a background thread.
struct Hdf5Stream {
    rx: crossbeam::channel::Receiver<Result<Arc<Chunk>, ChunkPipelineError>>,
}

impl ChunkStream for Hdf5Stream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        match self.rx.recv() {
            Ok(Ok(chunk)) => Ok(Some(chunk)),
            Ok(Err(err)) => Err(err),
            Err(crossbeam::channel::RecvError) => Ok(None), // channel closed — loading finished
        }
    }
}
