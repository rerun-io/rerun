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
    #[error("Unsupported codec: {0}")]
    UnsupportedCodec(String),

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

#[cfg(not(target_arch = "wasm32"))]
pub fn new_decoder(video: &crate::VideoData) -> Result<Box<dyn SyncDecoder + Send + 'static>> {
    re_log::trace!(
        "Looking for decoder for {}",
        video.human_readable_codec_string()
    );

    match &video.config.stsd.contents {
        #[cfg(feature = "av1")]
        re_mp4::StsdBoxContent::Av01(_av01_box) => {
            re_log::trace!("Decoding AV1…");
            Ok(Box::new(av1::SyncDav1dDecoder::new()?))
        }

        #[cfg(feature = "ffmpeg")]
        re_mp4::StsdBoxContent::Avc1(avc1_box) => {
            // TODO: check if we have ffmpeg ONCE, and remember
            re_log::trace!("Decoding H.264…");
            Ok(Box::new(ffmpeg::FfmpegCliH264Decoder::new(
                avc1_box.clone(),
                video.timescale,
            )?))
        }

        _ => Err(Error::UnsupportedCodec(video.human_readable_codec_string())),
    }
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
