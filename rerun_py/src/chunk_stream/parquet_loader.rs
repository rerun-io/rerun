use std::path::PathBuf;
use std::sync::Arc;

use pyo3::exceptions::{PyFileNotFoundError, PyValueError};
use pyo3::prelude::*;

use re_chunk::{Chunk, EntityPath};
use re_parquet::{
    ColumnGrouping, ComponentRule, IndexColumn, IndexType, MappedComponent, ParquetConfig,
    ScalarSuffixGroup, TimeUnit,
};

use super::error::ChunkPipelineError;
use super::py_stream::PyLazyChunkStreamInternal;
use super::stream::LazyChunkStream;
use super::{ChunkStream, ChunkStreamFactory};

/// Internal Parquet loader binding.
#[pyclass(
    frozen,
    name = "ParquetLoaderInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyParquetLoaderInternal {
    path: PathBuf,
    config: ParquetConfig,
    entity_path_prefix: EntityPath,
}

#[pymethods]
impl PyParquetLoaderInternal {
    #[new]
    #[expect(clippy::too_many_arguments)]
    #[pyo3(
        signature = (
            path,
            entity_path_prefix = None,
            column_grouping = "prefix",
            delimiter = '_',
            static_columns = None,
            index_columns = None,
            pos_suffixes = None,
            quat_suffixes = None,
            scalar_suffixes = None,
        ),
        text_signature = "(self, path, entity_path_prefix=None, column_grouping='prefix', delimiter='_', static_columns=None, index_columns=None, pos_suffixes=None, quat_suffixes=None, scalar_suffixes=None)"
    )]
    fn new(
        path: &str,
        entity_path_prefix: Option<String>,
        column_grouping: &str,
        delimiter: char,
        static_columns: Option<Vec<String>>,
        index_columns: Option<Vec<(String, String, Option<String>)>>,
        pos_suffixes: Option<Vec<Vec<String>>>,
        quat_suffixes: Option<Vec<Vec<String>>>,
        scalar_suffixes: Option<Vec<(Vec<String>, Vec<String>)>>,
    ) -> PyResult<Self> {
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(PyFileNotFoundError::new_err(format!(
                "Parquet file not found: {}",
                path.display()
            )));
        }

        let grouping = match column_grouping {
            "individual" => ColumnGrouping::Individual,
            "prefix" => ColumnGrouping::Prefix { delimiter },
            other => {
                return Err(PyValueError::new_err(format!(
                    "Unknown column_grouping: '{other}'. Expected 'prefix' or 'individual'."
                )));
            }
        };

        let mut archetype_rules = Vec::new();
        if let Some(pos_groups) = pos_suffixes {
            for suffixes in pos_groups {
                archetype_rules.push(ComponentRule {
                    suffixes,
                    target: MappedComponent::Translation3D,
                });
            }
        }
        if let Some(quat_groups) = quat_suffixes {
            for suffixes in quat_groups {
                archetype_rules.push(ComponentRule {
                    suffixes,
                    target: MappedComponent::RotationQuat,
                });
            }
        }

        let scalar_groups: Vec<ScalarSuffixGroup> = scalar_suffixes
            .unwrap_or_default()
            .into_iter()
            .map(|(suffixes, names)| ScalarSuffixGroup { suffixes, names })
            .collect();

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
            archetype_rules,
            scalar_suffixes: scalar_groups,
        };

        let prefix = entity_path_prefix
            .map(EntityPath::from)
            .unwrap_or_else(ParquetConfig::default_entity_path_prefix);

        Ok(Self {
            path,
            config,
            entity_path_prefix: prefix,
        })
    }

    /// Return a new lazy stream over all chunks in the Parquet file.
    fn stream(&self) -> PyLazyChunkStreamInternal {
        PyLazyChunkStreamInternal::new(LazyChunkStream::from_factory(Self {
            path: self.path.clone(),
            config: self.config.clone(),
            entity_path_prefix: self.entity_path_prefix.clone(),
        }))
    }

    /// The file path this loader was constructed with.
    #[getter]
    fn path(&self) -> PathBuf {
        self.path.clone()
    }
}

impl ChunkStreamFactory for PyParquetLoaderInternal {
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
            Err(_) => Ok(None), // channel closed — loading finished
        }
    }
}
