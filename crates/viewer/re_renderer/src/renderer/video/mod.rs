mod decoder;

use crate::resource_managers::GpuTexture2D;
use crate::RenderContext;
use re_video::demux::{mp4, VideoLoadError};
use re_video::TimeMs;

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
        media_type: Option<&str>,
        data: &[u8],
    ) -> Result<Self, VideoError> {
        let data = match media_type {
            Some("video/mp4") => mp4::load_mp4(data)?,
            Some(media_type) => {
                return Err(VideoError::Load(VideoLoadError::UnsupportedMediaType(
                    media_type.to_owned(),
                )))
            }
            None => return Err(VideoError::Load(VideoLoadError::UnknownMediaType)),
        };
        let decoder =
            decoder::VideoDecoder::new(render_context, data).ok_or_else(|| VideoError::Init)?;

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
    pub fn frame_at(&mut self, timestamp_s: f64) -> GpuTexture2D {
        re_tracing::profile_function!();
        self.decoder.frame_at(TimeMs::new(timestamp_s * 1e3))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum VideoError {
    #[error("{0}")]
    Load(#[from] VideoLoadError),

    #[error("failed to initialize video decoder")]
    Init,
}
