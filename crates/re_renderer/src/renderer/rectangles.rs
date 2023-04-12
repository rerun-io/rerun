//! Renderer that makes it easy to draw textured 2d rectangles with transparency
//!
//! Transparency: (TODO(andreas):)
//! We're not performing any sorting on transparency yet, so the transparent rectangles pretty much
//! only work correctly when they are directly layered in front of another opaque rectangle.
//! We do *not* disable depth write.
//!
//! Implementation details:
//! We assume the standard usecase are individual textured rectangles.
//! Since we're not allowed to bind many textures at once (no widespread bindless support!),
//! we are forced to have individual bind groups per rectangle and thus a draw call per rectangle.

use itertools::{izip, Itertools as _};
use smallvec::smallvec;

use crate::{
    allocator::create_and_fill_uniform_buffer_batch,
    depth_offset::DepthOffset,
    draw_phases::{DrawPhase, OutlineMaskProcessor},
    include_shader_module,
    resource_managers::{GpuTexture2DHandle, ResourceManagerError},
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, PipelineLayoutDesc, RenderPipelineDesc, SamplerDesc,
    },
    ColorMap, OutlineMaskPreference, PickingLayerProcessor, Rgba,
};

use super::{
    DrawData, FileResolver, FileSystem, RenderContext, Renderer, SharedRendererData,
    WgpuResourcePools,
};

mod gpu_data {
    use crate::wgpu_buffer_types;

    // Keep in sync with mirror in rectangle.wgsl
    #[repr(C, align(256))]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct UniformBuffer {
        pub top_left_corner_position: wgpu_buffer_types::Vec3Unpadded,
        /// 0=disabled, else see `colormap.rs`
        pub colormap: u32,

        pub extent_u: wgpu_buffer_types::Vec3Unpadded,
        pub sample_type: u32, // 1=float, 2=depth, 3=sint, 4=uint

        pub extent_v: wgpu_buffer_types::Vec3Unpadded,
        pub depth_offset: f32,

        pub multiplicative_tint: crate::Rgba,
        pub outline_mask: wgpu_buffer_types::UVec2,

        /// Range of the texture values.
        /// Will be mapped to the [0, 1] range before we colormap.
        pub range_min_max: wgpu_buffer_types::Vec2,

        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 5],
    }

    impl UniformBuffer {
        pub fn from_textured_rect(
            rectangle: &super::TexturedRect,
            texture_format: &wgpu::TextureFormat,
        ) -> Self {
            let texture_info = texture_format.describe();

            let super::ColormappedTexture {
                texture: _,
                range,
                colormap,
            } = rectangle.colormapped_texture;

            let sample_type = match texture_info.sample_type {
                // The number here must match the shader!
                wgpu::TextureSampleType::Float { .. } => 1,
                wgpu::TextureSampleType::Depth => 2,
                wgpu::TextureSampleType::Sint => 3,
                wgpu::TextureSampleType::Uint => 4,
            };

            let colormap = if texture_info.components == 1 {
                colormap as u32
            } else {
                0 // RGBA doesn't need a colormap
            };

            Self {
                top_left_corner_position: rectangle.top_left_corner_position.into(),
                colormap,
                extent_u: rectangle.extent_u.into(),
                sample_type,
                extent_v: rectangle.extent_v.into(),
                depth_offset: rectangle.depth_offset as f32,
                multiplicative_tint: rectangle.multiplicative_tint,
                outline_mask: rectangle.outline_mask.0.unwrap_or_default().into(),
                range_min_max: range.into(),
                end_padding: Default::default(),
            }
        }
    }
}

/// Texture filter setting for magnification (a texel covers several pixels).
#[derive(Debug)]
pub enum TextureFilterMag {
    Linear,
    Nearest,
    // TODO(andreas): Offer advanced (shader implemented) filters like cubic?
}

/// Texture filter setting for minification (several texels fall to one pixel).
#[derive(Debug)]
pub enum TextureFilterMin {
    Linear,
    Nearest,
    // TODO(andreas): Offer mipmapping here?
}

/// Describes a texture and how to map it to a color.
pub struct ColormappedTexture {
    pub texture: GpuTexture2DHandle,

    /// Min/max range of the values in the texture.
    /// Used for colormapping (if any).
    pub range: [f32; 2],

    /// The colormap to apply to single-component textures.
    pub colormap: ColorMap,
}

impl Default for ColormappedTexture {
    fn default() -> Self {
        Self {
            texture: GpuTexture2DHandle::invalid(),
            range: [0.0, 1.0],
            colormap: ColorMap::default(), // Whatever
        }
    }
}

impl ColormappedTexture {
    pub fn from_srgba_unorm(texture: GpuTexture2DHandle) -> Self {
        Self {
            texture,
            range: [0.0, 1.0],
            colormap: ColorMap::default(), // Unused
        }
    }
}

pub struct TexturedRect {
    /// Top left corner position in world space.
    pub top_left_corner_position: glam::Vec3,

    /// Vector that spans up the rectangle from its top left corner along the u axis of the texture.
    pub extent_u: glam::Vec3,

    /// Vector that spans up the rectangle from its top left corner along the v axis of the texture.
    pub extent_v: glam::Vec3,

    /// Texture that fills the rectangle
    pub colormapped_texture: ColormappedTexture,

    pub texture_filter_magnification: TextureFilterMag,
    pub texture_filter_minification: TextureFilterMin,

    /// Tint that is multiplied to the rect, supports pre-multiplied alpha.
    pub multiplicative_tint: Rgba,

    pub depth_offset: DepthOffset,

    /// Optional outline mask.
    pub outline_mask: OutlineMaskPreference,
}

impl Default for TexturedRect {
    fn default() -> Self {
        Self {
            top_left_corner_position: glam::Vec3::ZERO,
            extent_u: glam::Vec3::ZERO,
            extent_v: glam::Vec3::ZERO,
            colormapped_texture: Default::default(),
            texture_filter_magnification: TextureFilterMag::Nearest,
            texture_filter_minification: TextureFilterMin::Linear,
            multiplicative_tint: Rgba::WHITE,
            depth_offset: 0,
            outline_mask: OutlineMaskPreference::NONE,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RectangleError {
    #[error(transparent)]
    ResourceManagerError(#[from] ResourceManagerError),

    #[error("Texture required special features: {0:?}")]
    SpecialFeatures(wgpu::Features),

    // There's really no need for users to be able to sample depth textures.
    // We don't get filtering of depth textures any way.
    #[error("Depth textures not supported - use float or integer textures instead.")]
    DepthTexturesNotSupported,
}

#[derive(Clone)]
struct RectangleInstance {
    bind_group: GpuBindGroup,
    draw_outline_mask: bool,
}

#[derive(Clone)]
pub struct RectangleDrawData {
    instances: Vec<RectangleInstance>,
}

impl DrawData for RectangleDrawData {
    type Renderer = RectangleRenderer;
}

impl RectangleDrawData {
    pub fn new(
        ctx: &mut RenderContext,
        rectangles: &[TexturedRect],
    ) -> Result<Self, RectangleError> {
        crate::profile_function!();

        let mut renderers = ctx.renderers.write();
        let rectangle_renderer = renderers.get_or_create::<_, RectangleRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.gpu_resources,
            &ctx.device,
            &mut ctx.resolver,
        );

        if rectangles.is_empty() {
            return Ok(RectangleDrawData {
                instances: Vec::new(),
            });
        }

        // TODO(emilk): continue on error (skipping just that rectangle)?
        let textures: Vec<_> = rectangles
            .iter()
            .map(|rectangle| {
                ctx.texture_manager_2d
                    .get(&rectangle.colormapped_texture.texture)
            })
            .try_collect()?;

        let uniform_buffers = izip!(rectangles, &textures)
            .map(|(rect, texture)| {
                gpu_data::UniformBuffer::from_textured_rect(rect, &texture.creation_desc.format)
            })
            .collect_vec();

        let uniform_buffer_bindings = create_and_fill_uniform_buffer_batch(
            ctx,
            "rectangle uniform buffers".into(),
            uniform_buffers.into_iter(),
        );

        let mut instances = Vec::with_capacity(rectangles.len());
        for (rectangle, uniform_buffer, texture) in
            izip!(rectangles, uniform_buffer_bindings, textures)
        {
            let sampler = ctx.gpu_resources.samplers.get_or_create(
                &ctx.device,
                &SamplerDesc {
                    label: format!(
                        "rectangle sampler mag {:?} min {:?}",
                        rectangle.texture_filter_magnification,
                        rectangle.texture_filter_minification
                    )
                    .into(),
                    mag_filter: match rectangle.texture_filter_magnification {
                        TextureFilterMag::Linear => wgpu::FilterMode::Linear,
                        TextureFilterMag::Nearest => wgpu::FilterMode::Nearest,
                    },
                    min_filter: match rectangle.texture_filter_minification {
                        TextureFilterMin::Linear => wgpu::FilterMode::Linear,
                        TextureFilterMin::Nearest => wgpu::FilterMode::Nearest,
                    },
                    mipmap_filter: wgpu::FilterMode::Nearest,
                    ..Default::default()
                },
            );

            let texture_format = texture.creation_desc.format;
            let texture_description = texture_format.describe();
            if texture_description.required_features != Default::default() {
                return Err(RectangleError::SpecialFeatures(
                    texture_description.required_features,
                ));
            }

            let mut texture_float = ctx
                .texture_manager_2d
                .get(&ctx.texture_manager_2d.white_texture_unorm_handle().clone())?
                .handle;
            let mut texture_uint = ctx.texture_manager_2d.zeroed_texture_uint().handle;

            match texture_description.sample_type {
                wgpu::TextureSampleType::Float { .. } => {
                    texture_float = texture.handle;
                }
                wgpu::TextureSampleType::Depth => {
                    return Err(RectangleError::DepthTexturesNotSupported);
                }
                wgpu::TextureSampleType::Sint => {
                    re_log::error_once!("Sint textures not yet implemented.");
                }
                wgpu::TextureSampleType::Uint => {
                    texture_uint = texture.handle;
                }
            }

            instances.push(RectangleInstance {
                bind_group: ctx.gpu_resources.bind_groups.alloc(
                    &ctx.device,
                    &ctx.gpu_resources,
                    &BindGroupDesc {
                        label: "RectangleInstance::bind_group".into(),
                        entries: smallvec![
                            uniform_buffer,
                            BindGroupEntry::Sampler(sampler),
                            BindGroupEntry::DefaultTextureView(texture_float),
                            BindGroupEntry::DefaultTextureView(texture_uint),
                        ],
                        layout: rectangle_renderer.bind_group_layout,
                    },
                ),
                draw_outline_mask: rectangle.outline_mask.is_some(),
            });
        }

        Ok(RectangleDrawData { instances })
    }
}

pub struct RectangleRenderer {
    render_pipeline_color: GpuRenderPipelineHandle,
    render_pipeline_picking_layer: GpuRenderPipelineHandle,
    render_pipeline_outline_mask: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for RectangleRenderer {
    type RendererDrawData = RectangleDrawData;

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        crate::profile_function!();

        let bind_group_layout = pools.bind_group_layouts.get_or_create(
            device,
            &BindGroupLayoutDesc {
                label: "RectangleRenderer::bind_group_layout".into(),
                entries: vec![
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            // We could use dynamic offset here into a single large buffer.
                            // But we have to set a new texture anyways and its doubtful that splitting the bind group is of any use.
                            has_dynamic_offset: false,
                            min_binding_size: (std::mem::size_of::<gpu_data::UniformBuffer>()
                                as u64)
                                .try_into()
                                .ok(),
                        },
                        count: None,
                    },
                    // float sampler:
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // float texture:
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // uint texture:
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            },
        );

        let pipeline_layout = pools.pipeline_layouts.get_or_create(
            device,
            &PipelineLayoutDesc {
                label: "RectangleRenderer::pipeline_layout".into(),
                entries: vec![shared_data.global_bindings.layout, bind_group_layout],
            },
            &pools.bind_group_layouts,
        );

        let shader_module = pools.shader_modules.get_or_create(
            device,
            resolver,
            &include_shader_module!("../../shader/rectangle.wgsl"),
        );

        let render_pipeline_desc_color = RenderPipelineDesc {
            label: "RectangleRenderer::render_pipeline_color".into(),
            pipeline_layout,
            vertex_entrypoint: "vs_main".into(),
            vertex_handle: shader_module,
            fragment_entrypoint: "fs_main".into(),
            fragment_handle: shader_module,
            vertex_buffers: smallvec![],
            render_targets: smallvec![Some(wgpu::ColorTargetState {
                format: ViewBuilder::MAIN_TARGET_COLOR_FORMAT,
                // TODO(andreas): have two render pipelines, an opaque one and a transparent one. Transparent shouldn't write depth!
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE,
            multisample: ViewBuilder::MAIN_TARGET_DEFAULT_MSAA_STATE,
        };
        let render_pipeline_color = pools.render_pipelines.get_or_create(
            device,
            &render_pipeline_desc_color,
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );
        let render_pipeline_picking_layer = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "RectangleRenderer::render_pipeline_picking_layer".into(),
                fragment_entrypoint: "fs_main_picking_layer".into(),
                render_targets: smallvec![Some(PickingLayerProcessor::PICKING_LAYER_FORMAT.into())],
                depth_stencil: PickingLayerProcessor::PICKING_LAYER_DEPTH_STATE,
                multisample: PickingLayerProcessor::PICKING_LAYER_MSAA_STATE,
                ..render_pipeline_desc_color.clone()
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );
        let render_pipeline_outline_mask = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "RectangleRenderer::render_pipeline_outline_mask".into(),
                fragment_entrypoint: "fs_main_outline_mask".into(),
                render_targets: smallvec![Some(OutlineMaskProcessor::MASK_FORMAT.into())],
                depth_stencil: OutlineMaskProcessor::MASK_DEPTH_STATE,
                multisample: OutlineMaskProcessor::mask_default_msaa_state(
                    shared_data.config.hardware_tier,
                ),
                ..render_pipeline_desc_color
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        RectangleRenderer {
            render_pipeline_color,
            render_pipeline_picking_layer,
            render_pipeline_outline_mask,
            bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &'a Self::RendererDrawData,
    ) -> anyhow::Result<()> {
        crate::profile_function!();
        if draw_data.instances.is_empty() {
            return Ok(());
        }

        let pipeline_handle = match phase {
            DrawPhase::Opaque => self.render_pipeline_color,
            DrawPhase::PickingLayer => self.render_pipeline_picking_layer,
            DrawPhase::OutlineMask => self.render_pipeline_outline_mask,
            _ => unreachable!("We were called on a phase we weren't subscribed to: {phase:?}"),
        };
        let pipeline = pools.render_pipelines.get_resource(pipeline_handle)?;

        pass.set_pipeline(pipeline);

        for rectangles in &draw_data.instances {
            if phase == DrawPhase::OutlineMask && !rectangles.draw_outline_mask {
                continue;
            }
            pass.set_bind_group(1, &rectangles.bind_group, &[]);
            pass.draw(0..4, 0..1);
        }

        Ok(())
    }

    fn participated_phases() -> &'static [DrawPhase] {
        // TODO(andreas): This a hack. We have both opaque and transparent.
        &[
            DrawPhase::OutlineMask,
            DrawPhase::Opaque,
            DrawPhase::PickingLayer,
        ]
    }
}
