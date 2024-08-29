#![allow(dead_code, unused_variables, clippy::unnecessary_wraps)]

use crate::resource_managers::GpuTexture2D;
use crate::RenderContext;

// TODO(#7298): remove `allow` once we have native video decoding
#[allow(unused_imports)]
use super::latest_at_idx;

use re_video::TimeMs;

use super::alloc_video_frame_texture;

pub struct VideoDecoder {
    data: re_video::VideoData,
    texture: GpuTexture2D,
}

impl VideoDecoder {
    pub fn new(render_context: &RenderContext, data: re_video::VideoData) -> Option<Self> {
        re_log::warn_once!("Video playback not yet available in the native viewer, try the web viewer instead. See https://github.com/rerun-io/rerun/issues/7298 for more information.");

        let device = render_context.device.clone();
        let texture = alloc_video_frame_texture(
            &device,
            &render_context.gpu_resources.textures,
            data.config.coded_width as u32,
            data.config.coded_height as u32,
        );
        Some(Self { data, texture })
    }

    pub fn duration_ms(&self) -> f64 {
        self.data.duration.as_f64()
    }

    pub fn width(&self) -> u32 {
        self.data.config.coded_width as u32
    }

    pub fn height(&self) -> u32 {
        self.data.config.coded_height as u32
    }

    pub fn frame_at(&mut self, timestamp: TimeMs) -> GpuTexture2D {
        self.texture.clone()
    }
}
