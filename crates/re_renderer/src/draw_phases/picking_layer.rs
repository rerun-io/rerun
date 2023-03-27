use crate::{
    allocator::create_and_fill_uniform_buffer,
    global_bindings::FrameUniformBuffer,
    view_builder::ViewBuilder,
    wgpu_resources::{
        texture_row_data_info, GpuBindGroup, GpuTexture, TextureDesc, TextureRowDataInfo,
    },
    DebugLabel, GpuReadbackBuffer, GpuReadbackBufferIdentifier, RenderContext,
};

pub struct PickingLayerProcessor {
    pub picking_target: GpuTexture,
    picking_depth: GpuTexture,
    readback_buffer: GpuReadbackBuffer,
    bind_group_0: GpuBindGroup,
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
    /// TODO(andreas): This is a color format for the current WIP implementation. Will use [`wgpu::TextureFormat::Rgba32Uint`] later.
    pub const PICKING_LAYER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

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
        ctx: &mut RenderContext,
        view_name: &DebugLabel,
        screen_resolution: glam::UVec2,
        picking_rect_min: glam::UVec2,
        picking_rect_extent: u32,
        frame_uniform_buffer_content: &FrameUniformBuffer,
        enable_picking_target_sampling: bool,
    ) -> (Self, ScheduledPickingRect) {
        let row_info = texture_row_data_info(Self::PICKING_LAYER_FORMAT, picking_rect_extent);
        let buffer_size = row_info.bytes_per_row_padded * picking_rect_extent;
        let readback_buffer = ctx.gpu_readback_belt.lock().allocate(
            &ctx.device,
            &ctx.gpu_resources.buffers,
            buffer_size as u64,
        );

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

        let rect_min = picking_rect_min.as_vec2();
        let rect_max =
            rect_min + glam::vec2(picking_rect_extent as f32, picking_rect_extent as f32);
        let screen_resolution = screen_resolution.as_vec2();
        let rect_min_ndc = glam::vec2(
            rect_min.x / screen_resolution.x * 2.0 - 1.0,
            1.0 - rect_max.y / screen_resolution.y * 2.0,
        );
        let rect_max_ndc = glam::vec2(
            rect_max.x / screen_resolution.x * 2.0 - 1.0,
            1.0 - rect_min.y / screen_resolution.y * 2.0,
        );
        let rect_center = (rect_min_ndc + rect_max_ndc) * 0.5;
        let adjusted_projection_from_projection =
            glam::Mat4::from_scale(2.0 / (rect_max_ndc - rect_min_ndc).extend(1.0))
                * glam::Mat4::from_translation(-rect_center.extend(0.0));

        // Setup frame uniform buffer
        let previous_projection_from_world: glam::Mat4 =
            frame_uniform_buffer_content.projection_from_world.into();
        let previous_projection_from_view: glam::Mat4 =
            frame_uniform_buffer_content.projection_from_view.into();
        let frame_uniform_buffer_content = FrameUniformBuffer {
            projection_from_world: (adjusted_projection_from_projection
                * previous_projection_from_world)
                .into(),
            projection_from_view: (adjusted_projection_from_projection
                * previous_projection_from_view)
                .into(),
            ..*frame_uniform_buffer_content
        };

        let frame_uniform_buffer = create_and_fill_uniform_buffer(
            ctx,
            view_name
                .clone()
                .push_str(" - picking_layer frame uniform buffer"),
            frame_uniform_buffer_content,
        );

        let bind_group_0 = ctx.shared_renderer_data.global_bindings.create_bind_group(
            &mut ctx.gpu_resources,
            &ctx.device,
            frame_uniform_buffer,
        );

        let scheduled_rect = ScheduledPickingRect {
            identifier: readback_buffer.identifier,
            screen_position: picking_rect_min,
            extent: glam::uvec2(picking_rect_extent, picking_rect_extent),
            row_info,
        };

        (
            PickingLayerProcessor {
                bind_group_0,
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

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: view_name.clone().push_str(" - picking_layer pass").get(),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.picking_target.default_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: true, // Store for readback!
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
        });

        pass.set_bind_group(0, &self.bind_group_0, &[]);

        pass
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
