#![allow(dead_code, unused_variables, clippy::unnecessary_wraps)]

use std::sync::Arc;

use crate::{
    resource_managers::GpuTexture2D,
    video::{DecodeHardwareAcceleration, DecodingError, FrameDecodingResult},
    RenderContext,
};

// TODO(#7298): remove `allow` once we have native video decoding
#[allow(unused_imports)]
use super::latest_at_idx;

use super::alloc_video_frame_texture;

pub struct VideoDecoder {
    data: Arc<re_video::VideoData>,
    zeroed_texture: GpuTexture2D,
}

impl VideoDecoder {
    pub fn new(
        render_context: &RenderContext,
        data: Arc<re_video::VideoData>,
        _hw_acceleration: DecodeHardwareAcceleration,
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

    #[allow(clippy::unused_self)]
    pub fn frame_at(
        &mut self,
        _render_ctx: &RenderContext,
        _presentation_timestamp_s: f64,
    ) -> FrameDecodingResult {
        Err(DecodingError::NoNativeSupport)
    }
}
