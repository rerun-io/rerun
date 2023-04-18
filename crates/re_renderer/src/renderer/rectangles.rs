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
    resource_managers::{GpuTexture2D, ResourceManagerError},
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, PipelineLayoutDesc, RenderPipelineDesc, SamplerDesc,
    },
    Colormap, OutlineMaskPreference, PickingLayerProcessor, Rgba,
};

use super::{
    DrawData, FileResolver, FileSystem, RenderContext, Renderer, SharedRendererData,
    WgpuResourcePools,
};

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
#[derive(Clone)]
pub struct ColormappedTexture {
    pub texture: GpuTexture2D,

    /// Min/max range of the values in the texture.
    /// Used to normalize the input values (squash them to the 0-1 range).
    pub range: [f32; 2],

    /// Raise the normalized values to this power (before any color mapping).
    /// Acts like an inverse brightness.
    ///
    /// Default: 1.0
    pub gamma: f32,

    /// For any one-component texture, you need to supply a color mapper,
    /// which maps the normalized `.r` component to a color.
    ///
    /// Setting a color mapper for a four-component texture is an error.
    /// Failure to set a color mapper for a one-component texture is an error.
    pub color_mapper: Option<ColorMapper>,
}

/// How to map the normalized `.r` component to a color.
#[derive(Clone)]
pub enum ColorMapper {
    /// Apply the given function.
    Function(Colormap),

    /// Look up the color in this texture.
    ///
    /// The texture is indexed in a row-major fashion, so that the top left pixel
    /// corresponds to the the normalized value of 0.0, and the
    /// bottom right pixel is 1.0.
    ///
    /// The texture must have the format [`wgpu::TextureFormat::Rgba8UnormSrgb`].
    Texture(GpuTexture2D),
}

impl ColormappedTexture {
    pub fn from_unorm_srgba(texture: GpuTexture2D) -> Self {
        Self {
            texture,
            range: [0.0, 1.0],
            gamma: 1.0,
            color_mapper: None,
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

    pub options: RectangleOptions,
}

pub struct RectangleOptions {
    pub texture_filter_magnification: TextureFilterMag,
    pub texture_filter_minification: TextureFilterMin,

    /// Tint that is multiplied to the rect, supports pre-multiplied alpha.
    pub multiplicative_tint: Rgba,

    pub depth_offset: DepthOffset,

    /// Optional outline mask.
    pub outline_mask: OutlineMaskPreference,
}

impl Default for RectangleOptions {
    fn default() -> Self {
        Self {
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

    #[error("Color mapping is being applied to a four-component RGBA texture")]
    ColormappingRgbaTexture,

    #[error("Only 1 and 4 component textures are supported, got {0} components")]
    UnsupportedComponentCount(u8),

    #[error("No color mapper was supplied for this 1-component texture")]
    MissingColorMapper,

    #[error("Invalid color map texture format: {0:?}")]
    UnsupportedColormapTextureFormat(wgpu::TextureFormat),
}

mod gpu_data {
    use crate::wgpu_buffer_types;

    use super::{ColorMapper, RectangleError, TexturedRect};

    // Keep in sync with mirror in rectangle.wgsl

    // Which texture to read from?
    const SAMPLE_TYPE_FLOAT_FILTER: u32 = 1;
    const SAMPLE_TYPE_FLOAT_NOFILTER: u32 = 2;
    const SAMPLE_TYPE_SINT_NOFILTER: u32 = 3;
    const SAMPLE_TYPE_UINT_NOFILTER: u32 = 4;

    // How do we do colormapping?
    const COLOR_MAPPER_OFF: u32 = 1;
    const COLOR_MAPPER_FUNCTION: u32 = 2;
    const COLOR_MAPPER_TEXTURE: u32 = 3;

    const FILTER_NEAREST: u32 = 1;
    const FILTER_BILINEAR: u32 = 2;

    #[repr(C, align(256))]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct UniformBuffer {
        top_left_corner_position: wgpu_buffer_types::Vec3Unpadded,
        colormap_function: u32,

        extent_u: wgpu_buffer_types::Vec3Unpadded,
        sample_type: u32,

        extent_v: wgpu_buffer_types::Vec3Unpadded,
        depth_offset: f32,

        multiplicative_tint: crate::Rgba,
        outline_mask: wgpu_buffer_types::UVec2,

        /// Range of the texture values.
        /// Will be mapped to the [0, 1] range before we colormap.
        range_min_max: wgpu_buffer_types::Vec2,

        color_mapper: u32,
        gamma: f32,
        minification_filter: u32,
        magnification_filter: u32,

        _end_padding: [wgpu_buffer_types::PaddingRow; 16 - 6],
    }

    impl UniformBuffer {
        pub fn from_textured_rect(rectangle: &super::TexturedRect) -> Result<Self, RectangleError> {
            let texture_format = rectangle.colormapped_texture.texture.format();
            let texture_info = texture_format.describe();

            let TexturedRect {
                top_left_corner_position,
                extent_u,
                extent_v,
                colormapped_texture,
                options,
            } = rectangle;

            let super::ColormappedTexture {
                texture: _,
                range,
                gamma,
                color_mapper,
            } = colormapped_texture;

            let super::RectangleOptions {
                texture_filter_magnification: _,
                texture_filter_minification: _,
                multiplicative_tint,
                depth_offset,
                outline_mask,
            } = options;

            let sample_type = match texture_info.sample_type {
                wgpu::TextureSampleType::Float { .. } => {
                    if super::is_float_filterable(&texture_format) {
                        SAMPLE_TYPE_FLOAT_FILTER
                    } else {
                        SAMPLE_TYPE_FLOAT_NOFILTER
                    }
                }
                wgpu::TextureSampleType::Depth => {
                    return Err(RectangleError::DepthTexturesNotSupported);
                }
                wgpu::TextureSampleType::Sint => SAMPLE_TYPE_SINT_NOFILTER,
                wgpu::TextureSampleType::Uint => SAMPLE_TYPE_UINT_NOFILTER,
            };

            let mut colormap_function = 0;
            let color_mapper_int;

            match texture_info.components {
                1 => match color_mapper {
                    Some(ColorMapper::Function(colormap)) => {
                        color_mapper_int = COLOR_MAPPER_FUNCTION;
                        colormap_function = *colormap as u32;
                    }
                    Some(ColorMapper::Texture(_)) => {
                        color_mapper_int = COLOR_MAPPER_TEXTURE;
                    }
                    None => {
                        return Err(RectangleError::MissingColorMapper);
                    }
                },
                4 => {
                    if color_mapper.is_some() {
                        return Err(RectangleError::ColormappingRgbaTexture);
                    } else {
                        color_mapper_int = COLOR_MAPPER_OFF;
                    }
                }
                num_components => {
                    return Err(RectangleError::UnsupportedComponentCount(num_components))
                }
            }

            let minification_filter = match rectangle.options.texture_filter_minification {
                super::TextureFilterMin::Linear => FILTER_BILINEAR,
                super::TextureFilterMin::Nearest => FILTER_NEAREST,
            };
            let magnification_filter = match rectangle.options.texture_filter_magnification {
                super::TextureFilterMag::Linear => FILTER_BILINEAR,
                super::TextureFilterMag::Nearest => FILTER_NEAREST,
            };

            Ok(Self {
                top_left_corner_position: (*top_left_corner_position).into(),
                colormap_function,
                extent_u: (*extent_u).into(),
                sample_type,
                extent_v: (*extent_v).into(),
                depth_offset: *depth_offset as f32,
                multiplicative_tint: *multiplicative_tint,
                outline_mask: outline_mask.0.unwrap_or_default().into(),
                range_min_max: (*range).into(),
                color_mapper: color_mapper_int,
                gamma: *gamma,
                minification_filter,
                magnification_filter,
                _end_padding: Default::default(),
            })
        }
    }
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
        let uniform_buffers: Vec<_> = rectangles
            .iter()
            .map(gpu_data::UniformBuffer::from_textured_rect)
            .try_collect()?;

        let uniform_buffer_bindings = create_and_fill_uniform_buffer_batch(
            ctx,
            "rectangle uniform buffers".into(),
            uniform_buffers.into_iter(),
        );

        let mut instances = Vec::with_capacity(rectangles.len());
        for (rectangle, uniform_buffer) in izip!(rectangles, uniform_buffer_bindings) {
            let options = &rectangle.options;
            let sampler = ctx.gpu_resources.samplers.get_or_create(
                &ctx.device,
                &SamplerDesc {
                    label: format!(
                        "rectangle sampler mag {:?} min {:?}",
                        options.texture_filter_magnification, options.texture_filter_minification
                    )
                    .into(),
                    mag_filter: match options.texture_filter_magnification {
                        TextureFilterMag::Linear => wgpu::FilterMode::Linear,
                        TextureFilterMag::Nearest => wgpu::FilterMode::Nearest,
                    },
                    min_filter: match options.texture_filter_minification {
                        TextureFilterMin::Linear => wgpu::FilterMode::Linear,
                        TextureFilterMin::Nearest => wgpu::FilterMode::Nearest,
                    },
                    mipmap_filter: wgpu::FilterMode::Nearest,
                    ..Default::default()
                },
            );

            let texture = &rectangle.colormapped_texture.texture;
            let texture_format = texture.creation_desc.format;
            let texture_description = texture_format.describe();
            if texture_description.required_features != Default::default() {
                return Err(RectangleError::SpecialFeatures(
                    texture_description.required_features,
                ));
            }

            // We set up several texture sources, then instruct the shader to read from at most one of them.
            let mut texture_float_filterable = ctx.texture_manager_2d.zeroed_texture_float().handle;
            let mut texture_float_nofilter = ctx.texture_manager_2d.zeroed_texture_float().handle;
            let mut texture_sint = ctx.texture_manager_2d.zeroed_texture_sint().handle;
            let mut texture_uint = ctx.texture_manager_2d.zeroed_texture_uint().handle;

            match texture_description.sample_type {
                wgpu::TextureSampleType::Float { .. } => {
                    if is_float_filterable(&texture_format) {
                        texture_float_filterable = texture.handle;
                    } else {
                        texture_float_nofilter = texture.handle;
                    }
                }
                wgpu::TextureSampleType::Depth => {
                    return Err(RectangleError::DepthTexturesNotSupported);
                }
                wgpu::TextureSampleType::Sint => {
                    texture_sint = texture.handle;
                }
                wgpu::TextureSampleType::Uint => {
                    texture_uint = texture.handle;
                }
            }

            // We also set up an optional colormap texture.
            let colormap_texture = if let Some(ColorMapper::Texture(handle)) =
                &rectangle.colormapped_texture.color_mapper
            {
                let format = handle.format();
                if format != wgpu::TextureFormat::Rgba8UnormSrgb {
                    return Err(RectangleError::UnsupportedColormapTextureFormat(format));
                }
                handle.handle()
            } else {
                ctx.texture_manager_2d.zeroed_texture_float().handle
            };

            instances.push(RectangleInstance {
                bind_group: ctx.gpu_resources.bind_groups.alloc(
                    &ctx.device,
                    &ctx.gpu_resources,
                    &BindGroupDesc {
                        label: "RectangleInstance::bind_group".into(),
                        entries: smallvec![
                            uniform_buffer,
                            BindGroupEntry::Sampler(sampler),
                            BindGroupEntry::DefaultTextureView(texture_float_nofilter),
                            BindGroupEntry::DefaultTextureView(texture_sint),
                            BindGroupEntry::DefaultTextureView(texture_uint),
                            BindGroupEntry::DefaultTextureView(colormap_texture),
                            BindGroupEntry::DefaultTextureView(texture_float_filterable),
                        ],
                        layout: rectangle_renderer.bind_group_layout,
                    },
                ),
                draw_outline_mask: rectangle.options.outline_mask.is_some(),
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
                    // float textures without filtering (e.g. R32Float):
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // sint texture:
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Sint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // uint texture:
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // colormap texture:
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // float textures with filtering (e.g. Rgba8UnormSrgb):
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
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

        let shader_module_vs = pools.shader_modules.get_or_create(
            device,
            resolver,
            &include_shader_module!("../../shader/rectangle_vs.wgsl"),
        );
        let shader_module_fs = pools.shader_modules.get_or_create(
            device,
            resolver,
            &include_shader_module!("../../shader/rectangle_fs.wgsl"),
        );

        let render_pipeline_desc_color = RenderPipelineDesc {
            label: "RectangleRenderer::render_pipeline_color".into(),
            pipeline_layout,
            vertex_entrypoint: "vs_main".into(),
            vertex_handle: shader_module_vs,
            fragment_entrypoint: "fs_main".into(),
            fragment_handle: shader_module_fs,
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

fn is_float_filterable(format: &wgpu::TextureFormat) -> bool {
    format
        .describe()
        .guaranteed_format_features
        .flags
        .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
}
