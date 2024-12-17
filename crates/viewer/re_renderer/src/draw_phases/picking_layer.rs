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
    include_shader_module,
    rect::RectF32,
    texture_info::Texture2DBufferInfo,
    transform::{ndc_from_pixel, RectTransform},
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuRenderPipelineHandle,
        GpuRenderPipelinePoolAccessor, GpuTexture, GpuTextureHandle, PipelineLayoutDesc, PoolError,
        RenderPipelineDesc, TextureDesc,
    },
    DebugLabel, GpuReadbackBuffer, GpuReadbackIdentifier, RectInt, RenderContext,
};

use parking_lot::Mutex;
use smallvec::smallvec;

/// GPU retrieved & processed picking data result.
pub struct PickingResult<T: 'static + Send + Sync> {
    /// User data supplied on picking request.
    pub user_data: T,

    /// Picking rect supplied on picking request.
    /// Describes the area of the picking layer that was read back.
    pub rect: RectInt,

    /// Picking id data for the requested rectangle.
    ///
    /// GPU internal row padding has already been removed from this buffer.
    /// Pixel data is stored in the normal fashion - row wise, left to right, top to bottom.
    pub picking_id_data: Vec<PickingLayerId>,

    /// Picking depth data for the requested rectangle.
    ///
    /// Use [`PickingResult::picked_world_position`] for easy interpretation of the data.
    ///
    /// GPU internal row padding has already been removed from this buffer.
    /// Pixel data is stored in the normal fashion - row wise, left to right, top to bottom.
    pub picking_depth_data: Vec<f32>,

    /// Transforms a NDC position on the picking rect to a world position.
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
            ndc_from_pixel(pos_on_picking_rect.as_vec2(), self.rect.extent).extend(raw_depth),
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
    picking_rect: RectInt,
    world_from_cropped_projection: glam::Mat4,
    user_data: T,

    depth_readback_workaround_in_use: bool,
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

#[derive(thiserror::Error, Debug)]
pub enum PickingLayerError {
    #[error(transparent)]
    ReadbackError(#[from] crate::allocator::GpuReadbackError),

    #[error(transparent)]
    ResourcePoolError(#[from] PoolError),
}

/// Manages the rendering of the picking layer pass, its render targets & readback buffer.
///
/// The view builder creates this for every frame that requests a picking result.
pub struct PickingLayerProcessor {
    pub picking_target: GpuTexture,
    picking_depth_target: GpuTexture,
    readback_buffer: Mutex<GpuReadbackBuffer>,
    bind_group_0: GpuBindGroup,

    depth_readback_workaround: Option<DepthReadbackWorkaround>,
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
        ctx: &RenderContext,
        view_name: &DebugLabel,
        screen_resolution: glam::UVec2,
        picking_rect: RectInt,
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

        let direct_depth_readback = ctx.device_caps().tier.support_depth_readback();

        let picking_depth_target = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: format!("{view_name} - picking_layer depth target").into(),
                format: Self::PICKING_LAYER_DEPTH_FORMAT,
                usage: if direct_depth_readback {
                    wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC
                } else {
                    wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING
                },
                ..picking_target.creation_desc
            },
        );

        let depth_readback_workaround = (!direct_depth_readback).then(|| {
            DepthReadbackWorkaround::new(ctx, picking_rect.extent, picking_depth_target.handle)
        });

        let cropped_projection_from_projection = RectTransform {
            region_of_interest: picking_rect.into(),
            region: RectF32 {
                min: glam::Vec2::ZERO,
                extent: screen_resolution.as_vec2(),
            },
        }
        .to_ndc_scale_and_translation();

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

        let bind_group_0 = ctx.global_bindings.create_bind_group(
            &ctx.gpu_resources,
            &ctx.device,
            frame_uniform_buffer,
        );

        let row_info_id =
            Texture2DBufferInfo::new(Self::PICKING_LAYER_FORMAT, picking_rect.wgpu_extent());
        let row_info_depth = Texture2DBufferInfo::new(
            if direct_depth_readback {
                Self::PICKING_LAYER_DEPTH_FORMAT
            } else {
                DepthReadbackWorkaround::READBACK_FORMAT
            },
            picking_rect.wgpu_extent(),
        );

        // Offset of the depth buffer in the readback buffer needs to be aligned to size of a depth pixel.
        // This is "trivially true" if the size of the depth format is a multiple of the size of the id format.
        debug_assert!(
            Self::PICKING_LAYER_FORMAT.block_copy_size(None).unwrap()
                % Self::PICKING_LAYER_DEPTH_FORMAT
                    .block_copy_size(Some(wgpu::TextureAspect::DepthOnly))
                    .unwrap()
                == 0
        );
        let buffer_size = row_info_id.buffer_size_padded + row_info_depth.buffer_size_padded;

        let readback_buffer = Mutex::new(ctx.gpu_readback_belt.lock().allocate(
            &ctx.device,
            &ctx.gpu_resources.buffers,
            buffer_size,
            readback_identifier,
            Box::new(ReadbackBeltMetadata {
                picking_rect,
                user_data: readback_user_data,
                world_from_cropped_projection: cropped_projection_from_world.inverse(),
                depth_readback_workaround_in_use: depth_readback_workaround.is_some(),
            }),
        ));

        Self {
            bind_group_0,
            picking_target,
            picking_depth_target,
            readback_buffer,
            depth_readback_workaround,
        }
    }

    pub fn begin_render_pass<'a>(
        &'a self,
        view_name: &DebugLabel,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> wgpu::RenderPass<'a> {
        re_tracing::profile_function!();

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: DebugLabel::from(format!("{view_name} - picking_layer pass")).get(),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.picking_target.default_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store, // Store for readback!
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.picking_depth_target.default_view,
                depth_ops: Some(wgpu::Operations {
                    load: ViewBuilder::DEFAULT_DEPTH_CLEAR,
                    store: wgpu::StoreOp::Store, // Store for readback!
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_bind_group(0, &self.bind_group_0, &[]);

        pass
    }

    pub fn end_render_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_pipelines: &GpuRenderPipelinePoolAccessor<'_>,
    ) -> Result<(), PickingLayerError> {
        let extent = self.picking_target.texture.size();

        let readable_depth_texture =
            if let Some(depth_copy_workaround) = self.depth_readback_workaround.as_ref() {
                depth_copy_workaround.copy_to_readable_texture(
                    encoder,
                    render_pipelines,
                    &self.bind_group_0,
                )?
            } else {
                &self.picking_depth_target
            };

        self.readback_buffer.lock().read_multiple_texture2d(
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
                        texture: &readable_depth_texture.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: if self.depth_readback_workaround.is_some() {
                            wgpu::TextureAspect::All
                        } else {
                            wgpu::TextureAspect::DepthOnly
                        },
                    },
                    extent,
                ),
            ],
        )?;

        Ok(())
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
                // Assert that our texture data reinterpretation works out from a pixel size point of view.
                debug_assert_eq!(
                    Self::PICKING_LAYER_DEPTH_FORMAT
                        .block_copy_size(Some(wgpu::TextureAspect::DepthOnly))
                        .unwrap(),
                    std::mem::size_of::<f32>() as u32
                );
                debug_assert_eq!(
                    Self::PICKING_LAYER_FORMAT.block_copy_size(None).unwrap() as usize,
                    std::mem::size_of::<PickingLayerId>()
                );

                let buffer_info_id = Texture2DBufferInfo::new(
                    Self::PICKING_LAYER_FORMAT,
                    metadata.picking_rect.wgpu_extent(),
                );
                let buffer_info_depth = Texture2DBufferInfo::new(
                    if metadata.depth_readback_workaround_in_use {
                        DepthReadbackWorkaround::READBACK_FORMAT
                    } else {
                        Self::PICKING_LAYER_DEPTH_FORMAT
                    },
                    metadata.picking_rect.wgpu_extent(),
                );

                let picking_id_data = buffer_info_id
                    .remove_padding_and_convert(&data[..buffer_info_id.buffer_size_padded as _]);
                let mut picking_depth_data = buffer_info_depth
                    .remove_padding_and_convert(&data[buffer_info_id.buffer_size_padded as _..]);

                if metadata.depth_readback_workaround_in_use {
                    // Can't read back depth textures & can't read back R32Float textures either!
                    // See https://github.com/gfx-rs/wgpu/issues/3644
                    debug_assert_eq!(
                        DepthReadbackWorkaround::READBACK_FORMAT
                            .block_copy_size(None)
                            .unwrap() as usize,
                        std::mem::size_of::<f32>() * 4
                    );
                    picking_depth_data = picking_depth_data.into_iter().step_by(4).collect();
                }

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

/// Utility for copying a depth texture when it can't be read-back directly to a [`wgpu::TextureFormat::R32Float`] which is readable texture.
///
/// Implementation note:
/// This is a plain & simple "sample in shader and write to texture" utility.
/// It might be worth abstracting this further into a general purpose operator.
/// There is not much in here that is specific to the depth usecase!
struct DepthReadbackWorkaround {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group: GpuBindGroup,
    readable_texture: GpuTexture,
}

impl DepthReadbackWorkaround {
    /// There's two layers of workarounds here:
    /// * WebGL (via spec) not being able to read back depth textures
    /// * unclear behavior for any readback that isn't RGBA
    ///     Furthermore, integer textures also seemed to be problematic,
    ///     but it seems to work fine for [`wgpu::TextureFormat::Rgba32Uint`] which we use for our picking ID
    ///     Details see [wgpu#3644](https://github.com/gfx-rs/wgpu/issues/3644)
    const READBACK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

    fn new(
        ctx: &RenderContext,
        extent: glam::UVec2,
        depth_target_handle: GpuTextureHandle,
    ) -> Self {
        let readable_texture = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: "DepthCopyWorkaround::readable_texture".into(),
                format: Self::READBACK_FORMAT,
                usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
                size: wgpu::Extent3d {
                    width: extent.x,
                    height: extent.y,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
            },
        );

        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "DepthCopyWorkaround::bind_group_layout".into(),
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            },
        );

        let bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label: "DepthCopyWorkaround::bind_group".into(),
                entries: smallvec![BindGroupEntry::DefaultTextureView(depth_target_handle)],
                layout: bind_group_layout,
            },
        );

        let render_pipeline = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "DepthCopyWorkaround::render_pipeline".into(),
                pipeline_layout: ctx.gpu_resources.pipeline_layouts.get_or_create(
                    ctx,
                    &PipelineLayoutDesc {
                        label: "DepthCopyWorkaround::render_pipeline".into(),
                        entries: vec![ctx.global_bindings.layout, bind_group_layout],
                    },
                ),
                vertex_entrypoint: "main".into(),
                vertex_handle: ctx.gpu_resources.shader_modules.get_or_create(
                    ctx,
                    &include_shader_module!("../../shader/screen_triangle.wgsl"),
                ),
                fragment_entrypoint: "main".into(),
                fragment_handle: ctx.gpu_resources.shader_modules.get_or_create(
                    ctx,
                    &include_shader_module!("../../shader/copy_texture.wgsl"),
                ),
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(readable_texture.texture.format().into())],
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            },
        );

        Self {
            render_pipeline,
            bind_group,
            readable_texture,
        }
    }

    fn copy_to_readable_texture(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_pipelines: &GpuRenderPipelinePoolAccessor<'_>,
        global_binding_bind_group: &GpuBindGroup,
    ) -> Result<&GpuTexture, PoolError> {
        // Copy depth texture to a readable (color) texture with a screen filling triangle.
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: DebugLabel::from("Depth copy workaround").get(),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.readable_texture.default_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store, // Store for readback!
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        let pipeline = render_pipelines.get(self.render_pipeline)?;
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, global_binding_bind_group, &[]);
        pass.set_bind_group(1, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);

        Ok(&self.readable_texture)
    }
}
