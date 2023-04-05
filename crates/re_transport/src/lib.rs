//! Crate that handles transporting of rerun log types.

pub mod stream_rrd_from_http;

#[cfg(not(target_arch = "wasm32"))]
mod file_sink;

#[cfg(not(target_arch = "wasm32"))]
pub use file_sink::{FileSink, FileSinkError};
