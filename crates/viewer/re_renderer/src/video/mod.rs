mod decoder;

use re_video::{TimeMs, VideoLoadError};

use crate::{resource_managers::GpuTexture2D, RenderContext};

#[derive(thiserror::Error, Debug)]
pub enum VideoError {
    #[error(transparent)]
    Load(#[from] VideoLoadError),

    #[error(transparent)]
    Init(#[from] DecodingError),
}

/// Error that can occur during frame decoding.
// TODO(jan, andreas): These errors are for the most part specific to the web decoder right now.
#[derive(thiserror::Error, Debug)]
pub enum DecodingError {
    // TODO(#7298): Native support.
    #[error("Video playback not yet available in the native viewer. Try the web viewer instead.")]
    NoNativeSupport,

    #[error("Failed to create VideoDecoder: {0}")]
    DecoderSetupFailure(String),

    #[error("Video seems to be empty, no segments have beem found.")]
    EmptyVideo,

    #[error("The current segment is empty.")]
    EmptySegment,

    #[error("Failed to reset the decoder: {0}")]
    ResetFailure(String),

    #[error("Failed to configure the video decoder: {0}")]
    ConfigureFailure(String),

    #[error("The timestamp passed was negative.")]
    NegativeTimestamp,
}

/// Information about the status of a frame decoding.
pub enum FrameDecodingResult {
    /// The requested frame got decoded and is ready to be used.
    Ready(GpuTexture2D),

    /// The returned texture is from a previous frame or a placeholder, the decoder is still decoding the requested frame.
    Pending(GpuTexture2D),

    /// The decoder encountered an error and was not able to produce a texture for the requested timestamp.
    /// The returned texture is either a placeholder or the last successfully decoded texture.
    Error(DecodingError),
}

/// A video file.
///
/// Supports asynchronously decoding video into GPU textures via [`Video::frame_at`].
pub struct Video {
    decoder: decoder::VideoDecoder,
}

impl Video {
    /// Loads a video from the given data.
    ///
    /// Currently supports the following media types:
    /// - `video/mp4`
    pub fn load(
        render_context: &RenderContext,
        data: &[u8],
        media_type: Option<&str>,
    ) -> Result<Self, VideoError> {
        let data = re_video::VideoData::load_from_bytes(data, media_type)?;
        let decoder = decoder::VideoDecoder::new(render_context, data)?;

        Ok(Self { decoder })
    }

    /// Duration of the video in milliseconds.
    pub fn duration_ms(&self) -> f64 {
        self.decoder.duration_ms()
    }

    /// Natural width of the video.
    pub fn width(&self) -> u32 {
        self.decoder.width()
    }

    /// Natural height of the video.
    pub fn height(&self) -> u32 {
        self.decoder.height()
    }

    /// Returns a texture with the latest frame at the given timestamp.
    ///
    /// If the timestamp is negative, a zeroed texture is returned.
    ///
    /// This API is _asynchronous_, meaning that the decoder may not yet have decoded the frame
    /// at the given timestamp. If the frame is not yet available, the returned texture will be
    /// empty.
    ///
    /// This takes `&mut self` because the decoder maintains a buffer of decoded frames,
    /// which requires mutation. It is also not thread-safe by default.
    pub fn frame_at(&mut self, timestamp_s: f64) -> FrameDecodingResult {
        re_tracing::profile_function!();
        self.decoder.frame_at(TimeMs::new(timestamp_s * 1e3))
    }
}
