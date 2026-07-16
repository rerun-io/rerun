use std::path::{Path, PathBuf};
use std::sync::Arc;

use pyo3::exceptions::{PyFileNotFoundError, PyValueError};
use pyo3::prelude::*;

use re_chunk::Chunk;
use re_log_types::TimeType;
use re_mcap::{DecoderIdentifier, SelectedDecoders, TopicFilter};

use super::error::ChunkPipelineError;
use super::py_stream::PyLazyChunkStreamInternal;
use super::stream::LazyChunkStream;
use super::{ChunkStream, ChunkStreamFactory};

/// Internal MCAP reader binding.
#[pyclass(
    frozen,
    name = "McapReaderInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyMcapReaderInternal {
    path: PathBuf,
    loader: re_importer::importer_mcap::McapImporter,
    timeline_type: TimeType,
    timestamp_offset_ns: Option<i64>,

    /// The parsed MCAP summary, read once and shared across `time_bounds()` and every `stream()`
    /// so repeated (e.g. windowed) scans don't each re-parse it.
    summary: std::sync::OnceLock<Arc<re_mcap::Summary>>,
}

#[pymethods]
impl PyMcapReaderInternal {
    #[new]
    #[pyo3(
        text_signature = "(self, path, timeline_type, timestamp_offset_ns, decoders, include_topic_regex, exclude_topic_regex, start_time_ns, end_time_ns)"
    )]
    fn new(
        path: &str,
        timeline_type: &str,
        timestamp_offset_ns: Option<i64>,
        decoders: Option<Vec<String>>,
        include_topic_regex: Option<Vec<String>>,
        exclude_topic_regex: Option<Vec<String>>,
        start_time_ns: Option<i64>,
        end_time_ns: Option<i64>,
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
                let valid = re_importer::supported_mcap_decoder_identifiers(true);
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

        let topic_filter = compile_topic_filter(include_topic_regex, exclude_topic_regex)?;
        let time_range = compile_time_range(start_time_ns, end_time_ns)?;

        let loader = re_importer::importer_mcap::McapImporter::new(&selected_decoders)
            .with_raw_fallback(true)
            .with_topic_filter(topic_filter)
            .with_time_range(time_range);

        Ok(Self {
            path,
            loader,
            timeline_type,
            timestamp_offset_ns,
            summary: std::sync::OnceLock::new(),
        })
    }

    /// Return a new lazy stream over the MCAP file.
    ///
    /// `start_time_ns` and `end_time_ns` override the values baked in at construction, for this
    /// scan only; `None` keeps the reader's default. If either time bound is given, the pair
    /// replaces the reader's time range as a whole (a missing side opens that end).
    #[pyo3(signature = (*, start_time_ns=None, end_time_ns=None))]
    fn stream(
        &self,
        start_time_ns: Option<i64>,
        end_time_ns: Option<i64>,
    ) -> PyResult<PyLazyChunkStreamInternal> {
        let mut loader = self.loader.clone();
        if start_time_ns.is_some() || end_time_ns.is_some() {
            loader = loader.with_time_range(compile_time_range(start_time_ns, end_time_ns)?);
        }

        Ok(PyLazyChunkStreamInternal::new(
            LazyChunkStream::from_factory(McapStreamFactory::new(
                self.path.clone(),
                loader,
                self.timeline_type,
                self.timestamp_offset_ns,
                self.summary()?,
            )),
        ))
    }

    /// Return the `(min, max)` MCAP `log_time` bounds (nanoseconds, inclusive) of the file.
    fn time_bounds(&self) -> PyResult<(u64, u64)> {
        let summary = self.summary()?;

        // Prefer the statistics record; fall back to the chunk-index bounds if it is absent or
        // empty (the statistics record is optional per the MCAP spec).
        if let Some(stats) = &summary.stats
            && stats.message_count > 0
        {
            return Ok((stats.message_start_time, stats.message_end_time));
        }

        let mut lo = u64::MAX;
        let mut hi = 0_u64;
        for chunk in &summary.chunk_indexes {
            lo = lo.min(chunk.message_start_time);
            hi = hi.max(chunk.message_end_time);
        }
        if lo > hi {
            return Err(PyValueError::new_err("MCAP file contains no messages"));
        }
        Ok((lo, hi))
    }

    /// The file path this reader was constructed with.
    #[getter]
    fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// Return the list of all supported decoder identifiers.
    #[staticmethod]
    fn available_decoders() -> Vec<String> {
        re_importer::supported_mcap_decoder_identifiers(true)
            .into_iter()
            .map(|id| id.to_string())
            .collect()
    }
}

impl PyMcapReaderInternal {
    /// Return the parsed MCAP summary, reading and caching it on first use.
    fn summary(&self) -> PyResult<Arc<re_mcap::Summary>> {
        if let Some(summary) = self.summary.get() {
            return Ok(summary.clone());
        }

        let mmap = mmap_file(&self.path)?;
        let summary = re_mcap::read_summary(std::io::Cursor::new(&mmap[..]))
            .map_err(|err| PyValueError::new_err(format!("Failed to read MCAP summary: {err}")))?
            .ok_or_else(|| PyValueError::new_err("MCAP file does not contain a summary"))?;

        // A concurrent caller may have won the race; `get_or_init` keeps whichever landed first.
        Ok(self.summary.get_or_init(|| Arc::new(summary)).clone())
    }
}

/// Factory for creating chunk streams from MCAP files.
///
/// Wraps a [`re_importer::importer_mcap::McapImporter`] (which holds decoder config
/// and pre-built lenses) plus the file path and timeline settings.
pub struct McapStreamFactory {
    path: PathBuf,
    loader: re_importer::importer_mcap::McapImporter,
    timeline_type: TimeType,
    timestamp_offset_ns: Option<i64>,
    summary: Arc<re_mcap::Summary>,
}

impl McapStreamFactory {
    pub fn new(
        path: PathBuf,
        loader: re_importer::importer_mcap::McapImporter,
        timeline_type: TimeType,
        timestamp_offset_ns: Option<i64>,
        summary: Arc<re_mcap::Summary>,
    ) -> Self {
        Self {
            path,
            loader,
            timeline_type,
            timestamp_offset_ns,
            summary,
        }
    }
}

// TODO(RR-4850): this spawn-thread + bounded-channel block is hand-copied across
// mp4/mcap/parquet. Factor it into a shared `spawn_threaded_stream` adapter and
// benchmark whether mcap benefits from threaded pipelining. Note mcap pushes chunks
// via an `emit_chunks` callback rather than returning an iterator, so the shared
// adapter must accept a callback-style producer too (not just `Iterator`).
impl ChunkStreamFactory for McapStreamFactory {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        let (tx, rx) = crossbeam::channel::bounded::<Result<Arc<Chunk>, ChunkPipelineError>>(
            super::CHUNK_CHANNEL_CAPACITY,
        );

        let mmap = mmap_file(&self.path)?;
        let loader = self.loader.clone();
        let timeline_type = self.timeline_type;
        let timestamp_offset_ns = self.timestamp_offset_ns;
        let summary = self.summary.clone();

        std::thread::Builder::new()
            .name("mcap-chunk-source".into())
            .spawn(move || {
                let result = loader.emit_chunks_with_summary(
                    &mmap,
                    &summary,
                    timeline_type,
                    timestamp_offset_ns,
                    &|chunk| {
                        // Stop producing if the receiver has been dropped.
                        re_quota_channel::send_crossbeam(&tx, Ok(Arc::new(chunk))).ok();
                    },
                );
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

fn compile_topic_filter(
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
) -> PyResult<TopicFilter> {
    let include = include.unwrap_or_default();
    let exclude = exclude.unwrap_or_default();

    for pattern in &include {
        TopicFilter::default()
            .with_include_patterns(std::slice::from_ref(pattern))
            .map_err(|err| {
                PyValueError::new_err(format!("Invalid include topic regex {pattern:?}: {err}"))
            })?;
    }
    for pattern in &exclude {
        TopicFilter::default()
            .with_exclude_patterns(std::slice::from_ref(pattern))
            .map_err(|err| {
                PyValueError::new_err(format!("Invalid exclude topic regex {pattern:?}: {err}"))
            })?;
    }

    TopicFilter::default()
        .with_include_patterns(&include)
        .and_then(|filter| filter.with_exclude_patterns(&exclude))
        .map_err(|err| PyValueError::new_err(format!("Invalid topic regex: {err}")))
}

/// Normalize the optional `start`/`end` `log_time` bounds into an inclusive-start,
/// exclusive-end `[start, end)` range in nanoseconds.
///
/// Returns `None` (no filtering) when both bounds are `None`. A missing `start` opens the
/// range at 0; a missing `end` opens it at `u64::MAX`. MCAP `log_time` is unsigned, so
/// negative inputs are rejected, as is `start >= end` (a half-open range with `start == end`
/// is empty).
fn compile_time_range(
    start_time_ns: Option<i64>,
    end_time_ns: Option<i64>,
) -> PyResult<Option<(u64, u64)>> {
    if start_time_ns.is_none() && end_time_ns.is_none() {
        return Ok(None);
    }

    let start = match start_time_ns {
        Some(s) if s < 0 => {
            return Err(PyValueError::new_err(format!(
                "start_time_ns must be non-negative (MCAP log_time is unsigned), got {s}"
            )));
        }
        Some(s) => s as u64,
        None => 0,
    };
    let end = match end_time_ns {
        Some(e) if e < 0 => {
            return Err(PyValueError::new_err(format!(
                "end_time_ns must be non-negative (MCAP log_time is unsigned), got {e}"
            )));
        }
        Some(e) => e as u64,
        None => u64::MAX,
    };

    if start >= end {
        return Err(PyValueError::new_err(format!(
            "start_time_ns ({start}) must be less than end_time_ns ({end}); the range is half-open [start, end)"
        )));
    }

    Ok(Some((start, end)))
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
