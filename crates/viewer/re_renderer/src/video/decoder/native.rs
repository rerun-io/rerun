// TODO(#7298): decode on native

#![allow(dead_code, unused_variables, clippy::unnecessary_wraps)]

use std::sync::Arc;

use crate::{
    resource_managers::GpuTexture2D,
    video::{DecodingError, FrameDecodingResult},
    RenderContext,
};

// TODO(#7298): remove `allow` once we have native video decoding
#[allow(unused_imports)]
use super::latest_at_idx;

use super::{alloc_video_frame_texture, VideoDecoder};

/// A [`VideoDecoder`] that always fails.
pub struct NoNativeVideoDecoder {
    data: Arc<re_video::VideoData>,
    zeroed_texture: GpuTexture2D,
}

impl NoNativeVideoDecoder {
    pub fn new(
        render_context: &RenderContext,
        data: Arc<re_video::VideoData>,
    ) -> Result<Self, DecodingError> {
        let device = render_context.device.clone();
        let zeroed_texture = alloc_video_frame_texture(
            &device,
            &render_context.gpu_resources.textures,
            data.config.coded_width as u32,
            data.config.coded_height as u32,
        );
        Ok(Self {
            data,
            zeroed_texture,
        })
    }
}

impl VideoDecoder for NoNativeVideoDecoder {
    #[allow(clippy::unused_self)]
    fn frame_at(
        &mut self,
        _render_ctx: &RenderContext,
        _presentation_timestamp_s: f64,
    ) -> FrameDecodingResult {
        Err(DecodingError::NoNativeSupport)
    }
}
