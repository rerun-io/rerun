//! Renderer that makes it easy to draw textured 2D rectangles with transparency
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

use itertools::{Itertools as _, izip};
use smallvec::smallvec;

use super::{DrawData, DrawError, RenderContext, Renderer};
use crate::allocator::create_and_fill_uniform_buffer_batch;
use crate::depth_offset::DepthOffset;
use crate::draw_phases::{DrawPhase, OutlineMaskProcessor};
use crate::renderer::{DrawDataDrawable, DrawInstruction, DrawableCollectionViewInfo};
use crate::resource_managers::{AlphaChannelUsage, GpuTexture2D};
use crate::view_builder::ViewBuilder;
use crate::wgpu_resources::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
    GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor, PipelineLayoutDesc, RenderPipelineDesc,
};
use crate::{
    Colormap, DrawableCollector, OutlineMaskPreference, PickingLayerProcessor, Rgba,
    include_shader_module,
};

/// Texture filter setting for magnification (a texel covers several pixels).
#[derive(Debug, Clone, Copy)]
pub enum TextureFilterMag {
    Linear,
    Nearest,
    // TODO(andreas): Offer advanced (shader implemented) filters like cubic?
}

/// Texture filter setting for minification (several texels fall to one pixel).
#[derive(Debug, Clone, Copy)]
pub enum TextureFilterMin {
    Linear,
    Nearest,
    // TODO(andreas): Offer mipmapping here?
}

/// Describes how the color information is encoded in the texture.
// TODO(#10648): to be replaced by re_renderer based on-input conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShaderDecoding {
    /// Do BGR(A)->RGB(A) conversion is in the shader.
    Bgr,
}

/// How is the alpha channel in the texture?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureAlpha {
    /// Ignore the alpha channel and render the image opaquely.
    ///
    /// Use this for textures that don't have an alpha channel.
    Opaque = 1,

    /// The alpha in the texture is separate/unmulitplied.
    ///
    /// Use this for images with separate (unmulitplied) alpha channels (the normal kind).
    SeparateAlpha = 2,

    /// The RGB values have already been premultiplied with the alpha.
    AlreadyPremultiplied = 3,
}

/// Describes a texture and how to map it to a color.
#[derive(Clone, Debug)]
pub struct ColormappedTexture {
    pub texture: GpuTexture2D,

    /// Min/max range of the values in the texture.
    ///
    /// Used to normalize the input values (squash them to the 0-1 range).
    /// The normalization is applied before sRGB gamma decoding and alpha pre-multiplication
    /// (this transformation is also applied to alpha!).
    pub range: [f32; 2],

    /// Decode 0-1 sRGB gamma values to linear space before filtering?
    ///
    /// Only applies to [`wgpu::TextureFormat::Rgba8Unorm`] and float textures.
    pub decode_srgb: bool,

    /// How to treat the alpha channel.
    pub texture_alpha: TextureAlpha,

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
    pub color_mapper: ColorMapper,

    /// For textures that need decoding in the shader, for example NV12 encoded images.
    pub shader_decoding: Option<ShaderDecoding>,
}

/// How to map the normalized `.r` component to a color.
#[derive(Clone, Debug)]
pub enum ColorMapper {
    /// Colormapping is off. Take the .r color and splat onto rgb.
    OffGrayscale,

    /// Colormapping is off. Keep rgb as is.
    OffRGB,

    /// Apply the given function.
    Function(Colormap),

    /// Look up the color in this texture.
    ///
    /// The texture is indexed in a row-major fashion, so that the top left pixel
    /// corresponds to the normalized value of 0.0, and the
    /// bottom right pixel is 1.0.
    ///
    /// The texture must have the format [`wgpu::TextureFormat::Rgba8UnormSrgb`].
    Texture(GpuTexture2D),
}

impl ColorMapper {
    #[inline]
    pub fn is_on(&self) -> bool {
        match self {
            Self::OffGrayscale | Self::OffRGB => false,
            Self::Function(_) | Self::Texture(_) => true,
        }
    }
}

impl ColormappedTexture {
    /// Assumes a separate/unmultiplied alpha.
    pub fn from_unorm_rgba(texture: GpuTexture2D) -> Self {
        // If the texture is an sRGB texture, the GPU will decode it for us.
        let decode_srgb = !texture.format().is_srgb();
        Self {
            texture,
            decode_srgb,
            range: [0.0, 1.0],
            gamma: 1.0,
            texture_alpha: TextureAlpha::SeparateAlpha,
            color_mapper: ColorMapper::OffRGB,
            shader_decoding: None,
        }
    }

    pub fn width_height(&self) -> [u32; 2] {
        self.texture.width_height()
    }
}

#[derive(Clone, Debug)]
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

impl TexturedRect {
    /// Returns axis aligned bounding box for this rectangle.
    pub fn bounding_box(&self) -> macaw::BoundingBox {
        let left_top = self.top_left_corner_position;
        let extent_u = self.extent_u;
        let extent_v = self.extent_v;

        macaw::BoundingBox::from_points(
            [
                left_top,
                left_top + extent_u,
                left_top + extent_v,
                left_top + extent_v + extent_u,
            ]
            .into_iter(),
        )
    }
}

#[derive(Clone, Debug)]
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
    #[error("Texture required special features: {0:?}")]
    SpecialFeatures(wgpu::Features),

    #[error("Texture format not supported: {0:?} - use float or integer textures instead.")]
    TextureFormatNotSupported(wgpu::TextureFormat),

    #[error(
        "Color mapping cannot be applied to a four-component RGBA image, but only to a single-component image."
    )]
    ColormappingRgbaTexture,

    #[error("Only 1 and 4 component textures are supported, got {0} components")]
    UnsupportedComponentCount(u8),

    #[error("No color mapper was supplied for this 1-component texture")]
    MissingColorMapper,

    #[error("Invalid color map texture format: {0:?}")]
    UnsupportedColormapTextureFormat(wgpu::TextureFormat),

    #[error("decode_srgb set to true, but the texture was already sRGB aware")]
    DoubleDecodingSrgbTexture,
}

mod gpu_data {
    use super::{ColorMapper, RectangleError, TexturedRect};
    use crate::wgpu_buffer_types;

    // Keep in sync with mirror in rectangle.wgsl

    // Which texture to read from?
    const SAMPLE_TYPE_FLOAT: u32 = 1;
    const SAMPLE_TYPE_SINT: u32 = 2;
    const SAMPLE_TYPE_UINT: u32 = 3;

    // How do we do colormapping?
    const COLOR_MAPPER_OFF_GRAYSCALE: u32 = 1;
    const COLOR_MAPPER_OFF_RGB: u32 = 2;
    const COLOR_MAPPER_FUNCTION: u32 = 3;
    const COLOR_MAPPER_TEXTURE: u32 = 4;

    const FILTER_NEAREST: u32 = 1;
    const FILTER_BILINEAR: u32 = 2;

    #[repr(C)]
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

        decode_srgb: u32,
        texture_alpha: u32,
        bgra_to_rgba: u32,
        _row_padding: [u32; 1],

        _end_padding: [wgpu_buffer_types::PaddingRow; 16 - 7],
    }

    impl UniformBuffer {
        pub fn from_textured_rect(rectangle: &super::TexturedRect) -> Result<Self, RectangleError> {
            let texture_format = rectangle.colormapped_texture.texture.format();

            if texture_format.is_srgb() && rectangle.colormapped_texture.decode_srgb {
                return Err(RectangleError::DoubleDecodingSrgbTexture);
            }

            let TexturedRect {
                top_left_corner_position,
                extent_u,
                extent_v,
                colormapped_texture,
                options,
            } = rectangle;

            let super::ColormappedTexture {
                texture: _,
                decode_srgb,
                range,
                gamma,
                color_mapper,
                texture_alpha,
                shader_decoding,
            } = colormapped_texture;

            let super::RectangleOptions {
                texture_filter_magnification: _,
                texture_filter_minification: _,
                multiplicative_tint,
                depth_offset,
                outline_mask,
            } = options;

            let sample_type = match texture_format.sample_type(None, None) {
                Some(wgpu::TextureSampleType::Float { .. }) => SAMPLE_TYPE_FLOAT,
                Some(wgpu::TextureSampleType::Sint) => SAMPLE_TYPE_SINT,
                Some(wgpu::TextureSampleType::Uint) => SAMPLE_TYPE_UINT,
                _ => {
                    return Err(RectangleError::TextureFormatNotSupported(texture_format));
                }
            };

            let mut colormap_function = 0;
            let color_mapper_int = match texture_format.components() {
                1 => match color_mapper {
                    ColorMapper::OffGrayscale => COLOR_MAPPER_OFF_GRAYSCALE,
                    ColorMapper::OffRGB => COLOR_MAPPER_OFF_RGB,
                    ColorMapper::Function(colormap) => {
                        colormap_function = *colormap as u32;
                        COLOR_MAPPER_FUNCTION
                    }
                    ColorMapper::Texture(_) => COLOR_MAPPER_TEXTURE,
                },
                4 => match color_mapper {
                    ColorMapper::OffGrayscale => COLOR_MAPPER_OFF_GRAYSCALE, // This is a bit weird, but why not
                    ColorMapper::OffRGB => COLOR_MAPPER_OFF_RGB,
                    ColorMapper::Function(_) | ColorMapper::Texture(_) => {
                        return Err(RectangleError::ColormappingRgbaTexture);
                    }
                },
                num_components => {
                    return Err(RectangleError::UnsupportedComponentCount(num_components));
                }
            };

            let minification_filter = match rectangle.options.texture_filter_minification {
                super::TextureFilterMin::Linear => FILTER_BILINEAR,
                super::TextureFilterMin::Nearest => FILTER_NEAREST,
            };
            let magnification_filter = match rectangle.options.texture_filter_magnification {
                super::TextureFilterMag::Linear => FILTER_BILINEAR,
                super::TextureFilterMag::Nearest => FILTER_NEAREST,
            };
            let bgra_to_rgba = shader_decoding == &Some(super::ShaderDecoding::Bgr);

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
                decode_srgb: *decode_srgb as _,
                texture_alpha: *texture_alpha as _,
                bgra_to_rgba: bgra_to_rgba as _,
                _row_padding: Default::default(),
                _end_padding: Default::default(),
            })
        }
    }
}

#[derive(Clone)]
struct RectangleInstance {
    center_position: glam::Vec3A,
    bind_group: GpuBindGroup,
    draw_outline_mask: bool,
    has_transparency: bool,
}

#[derive(Clone)]
pub struct RectangleDrawData {
    instances: Vec<RectangleInstance>,
}

impl DrawData for RectangleDrawData {
    type Renderer = RectangleRenderer;

    fn collect_drawables(
        &self,
        view_info: &DrawableCollectionViewInfo,
        collector: &mut DrawableCollector<'_>,
    ) {
        // TODO(#1025, #4787): Better handling of 2D objects, use per-2D layer sorting instead of depth offsets.
        // For 2D draw order based sorting, we should never enable depth write and always perform back to front sorting.

        for (index, instance) in self.instances.iter().enumerate() {
            let mut phases = enumset::EnumSet::new();
            phases.insert(if instance.has_transparency {
                DrawPhase::Transparent
            } else {
                DrawPhase::Opaque
            });
            phases.insert(DrawPhase::PickingLayer);
            if instance.draw_outline_mask {
                phases.insert(DrawPhase::OutlineMask);
            }

            collector.add_drawable(
                phases,
                DrawDataDrawable::from_world_position(
                    view_info,
                    instance.center_position,
                    index as u32,
                ),
            );
        }
    }
}

impl RectangleDrawData {
    pub fn new(ctx: &RenderContext, rectangles: &[TexturedRect]) -> Result<Self, RectangleError> {
        re_tracing::profile_function!();

        let rectangle_renderer = ctx.renderer::<RectangleRenderer>();

        if rectangles.is_empty() {
            return Ok(Self {
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
            let texture = &rectangle.colormapped_texture.texture;
            let texture_format = texture.creation_desc.format;
            if texture_format.required_features() != Default::default() {
                return Err(RectangleError::SpecialFeatures(
                    texture_format.required_features(),
                ));
            }

            // We set up several texture sources, then instruct the shader to read from at most one of them.
            let mut texture_float = ctx.texture_manager_2d.zeroed_texture_float().handle;
            let mut texture_sint = ctx.texture_manager_2d.zeroed_texture_sint().handle;
            let mut texture_uint = ctx.texture_manager_2d.zeroed_texture_uint().handle;

            match texture_format.sample_type(None, None) {
                Some(wgpu::TextureSampleType::Float { .. }) => {
                    texture_float = texture.handle;
                }
                Some(wgpu::TextureSampleType::Sint) => {
                    texture_sint = texture.handle;
                }
                Some(wgpu::TextureSampleType::Uint) => {
                    texture_uint = texture.handle;
                }
                _ => {
                    return Err(RectangleError::TextureFormatNotSupported(texture_format));
                }
            }

            // We also set up an optional colormap texture.
            let colormap_texture =
                if let ColorMapper::Texture(handle) = &rectangle.colormapped_texture.color_mapper {
                    let format = handle.format();
                    if format != wgpu::TextureFormat::Rgba8UnormSrgb {
                        return Err(RectangleError::UnsupportedColormapTextureFormat(format));
                    }
                    handle.handle()
                } else {
                    ctx.texture_manager_2d.zeroed_texture_float().handle
                };

            instances.push(RectangleInstance {
                center_position: (rectangle.top_left_corner_position
                    + rectangle.extent_u * 0.5
                    + rectangle.extent_v * 0.5)
                    .into(),
                bind_group: ctx.gpu_resources.bind_groups.alloc(
                    &ctx.device,
                    &ctx.gpu_resources,
                    &BindGroupDesc {
                        label: "RectangleInstance::bind_group".into(),
                        entries: smallvec![
                            uniform_buffer,
                            BindGroupEntry::DefaultTextureView(texture_float),
                            BindGroupEntry::DefaultTextureView(texture_sint),
                            BindGroupEntry::DefaultTextureView(texture_uint),
                            BindGroupEntry::DefaultTextureView(colormap_texture)
                        ],
                        layout: rectangle_renderer.bind_group_layout,
                    },
                ),
                draw_outline_mask: rectangle.options.outline_mask.is_some(),
                has_transparency: rectangle.options.multiplicative_tint.a() < 1.0
                    || rectangle.colormapped_texture.texture.alpha_channel_usage()
                        == AlphaChannelUsage::AlphaChannelInUse
                    || matches!(&rectangle.colormapped_texture.color_mapper, ColorMapper::Texture(texture) if texture.alpha_channel_usage() == AlphaChannelUsage::AlphaChannelInUse),
            });
        }

        Ok(Self { instances })
    }
}

pub struct RectangleRenderer {
    render_pipeline_color_opaque: GpuRenderPipelineHandle,
    render_pipeline_color_transparent: GpuRenderPipelineHandle,
    render_pipeline_picking_layer: GpuRenderPipelineHandle,
    render_pipeline_outline_mask: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for RectangleRenderer {
    type RendererDrawData = RectangleDrawData;

    fn create_renderer(ctx: &RenderContext) -> Self {
        re_tracing::profile_function!();

        let render_pipelines = &ctx.gpu_resources.render_pipelines;

        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &(BindGroupLayoutDesc {
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
                    // float texture:
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
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
                        binding: 2,
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
                        binding: 3,
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
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            }),
        );

        let pipeline_layout = ctx.gpu_resources.pipeline_layouts.get_or_create(
            ctx,
            &(PipelineLayoutDesc {
                label: "RectangleRenderer::pipeline_layout".into(),
                entries: vec![ctx.global_bindings.layout, bind_group_layout],
            }),
        );

        let shader_module_vs = ctx.gpu_resources.shader_modules.get_or_create(
            ctx,
            &include_shader_module!("../../shader/rectangle_vs.wgsl"),
        );
        let shader_module_fs = ctx.gpu_resources.shader_modules.get_or_create(
            ctx,
            &include_shader_module!("../../shader/rectangle_fs.wgsl"),
        );

        let render_pipeline_desc_color_opaque = RenderPipelineDesc {
            label: "RectangleRenderer::render_pipeline_color_opaque".into(),
            pipeline_layout,
            vertex_entrypoint: "vs_main".into(),
            vertex_handle: shader_module_vs,
            fragment_entrypoint: "fs_main".into(),
            fragment_handle: shader_module_fs,
            vertex_buffers: smallvec![],
            render_targets: smallvec![Some(ViewBuilder::MAIN_TARGET_COLOR_FORMAT.into())],
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE),
            multisample: ViewBuilder::main_target_default_msaa_state(ctx.render_config(), false),
        };
        let render_pipeline_color_opaque =
            render_pipelines.get_or_create(ctx, &render_pipeline_desc_color_opaque);

        let render_pipeline_desc_color_transparent = RenderPipelineDesc {
            label: "RectangleRenderer::render_pipeline_color_transparent".into(),
            render_targets: smallvec![Some(wgpu::ColorTargetState {
                format: ViewBuilder::MAIN_TARGET_COLOR_FORMAT,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                cull_mode: None,
                ..Default::default()
            },
            ..render_pipeline_desc_color_opaque.clone()
        };
        let render_pipeline_color_transparent =
            render_pipelines.get_or_create(ctx, &render_pipeline_desc_color_transparent);

        let render_pipeline_picking_layer = render_pipelines.get_or_create(
            ctx,
            &(RenderPipelineDesc {
                label: "RectangleRenderer::render_pipeline_picking_layer".into(),
                fragment_entrypoint: "fs_main_picking_layer".into(),
                render_targets: smallvec![Some(PickingLayerProcessor::PICKING_LAYER_FORMAT.into())],
                depth_stencil: PickingLayerProcessor::PICKING_LAYER_DEPTH_STATE,
                multisample: PickingLayerProcessor::PICKING_LAYER_MSAA_STATE,
                ..render_pipeline_desc_color_opaque.clone()
            }),
        );
        let render_pipeline_outline_mask = render_pipelines.get_or_create(
            ctx,
            &(RenderPipelineDesc {
                label: "RectangleRenderer::render_pipeline_outline_mask".into(),
                fragment_entrypoint: "fs_main_outline_mask".into(),
                render_targets: smallvec![Some(OutlineMaskProcessor::MASK_FORMAT.into())],
                depth_stencil: OutlineMaskProcessor::MASK_DEPTH_STATE,
                multisample: OutlineMaskProcessor::mask_default_msaa_state(ctx.device_caps().tier),
                ..render_pipeline_desc_color_opaque.clone()
            }),
        );

        Self {
            render_pipeline_color_opaque,
            render_pipeline_color_transparent,
            render_pipeline_picking_layer,
            render_pipeline_outline_mask,
            bind_group_layout,
        }
    }

    fn draw(
        &self,
        render_pipelines: &GpuRenderPipelinePoolAccessor<'_>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        draw_instructions: &[DrawInstruction<'_, Self::RendererDrawData>],
    ) -> Result<(), DrawError> {
        re_tracing::profile_function!();

        let pipeline_handle = match phase {
            DrawPhase::Transparent => self.render_pipeline_color_transparent,
            DrawPhase::Opaque => self.render_pipeline_color_opaque,
            DrawPhase::PickingLayer => self.render_pipeline_picking_layer,
            DrawPhase::OutlineMask => self.render_pipeline_outline_mask,
            _ => unreachable!("We were called on a phase we weren't subscribed to: {phase:?}"),
        };
        let pipeline = render_pipelines.get(pipeline_handle)?;

        pass.set_pipeline(pipeline);

        for DrawInstruction {
            draw_data,
            drawables,
        } in draw_instructions
        {
            for drawable in *drawables {
                let rectangles = &draw_data.instances[drawable.draw_data_payload as usize];
                pass.set_bind_group(1, &rectangles.bind_group, &[]);
                pass.draw(0..4, 0..1);
            }
        }

        Ok(())
    }
}
