use std::path::Path;
use std::sync::Arc;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use re_log_types::{
    EntityPathFilter, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource,
};
use re_types_core::ComponentIdentifier;

use re_chunk_store::{ChunkStore, ChunkStoreConfig};

use super::ChunkStream;
use super::chunk_store::PyChunkStoreInternal;
use super::error::ChunkPipelineError;
use super::stream::{LazyChunkStream, StructuredFilter};
use crate::chunk::PyChunkInternal;

/// Internal lazy chunk stream binding.
///
/// This class implements of form of Rust-like move semantics. Builder methods (filter, split, etc.)
/// **consume** the inner stream via `Option::take()`. This ensures that no lazy stream is used more
/// than once in a pipeline. Terminals (collect, write_rrd, __iter__) borrow without consuming, so
/// the same stream can be materialized multiple times.
#[pyclass(
    frozen,
    name = "LazyChunkStreamInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyLazyChunkStreamInternal {
    // This implements the move semantics while keeping the class "frozen". When the lazy stream is
    // "consumed", aka composed into another lazy stream, the inner `Option` is taken. If the lazy
    // stream is used when it contains `None`, an exception is raised.
    inner: parking_lot::Mutex<Option<LazyChunkStream>>,
}

impl PyLazyChunkStreamInternal {
    pub(crate) fn new(stream: LazyChunkStream) -> Self {
        Self {
            inner: parking_lot::Mutex::new(Some(stream)),
        }
    }

    /// Take ownership of the inner stream (move semantics for builders).
    fn take_inner(&self) -> PyResult<LazyChunkStream> {
        self.inner.lock().take().ok_or_else(|| {
            PyValueError::new_err(
                "This stream has already been consumed by a pipeline operation \
                 (e.g. filter, drop, split, or merge).",
            )
        })
    }

    /// Compile the inner stream without consuming it (borrow semantics for terminals).
    fn compile_inner(&self) -> PyResult<Box<dyn ChunkStream>> {
        let guard = self.inner.lock();
        let stream = guard.as_ref().ok_or_else(|| {
            PyValueError::new_err(
                "This stream has already been consumed by a pipeline operation \
                 (e.g. filter, drop, split, or merge).",
            )
        })?;
        Ok(stream.compile())
    }
}

#[pymethods]
impl PyLazyChunkStreamInternal {
    /// Keep the matching portion of each chunk.
    #[pyo3(signature = (*, content=None, has_timeline=None, is_static=None, components=None))]
    fn filter(
        &self,
        content: Option<Vec<String>>,
        has_timeline: Option<String>,
        is_static: Option<bool>,
        components: Option<Vec<String>>,
    ) -> PyResult<Self> {
        let stream = self.take_inner()?;
        let f = build_structured_filter(content, has_timeline, is_static, components);
        Ok(Self::new(stream.filter(f)))
    }

    /// Drop the matching portion of each chunk.
    #[pyo3(signature = (*, content=None, has_timeline=None, is_static=None, components=None))]
    fn drop_matching(
        &self,
        content: Option<Vec<String>>,
        has_timeline: Option<String>,
        is_static: Option<bool>,
        components: Option<Vec<String>>,
    ) -> PyResult<Self> {
        let stream = self.take_inner()?;
        let f = build_structured_filter(content, has_timeline, is_static, components);
        Ok(Self::new(stream.drop_matching(f)))
    }

    /// Split into (matching, non_matching).
    #[pyo3(signature = (*, content=None, has_timeline=None, is_static=None, components=None))]
    fn split(
        &self,
        content: Option<Vec<String>>,
        has_timeline: Option<String>,
        is_static: Option<bool>,
        components: Option<Vec<String>>,
    ) -> PyResult<(Self, Self)> {
        let stream = self.take_inner()?;
        let f = build_structured_filter(content, has_timeline, is_static, components);
        let (a, b) = stream.split(f);
        Ok((Self::new(a), Self::new(b)))
    }

    /// Apply a Python callable to each chunk (1:1). Consumes this stream.
    fn map(&self, callable: Py<PyAny>) -> PyResult<Self> {
        let stream = self.take_inner()?;
        Ok(Self::new(stream.map(callable)))
    }

    /// Apply a Python callable to each chunk (0:N). Consumes this stream.
    fn flat_map(&self, callable: Py<PyAny>) -> PyResult<Self> {
        let stream = self.take_inner()?;
        Ok(Self::new(stream.flat_map(callable)))
    }

    /// Apply lenses to transform chunk data. Consumes this stream.
    #[expect(clippy::needless_pass_by_value)] // PyO3 requires owned Vec
    fn lenses(
        &self,
        lenses: Vec<PyRef<'_, crate::lenses::PyLensInternal>>,
        output_mode: &str,
    ) -> PyResult<Self> {
        let stream = self.take_inner()?;
        let mode = crate::lenses::parse_output_mode(output_mode)?;
        let mut collection = re_lenses_core::Lenses::new(mode);
        for lens in &lenses {
            collection = collection.add_lens(lens.inner().clone());
        }
        Ok(Self::new(stream.lenses(collection)))
    }

    /// Concatenate chunks from multiple streams into one.
    #[staticmethod]
    #[expect(clippy::needless_pass_by_value)] // PyO3 requires owned Vec for #[staticmethod]
    fn merge(streams: Vec<PyRef<'_, Self>>) -> PyResult<Self> {
        let mut inners = Vec::with_capacity(streams.len());
        for s in &streams {
            inners.push(s.take_inner()?);
        }
        Ok(Self::new(LazyChunkStream::merge(inners)))
    }

    /// Consume the stream and write all chunks to an RRD file.
    fn write_rrd(
        &self,
        py: Python<'_>,
        path: &str,
        application_id: &str,
        recording_id: &str,
    ) -> PyResult<()> {
        let mut compiled = self.compile_inner()?;
        let path = std::path::PathBuf::from(path);
        let app_id = application_id.to_owned();
        let rec_id = recording_id.to_owned();

        py.detach(move || write_rrd_compiled(&mut *compiled, &path, &app_id, &rec_id))
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))
    }

    /// Consume the stream and materialize all chunks into a ChunkStore.
    fn collect(&self, py: Python<'_>) -> PyResult<PyChunkStoreInternal> {
        let mut compiled = self.compile_inner()?;
        py.detach(move || -> Result<_, ChunkPipelineError> {
            let store_id = StoreId::random(StoreKind::Recording, "chunk-store");
            let mut store = ChunkStore::new(store_id, ChunkStoreConfig::ALL_DISABLED);
            while let Some(chunk) = compiled.next()? {
                store
                    .insert_chunk(&chunk)
                    .map_err(|err| ChunkPipelineError::ChunkStoreInsert {
                        reason: err.to_string(),
                    })?;
            }
            Ok(PyChunkStoreInternal::in_memory(store))
        })
        .map_err(PyErr::from)
    }

    /// Consume the stream and return all chunks as a list.
    fn to_chunks(&self, py: Python<'_>) -> PyResult<Vec<PyChunkInternal>> {
        let mut compiled = self.compile_inner()?;
        let chunks: Vec<Arc<re_chunk::Chunk>> = py
            .detach(move || -> Result<_, ChunkPipelineError> {
                let mut result = Vec::new();
                while let Some(chunk) = compiled.next()? {
                    result.push(chunk);
                }
                Ok(result)
            })
            .map_err(PyErr::from)?;
        Ok(chunks.into_iter().map(PyChunkInternal::new).collect())
    }

    /// Iterate over chunks one at a time.
    fn __iter__(&self) -> PyResult<PyLazyChunkStreamIterator> {
        let compiled = self.compile_inner()?;
        Ok(PyLazyChunkStreamIterator {
            stream: parking_lot::Mutex::new(compiled),
        })
    }

    /// Wrap a Python iterable of Chunks into a LazyChunkStream.
    #[staticmethod]
    #[expect(clippy::needless_pass_by_value)] // PyO3 requires owned Py<PyAny> for #[staticmethod]
    fn from_iter(py: Python<'_>, iterable: Py<PyAny>) -> PyResult<Self> {
        let iter_obj = iterable.call_method0(py, "__iter__")?;
        Ok(Self::new(LazyChunkStream::from_py_iter(iter_obj)))
    }
}

// ---------------------------------------------------------------------------
// Iterator
// ---------------------------------------------------------------------------

/// Python iterator over chunks from a compiled stream.
#[pyclass( // NOLINT: ignore[py-cls-eq]
    name = "LazyChunkStreamIterator",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyLazyChunkStreamIterator {
    // Mutex is needed because `frozen` pyclass requires `Sync`, but
    // `Box<dyn ChunkStream>` is only `Send` (the RRD decoder iterator isn't `Sync`).
    stream: parking_lot::Mutex<Box<dyn ChunkStream>>,
}

#[pymethods]
impl PyLazyChunkStreamIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self) -> PyResult<Option<PyChunkInternal>> {
        let mut stream = self.stream.lock();
        match stream.next() {
            Ok(Some(chunk)) => Ok(Some(PyChunkInternal::new(chunk))),
            Ok(None) => Ok(None),
            Err(err) => Err(PyErr::from(err)),
        }
    }

    #[expect(clippy::unused_self)]
    fn __repr__(&self) -> &'static str {
        "LazyChunkStreamIterator"
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a [`StructuredFilter`] from pre-normalized Python arguments.
///
/// All normalization (ContentFilter → list of strings, ComponentDescriptor → string)
/// happens on the Python side before reaching this function.
fn build_structured_filter(
    content: Option<Vec<String>>,
    has_timeline: Option<String>,
    is_static: Option<bool>,
    components: Option<Vec<String>>,
) -> StructuredFilter {
    let content = content.map(|exprs| {
        let rules = exprs.join(" ");
        EntityPathFilter::parse_forgiving(&rules).resolve_without_substitutions()
    });

    let has_timeline = has_timeline.map(|s| re_types_core::TimelineName::from(s.as_str()));

    let components = components.map(|cs| {
        cs.iter()
            .map(|s| ComponentIdentifier::from(s.as_str()))
            .collect()
    });

    StructuredFilter {
        content,
        has_timeline,
        is_static,
        components,
    }
}

/// Write all chunks from a pre-compiled [`ChunkStream`] to an RRD file.
fn write_rrd_compiled(
    stream: &mut dyn ChunkStream,
    path: &Path,
    app_id: &str,
    rec_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let store_id = StoreId::new(StoreKind::Recording, app_id, rec_id);
    let store_info = StoreInfo::new(
        store_id.clone(),
        StoreSource::Other("rerun-sdk-python-chunk-pipeline".into()),
    );

    let file = std::fs::File::create(path)?;
    let mut encoder = re_log_encoding::Encoder::new_eager(
        re_build_info::CrateVersion::LOCAL,
        re_log_encoding::EncodingOptions::PROTOBUF_COMPRESSED,
        file,
    )?;

    encoder.append(&LogMsg::SetStoreInfo(SetStoreInfo {
        row_id: re_tuid::Tuid::new(),
        info: store_info,
    }))?;

    while let Some(chunk) = stream.next()? {
        let arrow_msg = chunk.to_arrow_msg()?;
        encoder.append(&LogMsg::ArrowMsg(store_id.clone(), arrow_msg))?;
    }

    encoder.finish()?;
    Ok(())
}
