use smallvec::smallvec;

use super::ImageDataToTextureError;
use crate::allocator::create_and_fill_uniform_buffer;
use crate::renderer::{
    DrawData, DrawError, DrawInstruction, DrawableCollectionViewInfo, Renderer,
    screen_triangle_vertex_shader,
};
use crate::wgpu_resources::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
    GpuRenderPipelineHandle, GpuTexture, PipelineLayoutDesc, RenderPipelineDesc,
};
use crate::{DrawableCollector, RenderContext, include_shader_module};

/// The arrangement of the color filter on a raw 8 bit Bayer image.
///
/// The name lists the colors of the top-left 2x2 block, row by row.
///
/// Expects a single channel data texture with the same size as the output, see [`Self::DATA_TEXTURE_FORMAT`].
///
/// Keep indices in sync with `bayer_converter.wgsl`
#[derive(Clone, Copy, Debug)]
pub enum BayerPattern {
    Rggb = 0,
    Bggr = 1,
    Gbrg = 2,
    Grbg = 3,
}

impl std::fmt::Display for BayerPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rggb => write!(f, "RGGB"),
            Self::Bggr => write!(f, "BGGR"),
            Self::Gbrg => write!(f, "GBRG"),
            Self::Grbg => write!(f, "GRBG"),
        }
    }
}

impl BayerPattern {
    /// What format the input data texture is expected to be in.
    pub const DATA_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Uint;

    /// Size of the buffer needed to create the data texture, i.e. the raw input data.
    pub fn num_data_buffer_bytes([width, height]: [u32; 2]) -> usize {
        width as usize * height as usize
    }
}

mod gpu_data {
    use crate::wgpu_buffer_types;

    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct UniformBuffer {
        /// Uses [`super::BayerPattern`].
        pub bayer_pattern: wgpu_buffer_types::U32RowPadded,

        pub _end_padding: [wgpu_buffer_types::PaddingRow; 16 - 1],
    }
}

/// A work item for the Bayer converter.
pub struct BayerFormatConversionTask {
    bind_group: GpuBindGroup,
    target_texture: GpuTexture,
}

impl DrawData for BayerFormatConversionTask {
    type Renderer = BayerFormatConverter;

    fn collect_drawables(
        &self,
        _view_info: &DrawableCollectionViewInfo,
        _collector: &mut DrawableCollector<'_>,
    ) {
        // Doesn't participate in regular rendering.
    }
}

impl BayerFormatConversionTask {
    /// Format that a target texture must have in order to be used as output of this converter.
    ///
    /// Contains linear RGB values. Consumers must not apply sRGB decoding.
    pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    /// Usage flags that a target texture must have in order to be used as output of this converter.
    pub const REQUIRED_TARGET_TEXTURE_USAGE_FLAGS: wgpu::TextureUsages =
        wgpu::TextureUsages::RENDER_ATTACHMENT;

    /// Creates a new conversion task that can be used with [`BayerFormatConverter`].
    ///
    /// The input data texture has the format [`BayerPattern::DATA_TEXTURE_FORMAT`]
    /// and the same size as the target texture.
    pub fn new(
        ctx: &RenderContext,
        bayer_pattern: BayerPattern,
        input_data: &GpuTexture,
        target_texture: &GpuTexture,
    ) -> Result<Self, ImageDataToTextureError> {
        let input_desc = &input_data.creation_desc;
        if input_desc.format != BayerPattern::DATA_TEXTURE_FORMAT {
            return Err(ImageDataToTextureError::InvalidSourceTextureFormat {
                label: input_desc.label.clone(),
                actual_format: input_desc.format,
                required_format: BayerPattern::DATA_TEXTURE_FORMAT,
            });
        }
        if input_desc.size != target_texture.creation_desc.size {
            return Err(ImageDataToTextureError::InvalidSourceTextureSize {
                label: input_desc.label.clone(),
                actual_size: input_desc.size,
                expected_size: target_texture.creation_desc.size,
            });
        }

        let target_label = target_texture.creation_desc.label.clone();
        let renderer = ctx.renderer::<BayerFormatConverter>()?;

        let uniform_buffer = create_and_fill_uniform_buffer(
            ctx,
            format!("{target_label}_conversion").into(),
            gpu_data::UniformBuffer {
                bayer_pattern: (bayer_pattern as u32).into(),
                _end_padding: Default::default(),
            },
        );

        let bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label: "BayerFormatConversionTask::bind_group".into(),
                entries: smallvec![
                    uniform_buffer,
                    BindGroupEntry::DefaultTextureView(input_data.handle),
                ],
                layout: renderer.bind_group_layout,
            },
        );

        Ok(Self {
            bind_group,
            target_texture: target_texture.clone(),
        })
    }

    /// Runs the conversion from the input texture data.
    pub fn convert_input_data_to_texture(self, ctx: &RenderContext) -> Result<(), DrawError> {
        let mut encoder = ctx.active_frame.before_view_builder_encoder.lock();
        let mut pass = encoder
            .get()
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(self.target_texture.creation_desc.label.get()),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target_texture.default_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });

        ctx.renderer::<BayerFormatConverter>()?.draw(
            &ctx.gpu_resources.render_pipelines.resources(),
            crate::draw_phases::DrawPhase::Opaque, // Don't care about the phase.
            &mut pass,
            &[DrawInstruction {
                draw_data: &self,
                drawables: &[],
            }],
        )
    }
}

/// Converter for raw Bayer images.
///
/// Takes linear single channel color filter array data and draws demosaiced linear RGB to a fullscreen output texture.
/// Implemented as a [`Renderer`] in order to make use of the existing mechanisms for storing renderer data.
pub struct BayerFormatConverter {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for BayerFormatConverter {
    type RendererDrawData = BayerFormatConversionTask;

    fn create_renderer(ctx: &RenderContext) -> Self {
        let vertex_handle = screen_triangle_vertex_shader(ctx);

        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "BayerFormatConverter".into(),
                entries: vec![
                    // Uniform buffer with some information.
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: (std::mem::size_of::<gpu_data::UniformBuffer>()
                                as u64)
                                .try_into()
                                .ok(),
                        },
                        count: None,
                    },
                    // Input data texture.
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Uint,
                        },
                        count: None,
                    },
                ],
            },
        );

        let pipeline_layout = ctx.gpu_resources.pipeline_layouts.get_or_create(
            ctx,
            &PipelineLayoutDesc {
                label: "BayerFormatConverter".into(),
                // Note that this is a fairly unusual layout for us with the first entry
                // not being the globally set bind group!
                entries: vec![bind_group_layout],
            },
        );

        let shader_modules = &ctx.gpu_resources.shader_modules;
        let render_pipeline = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "BayerFormatConverter::render_pipeline".into(),
                pipeline_layout,
                vertex_entrypoint: "main".into(),
                vertex_handle,
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: shader_modules.get_or_create(
                    ctx,
                    &include_shader_module!("../../shader/conversions/bayer_converter.wgsl"),
                ),
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(BayerFormatConversionTask::OUTPUT_FORMAT.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            },
        );

        Self {
            render_pipeline,
            bind_group_layout,
        }
    }

    fn draw(
        &self,
        render_pipelines: &crate::wgpu_resources::GpuRenderPipelinePoolAccessor<'_>,
        _phase: crate::draw_phases::DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        draw_instructions: &[DrawInstruction<'_, Self::RendererDrawData>],
    ) -> Result<(), DrawError> {
        let pipeline = render_pipelines.get(self.render_pipeline)?;

        pass.set_pipeline(pipeline);

        for DrawInstruction { draw_data, .. } in draw_instructions {
            pass.set_bind_group(0, &draw_data.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        Ok(())
    }
}
