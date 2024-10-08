//! Video frame decoding.

#[cfg(feature = "av1")]
#[cfg(not(target_arch = "wasm32"))]
pub mod av1;

#[cfg(not(target_arch = "wasm32"))]
pub mod async_decoder;

#[cfg(not(target_arch = "wasm32"))]
pub use async_decoder::AsyncDecoder;

#[cfg(feature = "ffmpeg")]
#[cfg(not(target_arch = "wasm32"))]
pub mod ffmpeg;

use std::sync::atomic::AtomicBool;

use crate::Time;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[cfg(feature = "av1")]
    #[cfg(not(target_arch = "wasm32"))]
    #[error("dav1d: {0}")]
    Dav1d(#[from] dav1d::Error),
}

pub type Result<T = (), E = Error> = std::result::Result<T, E>;

pub type OutputCallback = dyn Fn(Result<Frame>) + Send + Sync;

/// Blocking decoder of video chunks.
pub trait SyncDecoder {
    /// Submit some work and read the results.
    ///
    /// Stop early if `should_stop` is `true` or turns `true`.
    fn submit_chunk(&mut self, should_stop: &AtomicBool, chunk: Chunk, on_output: &OutputCallback);

    /// Clear and reset everything
    fn reset(&mut self) {}
}

/// One chunk of encoded video data; usually one frame.
///
/// One loaded [`crate::Sample`].
pub struct Chunk {
    /// The start of a new [`crate::demux::GroupOfPictures`]?
    pub is_sync: bool,

    pub data: Vec<u8>,
    pub timestamp: Time,
    pub duration: Time,
}

/// One decoded video frame.
pub struct Frame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub timestamp: Time,
    pub duration: Time,
}

pub enum PixelFormat {
    Rgb8Unorm,
    Rgba8Unorm,
}
