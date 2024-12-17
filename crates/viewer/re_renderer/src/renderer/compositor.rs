use crate::{
    allocator::create_and_fill_uniform_buffer,
    include_shader_module,
    renderer::{screen_triangle_vertex_shader, DrawData, DrawError, Renderer},
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor, GpuTexture, PipelineLayoutDesc,
        RenderPipelineDesc,
    },
    OutlineConfig, Rgba,
};

use crate::{DrawPhase, RenderContext};

use smallvec::smallvec;

mod gpu_data {
    use crate::wgpu_buffer_types;

    /// Keep in sync with `composite.wgsl`
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct CompositeUniformBuffer {
        pub outline_color_layer_a: wgpu_buffer_types::Vec4,
        pub outline_color_layer_b: wgpu_buffer_types::Vec4,
        pub outline_radius_pixel: f32,
        pub blend_with_background: u32,
        pub padding: [u32; 2],
        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 3],
    }
}

pub struct Compositor {
    render_pipeline_opaque: GpuRenderPipelineHandle,
    render_pipeline_blended: GpuRenderPipelineHandle,
    render_pipeline_screenshot: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

#[derive(Clone)]
pub struct CompositorDrawData {
    /// [`GpuBindGroup`] pointing at the current image source and
    /// a uniform buffer for describing a tonemapper/compositor configuration.
    bind_group: GpuBindGroup,

    /// If true, the compositor will blend with the image.
    enable_blending: bool,
}

impl DrawData for CompositorDrawData {
    type Renderer = Compositor;
}

impl CompositorDrawData {
    pub fn new(
        ctx: &RenderContext,
        color_texture: &GpuTexture,
        outline_final_voronoi: Option<&GpuTexture>,
        outline_config: &Option<OutlineConfig>,
        enable_blending: bool,
    ) -> Self {
        let compositor = ctx.renderer::<Compositor>();

        let outline_config = outline_config.clone().unwrap_or(OutlineConfig {
            outline_radius_pixel: 0.0,
            color_layer_a: Rgba::TRANSPARENT,
            color_layer_b: Rgba::TRANSPARENT,
        });

        let uniform_buffer_binding = create_and_fill_uniform_buffer(
            ctx,
            "CompositorDrawData".into(),
            gpu_data::CompositeUniformBuffer {
                outline_color_layer_a: outline_config.color_layer_a.into(),
                outline_color_layer_b: outline_config.color_layer_b.into(),
                outline_radius_pixel: outline_config.outline_radius_pixel,
                blend_with_background: enable_blending as u32,
                padding: Default::default(),
                end_padding: Default::default(),
            },
        );

        let outline_final_voronoi_handle = outline_final_voronoi.map_or_else(
            || ctx.texture_manager_2d.white_texture_unorm().handle,
            |t| t.handle,
        );

        Self {
            bind_group: ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &ctx.gpu_resources,
                &BindGroupDesc {
                    label: "CompositorDrawData::bind_group".into(),
                    entries: smallvec![
                        uniform_buffer_binding,
                        BindGroupEntry::DefaultTextureView(color_texture.handle),
                        BindGroupEntry::DefaultTextureView(outline_final_voronoi_handle)
                    ],
                    layout: compositor.bind_group_layout,
                },
            ),
            enable_blending,
        }
    }
}

impl Renderer for Compositor {
    type RendererDrawData = CompositorDrawData;

    fn create_renderer(ctx: &RenderContext) -> Self {
        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "Compositor::bind_group_layout".into(),
                entries: vec![
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
                                gpu_data::CompositeUniformBuffer,
                            >(
                            )
                                as _),
                        },
                        count: None,
                    },
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
                ],
            },
        );

        let vertex_handle = screen_triangle_vertex_shader(ctx);

        let render_pipeline_descriptor = RenderPipelineDesc {
            label: "CompositorDrawData::render_pipeline_regular".into(),
            pipeline_layout: ctx.gpu_resources.pipeline_layouts.get_or_create(
                ctx,
                &PipelineLayoutDesc {
                    label: "compositor".into(),
                    entries: vec![ctx.global_bindings.layout, bind_group_layout],
                },
            ),
            vertex_entrypoint: "main".into(),
            vertex_handle,
            fragment_entrypoint: "main".into(),
            fragment_handle: ctx
                .gpu_resources
                .shader_modules
                .get_or_create(ctx, &include_shader_module!("../../shader/composite.wgsl")),
            vertex_buffers: smallvec![],
            render_targets: smallvec![Some(ctx.output_format_color().into())],
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
        };

        let render_pipeline_opaque = ctx
            .gpu_resources
            .render_pipelines
            .get_or_create(ctx, &render_pipeline_descriptor);

        let render_pipeline_blended = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                render_targets: smallvec![Some(wgpu::ColorTargetState {
                    format: ctx.output_format_color(),
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                ..render_pipeline_descriptor.clone()
            },
        );

        let render_pipeline_screenshot = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "CompositorDrawData::render_pipeline_screenshot".into(),
                render_targets: smallvec![Some(ViewBuilder::SCREENSHOT_COLOR_FORMAT.into())],
                ..render_pipeline_descriptor
            },
        );

        Self {
            render_pipeline_opaque,
            render_pipeline_blended,
            render_pipeline_screenshot,
            bind_group_layout,
        }
    }

    fn draw(
        &self,
        render_pipelines: &GpuRenderPipelinePoolAccessor<'_>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        draw_data: &CompositorDrawData,
    ) -> Result<(), DrawError> {
        let pipeline_handle = match phase {
            DrawPhase::Compositing => {
                if draw_data.enable_blending {
                    self.render_pipeline_blended
                } else {
                    self.render_pipeline_opaque
                }
            }
            DrawPhase::CompositingScreenshot => self.render_pipeline_screenshot,
            _ => unreachable!("We were called on a phase we weren't subscribed to: {phase:?}"),
        };
        let pipeline = render_pipelines.get(pipeline_handle)?;

        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, &draw_data.bind_group, &[]);
        pass.draw(0..3, 0..1);

        Ok(())
    }

    fn participated_phases() -> &'static [DrawPhase] {
        &[DrawPhase::Compositing, DrawPhase::CompositingScreenshot]
    }
}
