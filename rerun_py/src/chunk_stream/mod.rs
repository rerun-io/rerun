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
//! - The PyO3 bindings ([`rrd_loader`], [`py_stream`]) translate between
//!   Python objects and the Rust pipeline types.

mod engine;
pub mod error;
mod mcap_loader;
mod py_stream;
pub(crate) mod rrd_loader;
pub mod stream;

use std::sync::Arc;

use pyo3::types::{PyModule, PyModuleMethods as _};
use pyo3::{Bound, PyResult};
use re_chunk::Chunk;

use self::error::ChunkPipelineError;
use self::mcap_loader::PyMcapLoaderInternal;
use self::py_stream::{PyLazyChunkStreamInternal, PyLazyChunkStreamIterator};
use self::rrd_loader::PyRrdLoaderInternal;

/// Register chunk pipeline classes into the module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRrdLoaderInternal>()?;
    m.add_class::<PyMcapLoaderInternal>()?;
    m.add_class::<PyLazyChunkStreamInternal>()?;
    m.add_class::<PyLazyChunkStreamIterator>()?;
    Ok(())
}

// TODO(ab): this is a blind guess. We should benchmark/profile to find a good value.
const CHUNK_CHANNEL_CAPACITY: usize = 16;

/// Pull-based chunk stream. Terminals call `next()` in a loop.
///
/// `Ok(None)` indicates successful termination of the stream.
/// `Err(err)` indicates a fatal error that should terminate the pipeline.
pub trait ChunkStream: Send {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError>;
}

/// Factory that creates a [`ChunkStream`], e.g. from a data source.
///
/// Each call to [`create()`](ChunkStreamFactory::create) produces an independent, fresh stream
/// (new file handle, new decoder state, etc.). Implementations hold source configuration
/// (e.g. paths, decoder settings, etc.).
pub trait ChunkStreamFactory: Send + Sync {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError>;
}
