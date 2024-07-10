//! Handles different ways of loading Rerun data, e.g.:
//!
//! - Over HTTPS
//! - Over WebSockets
//! - From disk
//!
//! Also handles different file types: rrd, images, text files, 3D models, point clouds…

mod data_source;
mod web_sockets;

#[cfg(not(target_arch = "wasm32"))]
mod load_stdin;

pub use self::data_source::DataSource;
pub use self::web_sockets::connect_to_ws_url;

// ----------------------------------------------------------------------------

/// The contents of a file.
///
/// This is what you get when loading a file on Web, or when using drag-n-drop.
//
// TODO(#4554): drag-n-drop streaming support
#[derive(Clone)]
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
