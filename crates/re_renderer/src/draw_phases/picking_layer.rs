use crate::{
    view_builder::ViewBuilder,
    wgpu_resources::{texture_row_data_info, GpuTexture, TextureDesc, TextureRowDataInfo},
    DebugLabel, GpuReadbackBuffer, GpuReadbackBufferIdentifier, RenderContext,
};

pub struct PickingLayerProcessor {
    pub picking_target: GpuTexture,
    picking_depth: GpuTexture,
    readback_buffer: GpuReadbackBuffer,
}

#[derive(Clone)]
pub struct ScheduledPickingRect {
    pub identifier: GpuReadbackBufferIdentifier,
    pub screen_position: glam::UVec2,
    pub extent: glam::UVec2,
    pub row_info: TextureRowDataInfo,
}

impl PickingLayerProcessor {
    /// The texture format used for the picking layer.
    pub const PICKING_LAYER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb; // TODO: Integers and stuff.

    pub const PICKING_LAYER_DEPTH_FORMAT: wgpu::TextureFormat =
        ViewBuilder::MAIN_TARGET_DEPTH_FORMAT;

    pub const PICKING_LAYER_MSAA_STATE: wgpu::MultisampleState = wgpu::MultisampleState {
        count: 1,
        mask: !0,
        alpha_to_coverage_enabled: false,
    };

    pub const PICKING_LAYER_DEPTH_STATE: Option<wgpu::DepthStencilState> =
        ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE;

    pub fn new(
        ctx: &RenderContext,
        view_name: &DebugLabel,
        picking_rect_min: glam::UVec2,
        picking_rect_extent: u32,
        enable_picking_target_sampling: bool,
    ) -> (Self, ScheduledPickingRect) {
        let row_info = texture_row_data_info(Self::PICKING_LAYER_FORMAT, picking_rect_extent);
        let buffer_size = row_info.bytes_per_row_padded * picking_rect_extent;
        let readback_buffer = ctx.gpu_readback_belt.lock().allocate(
            &ctx.device,
            &ctx.gpu_resources.buffers,
            buffer_size as u64,
        );

        // TODO: Handle out of bounds stuff.
        let mut picking_target_usage =
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
        picking_target_usage.set(
            wgpu::TextureUsages::TEXTURE_BINDING,
            enable_picking_target_sampling,
        );

        let picking_target = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: view_name.clone().push_str(" - PickingLayerProcessor"),
                size: wgpu::Extent3d {
                    width: picking_rect_extent,
                    height: picking_rect_extent,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::PICKING_LAYER_FORMAT,
                usage: picking_target_usage,
            },
        );
        let picking_depth = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: view_name.clone().push_str(" - picking_layer depth"),
                format: Self::PICKING_LAYER_DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                ..picking_target.creation_desc
            },
        );

        let scheduled_rect = ScheduledPickingRect {
            identifier: readback_buffer.identifier,
            screen_position: picking_rect_min,
            extent: glam::uvec2(picking_rect_extent, picking_rect_extent),
            row_info,
        };

        (
            PickingLayerProcessor {
                picking_target,
                picking_depth,
                readback_buffer,
            },
            scheduled_rect,
        )
    }

    pub fn begin_render_pass<'a>(
        &'a self,
        view_name: &DebugLabel,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> wgpu::RenderPass<'a> {
        crate::profile_function!();

        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: view_name.clone().push_str(" - picking_layer pass").get(),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.picking_target.default_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: false,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.picking_depth.default_view,
                depth_ops: Some(wgpu::Operations {
                    load: ViewBuilder::DEFAULT_DEPTH_CLEAR,
                    store: false,
                }),
                stencil_ops: None,
            }),
        })
    }

    pub fn end_render_pass(self, encoder: &mut wgpu::CommandEncoder) {
        self.readback_buffer.read_texture2d(
            encoder,
            wgpu::ImageCopyTexture {
                texture: &self.picking_target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            glam::uvec2(
                self.picking_target.texture.width(),
                self.picking_target.texture.height(),
            ),
        );
    }
}
