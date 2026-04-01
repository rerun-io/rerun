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
mod py_stream;
mod rrd_loader;
pub mod stream;

use pyo3::types::{PyModule, PyModuleMethods as _};
use pyo3::{Bound, PyResult};

pub use self::py_stream::{PyLazyChunkStreamInternal, PyLazyChunkStreamIterator};
pub use self::rrd_loader::PyRrdLoaderInternal;

/// Register chunk pipeline classes into the module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRrdLoaderInternal>()?;
    m.add_class::<PyLazyChunkStreamInternal>()?;
    m.add_class::<PyLazyChunkStreamIterator>()?;
    Ok(())
}
