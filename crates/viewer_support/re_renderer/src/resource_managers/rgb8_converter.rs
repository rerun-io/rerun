use smallvec::smallvec;

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

mod gpu_data {
    use crate::wgpu_buffer_types;

    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct UniformBuffer {
        pub target_texture_size: [u32; 2],
        pub is_bgr: u32,
        pub _padding: u32,
        pub _end_padding: [wgpu_buffer_types::PaddingRow; 15],
    }
}
/// A work item that converts tightly packed 8-bit RGB/BGR data into an RGBA texture.
pub struct Rgb8FormatConversionTask {
    bind_group: GpuBindGroup,
    target_texture: GpuTexture,
}

impl DrawData for Rgb8FormatConversionTask {
    type Renderer = Rgb8FormatConverter;

    fn collect_drawables(
        &self,
        _view_info: &DrawableCollectionViewInfo,
        _collector: &mut DrawableCollector<'_>,
    ) {
    }
}

impl Rgb8FormatConversionTask {
    pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    pub const REQUIRED_TARGET_TEXTURE_USAGE_FLAGS: wgpu::TextureUsages =
        wgpu::TextureUsages::RENDER_ATTACHMENT;

    pub fn new(
        ctx: &RenderContext,
        is_bgr: bool,
        input_data: &GpuTexture,
        target_texture: &GpuTexture,
    ) -> Result<Self, crate::RendererRegistrationError> {
        let target_label = target_texture.creation_desc.label.clone();
        let renderer = ctx.renderer::<Rgb8FormatConverter>()?;

        let uniform_buffer = create_and_fill_uniform_buffer(
            ctx,
            format!("{target_label}_rgb8_conversion").into(),
            gpu_data::UniformBuffer {
                target_texture_size: [
                    target_texture.creation_desc.size.width,
                    target_texture.creation_desc.size.height,
                ],
                is_bgr: u32::from(is_bgr),
                _padding: 0,
                _end_padding: Default::default(),
            },
        );

        let bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label: "Rgb8FormatConverter::bind_group".into(),
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

        ctx.renderer::<Rgb8FormatConverter>()?.draw(
            &ctx.gpu_resources.render_pipelines.resources(),
            crate::draw_phases::DrawPhase::Opaque,
            &mut pass,
            &[DrawInstruction {
                draw_data: &self,
                drawables: &[],
            }],
        )
    }
}

/// GPU converter for tightly packed three-channel 8-bit RGB/BGR image data.
///
/// Input bytes are packed into an integer source texture and expanded to RGBA in a fullscreen pass.
pub struct Rgb8FormatConverter {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for Rgb8FormatConverter {
    type RendererDrawData = Rgb8FormatConversionTask;

    fn create_renderer(ctx: &RenderContext) -> Self {
        let vertex_handle = screen_triangle_vertex_shader(ctx);
        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "Rgb8FormatConverter".into(),
                entries: vec![
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
                label: "Rgb8FormatConverter".into(),
                entries: vec![bind_group_layout],
            },
        );

        let shader_modules = &ctx.gpu_resources.shader_modules;
        let render_pipeline = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "Rgb8FormatConverter::render_pipeline".into(),
                pipeline_layout,
                vertex_entrypoint: "main".into(),
                vertex_handle,
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: shader_modules.get_or_create(
                    ctx,
                    &include_shader_module!("../../shader/conversions/rgb8_converter.wgsl"),
                ),
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(Rgb8FormatConversionTask::OUTPUT_FORMAT.into())],
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
