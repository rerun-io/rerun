//! Easy screenshot taking.
//!
//! Executes compositing into a special target if we want to take a screenshot.
//! This is necessary because `composite` is expected to write directly into the final output
//! from which we can't read back and of which we don't control the format.
//!
//! This comes with the perk that we can do extra things here if we want!
//!
//! TODO(andreas): One thing to add would be more anti-aliasing!
//! We could render the same image with subpixel moved camera in order to get super-sampling without hitting texture size limitations.
//! Or alternatively try to render the images in several tiles 🤔. In any case this would greatly improve quality!

use ecolor::Rgba;

use crate::{
    allocator::GpuReadbackUserData,
    wgpu_resources::{GpuTexture, TextureDesc, TextureRowDataInfo},
    DebugLabel, GpuReadbackBuffer, GpuReadbackIdentifier, RenderContext,
};

pub struct ScreenshotProcessor {
    screenshot_texture: GpuTexture,
    screenshot_readback_buffer: GpuReadbackBuffer,
}

impl ScreenshotProcessor {
    /// The texture format used for screenshots.
    pub const SCREENSHOT_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    pub fn new(
        ctx: &RenderContext,
        view_name: &DebugLabel,
        resolution: glam::UVec2,
        readback_identifier: GpuReadbackIdentifier,
        readback_user_data: GpuReadbackUserData,
    ) -> Self {
        let row_info = TextureRowDataInfo::new(Self::SCREENSHOT_COLOR_FORMAT, resolution.x);
        let buffer_size = row_info.bytes_per_row_padded * resolution.y;
        let screenshot_readback_buffer = ctx.gpu_readback_belt.lock().allocate(
            &ctx.device,
            &ctx.gpu_resources.buffers,
            buffer_size as u64,
            readback_identifier,
            readback_user_data,
        );

        let screenshot_texture = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: format!("{view_name} - ScreenshotProcessor").into(),
                size: wgpu::Extent3d {
                    width: resolution.x,
                    height: resolution.y,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::SCREENSHOT_COLOR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            },
        );

        Self {
            screenshot_texture,
            screenshot_readback_buffer,
        }
    }

    pub fn begin_render_pass<'a>(
        &'a self,
        view_name: &DebugLabel,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> wgpu::RenderPass<'a> {
        crate::profile_function!();

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: DebugLabel::from(format!("{view_name} - screenshot")).get(),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.screenshot_texture.default_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        pass
    }

    pub fn end_render_pass(self, encoder: &mut wgpu::CommandEncoder) {
        self.screenshot_readback_buffer.read_texture2d(
            encoder,
            wgpu::ImageCopyTexture {
                texture: &self.screenshot_texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            glam::uvec2(
                self.screenshot_texture.texture.width(),
                self.screenshot_texture.texture.height(),
            ),
        );
    }
}
