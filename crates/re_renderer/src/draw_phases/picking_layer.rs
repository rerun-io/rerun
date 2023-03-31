//! GPU driven picking.
//!
//! This module provides the [`PickingLayerProcessor`] which is responsible for rendering & processing the picking layer.
//! Picking is done in a separate pass to a as-small-as needed render target (size is user configurable).
//!
//! The picking layer is a RGBA texture with 32bit per channel, the red & green channel are used for the [`PickingLayerObjectId`],
//! the blue & alpha channel are used for the [`PickingLayerInstanceId`].
//! (Keep in mind that GPUs are little endian, so R will have the lower bytes and G the higher ones)
//!
//! In order to accomplish small render targets, the projection matrix is cropped to only render the area of interest.

use crate::{
    allocator::create_and_fill_uniform_buffer,
    global_bindings::FrameUniformBuffer,
    view_builder::ViewBuilder,
    wgpu_resources::{GpuBindGroup, GpuTexture, Texture2DBufferInfo, TextureDesc},
    DebugLabel, GpuReadbackBuffer, GpuReadbackIdentifier, IntRect, RenderContext,
};

/// GPU retrieved & processed picking data result.
pub struct PickingResult<T: 'static + Send + Sync> {
    /// User data supplied on picking request.
    pub user_data: T,

    /// Picking rect supplied on picking request.
    /// Describes the area of the picking layer that was read back.
    pub rect: IntRect,

    /// Picking id data for the requested rectangle.
    ///
    /// GPU internal row padding has already been removed.
    /// Data is stored row wise, left to right, top to bottom.
    pub picking_id_data: Vec<PickingLayerId>,

    /// Picking depth data for the requested rectangle.
    /// TODO: refer to utility for interpretation
    ///
    /// GPU internal row padding has already been removed.
    /// Data is stored row wise, left to right, top to bottom.
    pub picking_depth_data: Vec<f32>,

    /// Transforms a position on the picking rect to a world position.
    world_from_cropped_projection: glam::Mat4,
}

impl<T: 'static + Send + Sync> PickingResult<T> {
    /// Returns the picked world position.
    ///
    /// Panics if the position is outside of the picking rect.
    ///
    /// Keep in mind that the picked position may be (negative) infinity if nothing was picked.
    #[inline]
    pub fn picked_world_position(&self, pos_on_picking_rect: glam::UVec2) -> glam::Vec3 {
        let raw_depth = self.picking_depth_data
            [(pos_on_picking_rect.y * self.rect.width() + pos_on_picking_rect.x) as usize];

        self.world_from_cropped_projection.project_point3(
            pixel_coord_to_ndc(pos_on_picking_rect.as_vec2(), self.rect.extent.as_vec2())
                .extend(raw_depth),
        )
    }

    /// Returns the picked picking id.
    ///
    /// Panics if the position is outside of the picking rect.
    #[inline]
    pub fn picked_id(&self, pos_on_picking_rect: glam::UVec2) -> PickingLayerId {
        self.picking_id_data
            [(pos_on_picking_rect.y * self.rect.width() + pos_on_picking_rect.x) as usize]
    }
}

/// Type used as user data on the gpu readback belt.
struct ReadbackBeltMetadata<T: 'static + Send + Sync> {
    picking_rect: IntRect,
    world_from_cropped_projection: glam::Mat4,
    user_data: T,
}

/// The first 64bit of the picking layer.
///
/// Typically used to identify higher level objects
/// Some renderers might allow to change this part of the picking identifier only at a coarse grained level.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod, Default, Debug, PartialEq, Eq)]
pub struct PickingLayerObjectId(pub u64);

/// The second 64bit of the picking layer.
///
/// Typically used to identify instances.
/// Some renderers might allow to change only this part of the picking identifier at a fine grained level.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod, Default, Debug, PartialEq, Eq)]
pub struct PickingLayerInstanceId(pub u64);

/// Combination of `PickingLayerObjectId` and `PickingLayerInstanceId`.
///
/// This is the same memory order as it is found in the GPU picking layer texture.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod, Default, Debug, PartialEq, Eq)]
pub struct PickingLayerId {
    pub object: PickingLayerObjectId,
    pub instance: PickingLayerInstanceId,
}

impl From<PickingLayerId> for [u32; 4] {
    fn from(val: PickingLayerId) -> Self {
        [
            val.object.0 as u32,
            (val.object.0 >> 32) as u32,
            val.instance.0 as u32,
            (val.instance.0 >> 32) as u32,
        ]
    }
}

/// Converts a pixel coordinate to normalized device coordinates.
fn pixel_coord_to_ndc(coord: glam::Vec2, target_resolution: glam::Vec2) -> glam::Vec2 {
    glam::vec2(
        coord.x / target_resolution.x * 2.0 - 1.0,
        1.0 - coord.y / target_resolution.y * 2.0,
    )
}

/// Manages the rendering of the picking layer pass, its render targets & readback buffer.
///
/// The view builder creates this for every frame that requests a picking result.
pub struct PickingLayerProcessor {
    pub picking_target: GpuTexture,
    picking_depth: GpuTexture,
    readback_buffer: GpuReadbackBuffer,
    bind_group_0: GpuBindGroup,
}

impl PickingLayerProcessor {
    /// The texture format used for the picking layer.
    pub const PICKING_LAYER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Uint;
    /// The depth format used for the picking layer - f32 makes it easiest to deal with retrieved depth and is guaranteed to be copyable.
    pub const PICKING_LAYER_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub const PICKING_LAYER_MSAA_STATE: wgpu::MultisampleState = wgpu::MultisampleState {
        count: 1,
        mask: !0,
        alpha_to_coverage_enabled: false,
    };

    pub const PICKING_LAYER_DEPTH_STATE: Option<wgpu::DepthStencilState> =
        ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE;

    /// New picking layer for a given screen.
    ///
    /// Note that out-of-bounds rectangles *are* allowed, the picking layer will *not* be clipped to the screen.
    /// This means that the content of the picking layer rectangle will behave as-if the screen was bigger,
    /// containing valid picking data.
    /// It's up to the user when interpreting the picking data to do any required clipping.
    ///
    /// `enable_picking_target_sampling` should be enabled only for debugging purposes.
    /// It allows to sample the picking layer texture in a shader.
    #[allow(clippy::too_many_arguments)]
    pub fn new<T: 'static + Send + Sync>(
        ctx: &mut RenderContext,
        view_name: &DebugLabel,
        screen_resolution: glam::UVec2,
        picking_rect: IntRect,
        frame_uniform_buffer_content: &FrameUniformBuffer,
        enable_picking_target_sampling: bool,
        readback_identifier: GpuReadbackIdentifier,
        readback_user_data: T,
    ) -> Self {
        let mut picking_target_usage =
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
        picking_target_usage.set(
            wgpu::TextureUsages::TEXTURE_BINDING,
            enable_picking_target_sampling,
        );

        let picking_target = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: format!("{view_name} - PickingLayerProcessor").into(),
                size: picking_rect.wgpu_extent(),
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
                label: format!("{view_name} - picking_layer depth").into(),
                format: Self::PICKING_LAYER_DEPTH_FORMAT,
                ..picking_target.creation_desc
            },
        );

        let rect_min = picking_rect.top_left_corner.as_vec2();
        let rect_max = rect_min + picking_rect.extent.as_vec2();
        let screen_resolution = screen_resolution.as_vec2();
        // y axis is flipped in NDC, therefore we need to flip the y axis of the rect.
        let rect_min_ndc =
            pixel_coord_to_ndc(glam::vec2(rect_min.x, rect_max.y), screen_resolution);
        let rect_max_ndc =
            pixel_coord_to_ndc(glam::vec2(rect_max.x, rect_min.y), screen_resolution);
        let rect_center_ndc = (rect_min_ndc + rect_max_ndc) * 0.5;
        let cropped_projection_from_projection =
            glam::Mat4::from_scale(2.0 / (rect_max_ndc - rect_min_ndc).extend(1.0))
                * glam::Mat4::from_translation(-rect_center_ndc.extend(0.0));

        // Setup frame uniform buffer
        let previous_projection_from_world: glam::Mat4 =
            frame_uniform_buffer_content.projection_from_world.into();
        let cropped_projection_from_world =
            cropped_projection_from_projection * previous_projection_from_world;
        let previous_projection_from_view: glam::Mat4 =
            frame_uniform_buffer_content.projection_from_view.into();
        let cropped_projection_from_view =
            cropped_projection_from_projection * previous_projection_from_view;

        let frame_uniform_buffer_content = FrameUniformBuffer {
            projection_from_world: cropped_projection_from_world.into(),
            projection_from_view: cropped_projection_from_view.into(),
            ..*frame_uniform_buffer_content
        };

        let frame_uniform_buffer = create_and_fill_uniform_buffer(
            ctx,
            format!("{view_name} - picking_layer frame uniform buffer").into(),
            frame_uniform_buffer_content,
        );

        let bind_group_0 = ctx.shared_renderer_data.global_bindings.create_bind_group(
            &mut ctx.gpu_resources,
            &ctx.device,
            frame_uniform_buffer,
        );

        let row_info_id = Texture2DBufferInfo::new(Self::PICKING_LAYER_FORMAT, picking_rect.extent);
        let row_info_depth =
            Texture2DBufferInfo::new(Self::PICKING_LAYER_DEPTH_FORMAT, picking_rect.extent);

        // Offset of the depth buffer in the readback buffer needs to be aligned to size of a depth pixel.
        // This is "trivially true" if the size of the depth format is a multiple of the size of the id format.
        debug_assert!(
            Self::PICKING_LAYER_FORMAT.describe().block_size
                % Self::PICKING_LAYER_DEPTH_FORMAT.describe().block_size
                == 0
        );
        let buffer_size = row_info_id.buffer_size_padded + row_info_depth.buffer_size_padded;

        let readback_buffer = ctx.gpu_readback_belt.lock().allocate(
            &ctx.device,
            &ctx.gpu_resources.buffers,
            buffer_size,
            readback_identifier,
            Box::new(ReadbackBeltMetadata {
                picking_rect,
                user_data: readback_user_data,
                world_from_cropped_projection: cropped_projection_from_world.inverse(),
            }),
        );

        PickingLayerProcessor {
            bind_group_0,
            picking_target,
            picking_depth,
            readback_buffer,
        }
    }

    pub fn begin_render_pass<'a>(
        &'a self,
        view_name: &DebugLabel,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> wgpu::RenderPass<'a> {
        crate::profile_function!();

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: DebugLabel::from(format!("{view_name} - picking_layer pass")).get(),
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
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        pass.set_bind_group(0, &self.bind_group_0, &[]);

        pass
    }

    pub fn end_render_pass(self, encoder: &mut wgpu::CommandEncoder) {
        let extent = glam::uvec2(
            self.picking_target.texture.width(),
            self.picking_target.texture.height(),
        );
        self.readback_buffer.read_multiple_texture2d(
            encoder,
            &[
                (
                    wgpu::ImageCopyTexture {
                        texture: &self.picking_target.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    extent,
                ),
                (
                    wgpu::ImageCopyTexture {
                        texture: &self.picking_depth.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::DepthOnly,
                    },
                    extent,
                ),
            ],
        );
    }

    /// Returns the oldest received picking results for a given identifier and user data type.
    ///
    /// It is recommended to call this method repeatedly until it returns `None` to ensure that all
    /// pending data is flushed.
    ///
    /// Ready data that hasn't been retrieved for more than a frame will be discarded.
    ///
    /// See also [`crate::view_builder::ViewBuilder::schedule_picking_rect`]
    pub fn next_readback_result<T: 'static + Send + Sync>(
        ctx: &RenderContext,
        identifier: GpuReadbackIdentifier,
    ) -> Option<PickingResult<T>> {
        let mut result = None;
        ctx.gpu_readback_belt
            .lock()
            .readback_data::<ReadbackBeltMetadata<T>>(identifier, |data, metadata| {
                let buffer_info_id = Texture2DBufferInfo::new(
                    Self::PICKING_LAYER_FORMAT,
                    metadata.picking_rect.extent,
                );
                let buffer_info_depth = Texture2DBufferInfo::new(
                    Self::PICKING_LAYER_DEPTH_FORMAT,
                    metadata.picking_rect.extent,
                );

                let picking_id_data = buffer_info_id
                    .remove_padding_and_convert(&data[..buffer_info_id.buffer_size_padded as _]);
                let picking_depth_data = buffer_info_depth
                    .remove_padding_and_convert(&data[buffer_info_id.buffer_size_padded as _..]);

                result = Some(PickingResult {
                    picking_id_data,
                    picking_depth_data,
                    user_data: metadata.user_data,
                    rect: metadata.picking_rect,
                    world_from_cropped_projection: metadata.world_from_cropped_projection,
                });
            });
        result
    }
}
