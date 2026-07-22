//! Lazy, composable chunk pipeline — Python SDK bridge.
//!
//! # Layout
//!
//! This module separates **what** the pipeline does from **how** it executes:
//!
//! - [`stream`] contains the declarative pipeline description: filter types,
//!   pipeline steps, stream sources, and the [`stream::LazyChunkStream`] builder.
//!   These types map directly to the Python-level API and are expected to be
//!   stable.
//!
//! - [`engine`] contains the pull-based execution engine that sits behind
//!   [`stream::LazyChunkStream::compile`]. It is an implementation detail and
//!   may be replaced wholesale (e.g. with a DataFusion-backed optimizer) without
//!   affecting the public API.
//!
//! - The PyO3 bindings ([`rrd_reader`], [`py_stream`]) translate between
//!   Python objects and the Rust pipeline types.

pub mod chunk_store;
mod engine;
pub mod error;
mod hdf5_reader;
pub mod lazy_store;
mod mcap_reader;
mod mp4_reader;
mod parquet_reader;
mod py_stream;
pub mod rrd_reader;
pub mod stream;
mod summary;
pub mod urdf_tree_stream;

use std::sync::Arc;

use pyo3::types::{PyModule, PyModuleMethods as _};
use pyo3::{Bound, PyResult, wrap_pyfunction};

pub use py_stream::PyLazyChunkStreamInternal;

/// Register chunk pipeline classes into the module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<rrd_reader::PyRrdReaderInternal>()?;
    m.add_class::<rrd_reader::PyStoreEntryInternal>()?;
    m.add_class::<mcap_reader::PyMcapReaderInternal>()?;
    m.add_class::<mp4_reader::PyMp4ReaderInternal>()?;
    m.add_class::<hdf5_reader::PyHdf5ReaderInternal>()?;
    m.add_class::<mp4_reader::PyMp4TranscodeOptions>()?;
    m.add_class::<parquet_reader::PyParquetReaderInternal>()?;
    m.add_class::<py_stream::PyLazyChunkStreamInternal>()?;
    m.add_class::<py_stream::PyLazyChunkStreamIterator>()?;
    m.add_class::<chunk_store::PyChunkStoreInternal>()?;
    m.add_class::<lazy_store::PyLazyStoreInternal>()?;
    m.add_function(wrap_pyfunction!(
        py_stream::_optimization_profile_values,
        m
    )?)?;
    Ok(())
}

// TODO(RR-4850): revisit as part of the shared iterator→ChunkStream adapter — this
// capacity should likely be a parameter of the threaded adapter (and the benchmark
// should determine whether each reader wants threading at all).
const CHUNK_CHANNEL_CAPACITY: usize = 16;

/// Pull-based chunk stream. Terminals call `next()` in a loop.
///
/// `Ok(None)` indicates successful termination of the stream.
/// `Err(err)` indicates a fatal error that should terminate the pipeline.
pub trait ChunkStream: Send {
    fn next(&mut self) -> Result<Option<Arc<re_chunk::Chunk>>, error::ChunkPipelineError>;
}

/// Factory that creates a [`ChunkStream`], e.g. from a data source.
///
/// Each call to [`create()`](ChunkStreamFactory::create) produces an independent, fresh stream
/// (new file handle, new decoder state, etc.). Implementations hold source configuration
/// (e.g. paths, decoder settings, etc.).
pub trait ChunkStreamFactory: Send + Sync {
    fn create(&self) -> Result<Box<dyn ChunkStream>, error::ChunkPipelineError>;

    /// Create a stream with `filter` pushed into the source as far as the source can manage.
    ///
    /// The returned stream is responsible for producing chunks that satisfy `filter` —
    /// implementations that can't fully absorb the filter wrap their result in an
    /// [`engine::FilterStream`] (the default impl does exactly that).
    ///
    /// The default implementation does no pushdown: it calls [`Self::create`] and wraps the
    /// result with the input filter. Sources that can do better should override this.
    fn create_with_pushdown(
        &self,
        filter: &stream::StructuredFilter,
    ) -> Result<Box<dyn ChunkStream>, error::ChunkPipelineError> {
        Ok(Box::new(engine::FilterStream::new(
            self.create()?,
            filter.clone(),
        )))
    }
}
