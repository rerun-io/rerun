//! Handles different ways of loading Rerun data, e.g.:
//!
//! - Over HTTPS
//! - Over gRPC
//! - From disk
//!
//! Also handles different file types: rrd, images, text files, 3D models, point clouds…

mod data_source;
pub(crate) mod fetch_file_from_http;
mod stream_rrd_from_http;

#[cfg(not(target_arch = "wasm32"))]
mod load_stdin;

pub use re_log_channel::RecordingOpenBehavior;

pub use self::data_source::{
    AuthErrorHandler, FromUriOptions, LogDataSource, LogDataSourceAnalytics,
};
