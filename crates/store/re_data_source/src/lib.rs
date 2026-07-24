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

// ----------------------------------------------------------------------------

/// The contents of a dropped file.
//
// TODO(#4554): drag-n-drop streaming support
#[cfg(target_arch = "wasm32")]
#[derive(Clone, PartialEq, Eq)]
pub struct FileContents {
    pub path: std::path::PathBuf,
    pub bytes: std::sync::Arc<[u8]>,
}

#[cfg(target_arch = "wasm32")]
impl FileContents {
    // TODO(RR-5263): Remove again once we stream to OPFS.
    pub async fn from_file(file: web_sys::File) -> anyhow::Result<Self> {
        let path = std::path::PathBuf::from(file.name());
        let buffer = file
            .array_buffer()
            .await
            // NOLINT: `JsValue` error formatting will be fixed in a follow-up PR using `re_web`.
            .map_err(|err| anyhow::anyhow!("failed to read file: {err:?}"))?;

        Ok(Self {
            path,
            bytes: js_sys::Uint8Array::new(&buffer).to_vec().into(),
        })
    }
}

#[cfg(target_arch = "wasm32")]
impl std::fmt::Debug for FileContents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileContents")
            .field("path", &self.path)
            .field("bytes", &format_args!("{} bytes", self.bytes.len()))
            .finish()
    }
}
