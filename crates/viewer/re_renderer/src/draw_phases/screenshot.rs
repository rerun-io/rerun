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

use re_mutex::Mutex;

use crate::allocator::GpuReadbackError;
use crate::texture_info::Texture2DBufferInfo;
use crate::wgpu_resources::{GpuTexture, TextureDesc};
use crate::{DebugLabel, GpuReadbackBuffer, GpuReadbackIdentifier, RenderContext};

/// Type used as user data on the gpu readback belt.
struct ReadbackBeltMetadata<T: 'static + Send + Sync> {
    extent: wgpu::Extent3d,
    user_data: T,
}

pub struct ScreenshotProcessor {
    screenshot_texture: GpuTexture,
    screenshot_readback_buffer: Mutex<GpuReadbackBuffer>,
}

impl ScreenshotProcessor {
    /// The texture format used for screenshots.
    pub const SCREENSHOT_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    pub fn new<T: 'static + Send + Sync>(
        ctx: &RenderContext,
        view_name: &DebugLabel,
        resolution: glam::UVec2,
        readback_identifier: GpuReadbackIdentifier,
        readback_user_data: T,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: resolution.x,
            height: resolution.y,
            depth_or_array_layers: 1,
        };
        let buffer_info = Texture2DBufferInfo::new(Self::SCREENSHOT_COLOR_FORMAT, size);
        let screenshot_readback_buffer = Mutex::new(ctx.gpu_readback_belt.lock().allocate(
            &ctx.device,
            &ctx.gpu_resources.buffers,
            buffer_info.buffer_size_padded,
            readback_identifier,
            Box::new(ReadbackBeltMetadata {
                extent: size,
                user_data: readback_user_data,
            }),
        ));

        let screenshot_texture = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: format!("{view_name} - ScreenshotProcessor").into(),
                size,
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
        re_tracing::profile_function!();

        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: DebugLabel::from(format!("{view_name} - screenshot")).get(),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.screenshot_texture.default_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        })
    }

    pub fn end_render_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<(), GpuReadbackError> {
        self.screenshot_readback_buffer.lock().read_texture2d(
            encoder,
            wgpu::TexelCopyTextureInfo {
                texture: &self.screenshot_texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            self.screenshot_texture.texture.size(),
        )
    }

    /// Returns the oldest received screenshot results for a given identifier and user data type.
    ///
    /// It is recommended to call this method repeatedly until it returns `None` to ensure that all
    /// pending data is flushed.
    ///
    /// Ready data that hasn't been retrieved for more than a frame will be discarded.
    ///
    /// See also [`crate::view_builder::ViewBuilder::schedule_screenshot`]
    pub fn next_readback_result<T: 'static + Send + Sync>(
        ctx: &RenderContext,
        identifier: GpuReadbackIdentifier,
        on_screenshot: impl FnOnce(&[u8], glam::UVec2, T),
    ) -> Option<()> {
        ctx.gpu_readback_belt.lock().readback_next_available(
            identifier,
            |data: &[u8], metadata: Box<ReadbackBeltMetadata<T>>| {
                let buffer_info =
                    Texture2DBufferInfo::new(Self::SCREENSHOT_COLOR_FORMAT, metadata.extent);
                let texture_data = buffer_info.remove_padding(data);
                on_screenshot(
                    &texture_data,
                    glam::uvec2(metadata.extent.width, metadata.extent.height),
                    metadata.user_data,
                );
            },
        )
    }
}
