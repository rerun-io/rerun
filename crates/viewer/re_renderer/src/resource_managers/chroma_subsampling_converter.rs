use smallvec::smallvec;

use crate::{
    allocator::create_and_fill_uniform_buffer,
    include_shader_module,
    renderer::{screen_triangle_vertex_shader, DrawData, DrawError, Renderer},
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, GpuTexture, PipelineLayoutDesc, RenderPipelineDesc, TextureDesc,
    },
    DebugLabel, RenderContext,
};

use super::ColorPrimaries;

/// Supported chroma subsampling input formats.
///
/// Keep indices in sync with `chroma_subsampling_converter.wgsl`
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug)]
pub enum ChromaSubsamplingPixelFormat {
    /// 4:2:0 subsampling with a separate Y plane, followed by a UV plane.
    ///
    /// Expects single channel texture format.
    ///
    /// First comes entire image in Y in one plane,
    /// followed by a plane with interleaved lines ordered as U0, V0, U1, V1, etc.
    ///
    ///          width
    ///          __________
    ///          |         |
    /// height   |    Y    |
    ///          |         |
    ///          |_________|
    /// height/2 | U,V,U,… |
    ///          |_________|
    Y_UV12 = 0,

    /// YUV 4:2:2 subsampling, single plane.
    ///
    /// Expects single channel texture format.
    ///
    /// The order of the channels is Y0, U0, Y1, V0, all in the same plane.
    ///
    ///             width * 2
    ///        __________________
    ///        |                 |
    /// height | Y0, U0, Y1, V0… |
    ///        |_________________|
    ///
    YUYV16 = 1,
}

impl ChromaSubsamplingPixelFormat {
    pub fn expected_data_width_height(&self, width: u32, height: u32) -> (u32, u32) {
        match self {
            Self::Y_UV12 => (width, height + height / 2),
            Self::YUYV16 => (width * 2, height),
        }
    }

    pub fn expected_data_texture_format(&self) -> wgpu::TextureFormat {
        // TODO(andreas): How to deal with higher precision formats here?
        // If done right that should be easy to drop in.
        // TODO(andreas): Are we set on integer formats here, does this actually make sense?

        #[allow(clippy::match_same_arms)]
        match self {
            Self::Y_UV12 => wgpu::TextureFormat::R8Uint,
            // TODO(andreas): Why not use [`wgpu::TextureFormat::Rg8Uint`] here?
            Self::YUYV16 => wgpu::TextureFormat::R8Uint,
        }
    }

    /// Size of the buffer needed to create the data texture, i.e. the raw input data.
    pub fn expected_data_buffer_size(&self, width: u32, height: u32) -> usize {
        let num_pixels = width as usize * height as usize;
        match self {
            Self::Y_UV12 => 12 * num_pixels / 8,
            Self::YUYV16 => 16 * num_pixels / 8,
        }
    }
}

mod gpu_data {
    use crate::wgpu_buffer_types;

    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct UniformBuffer {
        /// Uses [`super::ChromaSubsamplingPixelFormat`].
        pub format: u32,

        /// Uses [`super::ColorPrimaries`].
        pub primaries: u32,

        pub target_texture_size: [u32; 2],

        pub _end_padding: [wgpu_buffer_types::PaddingRow; 16 - 1],
    }
}

/// A work item for the subsampling converter.
pub struct ChromaSubsamplingConversionTask {
    bind_group: GpuBindGroup,
    target_texture: GpuTexture,
}

impl DrawData for ChromaSubsamplingConversionTask {
    type Renderer = ChromaSubsamplingConverter;
}

impl ChromaSubsamplingConversionTask {
    /// sRGB encoded 8 bit texture.
    ///
    /// Not using [`wgpu::TextureFormat::Rgba8UnormSrgb`] since consumers typically consume this
    /// texture with software EOTF ("to linear") for more flexibility.
    pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    /// Creates a new conversion task that can be used with [`ChromaSubsamplingConversionTask`].
    ///
    /// Does *not* validate that the input data has the expected format,
    /// see methods of [`ChromaSubsamplingPixelFormat`] for details.
    pub fn new(
        ctx: &RenderContext,
        format: ChromaSubsamplingPixelFormat,
        primaries: ColorPrimaries,
        input_data: &GpuTexture,
        output_label: &DebugLabel,
        output_width: u32,
        output_height: u32,
    ) -> Self {
        let target_texture = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: output_label.clone(),
                size: wgpu::Extent3d {
                    width: output_width,
                    height: output_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1, // We don't have mipmap level generation yet!
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::OUTPUT_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
            },
        );

        let renderer = ctx.renderer::<ChromaSubsamplingConverter>();

        let uniform_buffer = create_and_fill_uniform_buffer(
            ctx,
            format!("{output_label}_conversion").into(),
            gpu_data::UniformBuffer {
                format: format as _,
                primaries: primaries as _,
                target_texture_size: [output_width, output_height],

                _end_padding: Default::default(),
            },
        );

        let bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label: "RectangleInstance::bind_group".into(),
                entries: smallvec![
                    uniform_buffer,
                    BindGroupEntry::DefaultTextureView(input_data.handle),
                ],
                layout: renderer.bind_group_layout,
            },
        );

        Self {
            bind_group,
            target_texture,
        }
    }

    /// Runs the conversion from the input texture data.
    pub fn convert_input_data_to_texture(
        self,
        ctx: &RenderContext,
    ) -> Result<GpuTexture, DrawError> {
        // TODO(andreas): Does this have to be on the global view encoder?
        // If this ever becomes a problem we could easily schedule this to another encoder as long as
        // we gurantee that the conversion is enqueued before the resulting texture is used.
        // Given that we already have this neatly encapsulated work package this would be quite easy to do!
        let mut encoder = ctx.active_frame.before_view_builder_encoder.lock();
        let mut pass = encoder
            .get()
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: self.target_texture.creation_desc.label.get(),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target_texture.default_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });

        ctx.renderer::<ChromaSubsamplingConverter>().draw(
            &ctx.gpu_resources.render_pipelines.resources(),
            crate::draw_phases::DrawPhase::Opaque, // Don't care about the phase.
            &mut pass,
            &self,
        )?;

        Ok(self.target_texture)
    }
}

/// Converter for chroma subsampling formats.
///
/// Takes chroma subsampled data and draws to a fullscreen sRGB output texture.
/// Implemented as a [`Renderer`] in order to make use of the existing mechanisms for storing renderer data.
/// (we need some place to lazily create the render pipeline, store a handle to it and encapsulate the draw logic!)
pub struct ChromaSubsamplingConverter {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for ChromaSubsamplingConverter {
    type RendererDrawData = ChromaSubsamplingConversionTask;

    fn create_renderer(ctx: &RenderContext) -> Self {
        let vertex_handle = screen_triangle_vertex_shader(ctx);

        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "ChromaSubsamplingConverter".into(),
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
                label: "ChromaSubsamplingConverter".into(),
                // Note that this is a fairly unusual layout for us with the first entry
                // not being the globally set bind group!
                entries: vec![bind_group_layout],
            },
        );

        let shader_modules = &ctx.gpu_resources.shader_modules;
        let render_pipeline = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "TestTriangle::render_pipeline".into(),
                pipeline_layout,
                vertex_entrypoint: "main".into(),
                vertex_handle,
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: shader_modules.get_or_create(
                    ctx,
                    &include_shader_module!(
                        "../../shader/conversions/chroma_subsampling_converter.wgsl"
                    ),
                ),
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(
                    ChromaSubsamplingConversionTask::OUTPUT_FORMAT.into()
                )],
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
        draw_data: &Self::RendererDrawData,
    ) -> Result<(), DrawError> {
        let pipeline = render_pipelines.get(self.render_pipeline)?;

        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &draw_data.bind_group, &[]);
        pass.draw(0..3, 0..1);

        Ok(())
    }

    fn participated_phases() -> &'static [crate::draw_phases::DrawPhase] {
        // Doesn't participate in regular rendering.
        &[]
    }
}
