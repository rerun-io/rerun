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

pub use self::data_source::{AuthErrorHandler, LogDataSource, LogDataSourceAnalytics};

// ----------------------------------------------------------------------------

/// The contents of a file.
///
/// This is what you get when loading a file on Web, or when using drag-n-drop.
//
// TODO(#4554): drag-n-drop streaming support
#[derive(Clone, PartialEq, Eq)]
pub struct FileContents {
    pub name: String,
    pub bytes: std::sync::Arc<[u8]>,
}

impl std::fmt::Debug for FileContents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileContents")
            .field("name", &self.name)
            .field("bytes", &format_args!("{} bytes", self.bytes.len()))
            .finish()
    }
}
