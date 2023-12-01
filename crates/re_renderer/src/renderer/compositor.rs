use crate::{
    allocator::create_and_fill_uniform_buffer,
    context::SharedRendererData,
    include_shader_module,
    renderer::{screen_triangle_vertex_shader, DrawData, DrawError, Renderer},
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor, GpuTexture, PipelineLayoutDesc,
        RenderPipelineDesc, WgpuResourcePools,
    },
    OutlineConfig, Rgba,
};

use crate::{DrawPhase, FileResolver, FileSystem, RenderContext};

use smallvec::smallvec;

mod gpu_data {
    use crate::wgpu_buffer_types;

    /// Keep in sync with `composite.wgsl`
    #[repr(C, align(256))]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct CompositeUniformBuffer {
        pub outline_color_layer_a: wgpu_buffer_types::Vec4,
        pub outline_color_layer_b: wgpu_buffer_types::Vec4,
        pub outline_radius_pixel: wgpu_buffer_types::F32RowPadded,
        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 3],
    }
}

pub struct Compositor {
    render_pipeline_regular: GpuRenderPipelineHandle,
    render_pipeline_screenshot: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

#[derive(Clone)]
pub struct CompositorDrawData {
    /// [`GpuBindGroup`] pointing at the current image source and
    /// a uniform buffer for describing a tonemapper/compositor configuration.
    bind_group: GpuBindGroup,
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
    ) -> Self {
        let compositor = ctx.get_renderer::<Compositor>();

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
                outline_radius_pixel: outline_config.outline_radius_pixel.into(),
                end_padding: Default::default(),
            },
        );

        let outline_final_voronoi_handle = outline_final_voronoi.map_or_else(
            || ctx.texture_manager_2d.white_texture_unorm().handle,
            |t| t.handle,
        );

        CompositorDrawData {
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
        }
    }
}

impl Renderer for Compositor {
    type RendererDrawData = CompositorDrawData;

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &FileResolver<Fs>,
    ) -> Self {
        let bind_group_layout = pools.bind_group_layouts.get_or_create(
            device,
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

        let vertex_handle = screen_triangle_vertex_shader(pools, device, resolver);

        let render_pipeline_descriptor = RenderPipelineDesc {
            label: "CompositorDrawData::render_pipeline_regular".into(),
            pipeline_layout: pools.pipeline_layouts.get_or_create(
                device,
                &PipelineLayoutDesc {
                    label: "compositor".into(),
                    entries: vec![shared_data.global_bindings.layout, bind_group_layout],
                },
                &pools.bind_group_layouts,
            ),
            vertex_entrypoint: "main".into(),
            vertex_handle,
            fragment_entrypoint: "main".into(),
            fragment_handle: pools.shader_modules.get_or_create(
                device,
                resolver,
                &include_shader_module!("../../shader/composite.wgsl"),
            ),
            vertex_buffers: smallvec![],
            render_targets: smallvec![Some(shared_data.config.output_format_color.into())],
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
        };
        let render_pipeline_regular = pools.render_pipelines.get_or_create(
            device,
            &render_pipeline_descriptor,
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );
        let render_pipeline_screenshot = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "CompositorDrawData::render_pipeline_screenshot".into(),
                render_targets: smallvec![Some(ViewBuilder::SCREENSHOT_COLOR_FORMAT.into())],
                ..render_pipeline_descriptor
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        Compositor {
            render_pipeline_regular,
            render_pipeline_screenshot,
            bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        render_pipelines: &'a GpuRenderPipelinePoolAccessor<'a>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &'a CompositorDrawData,
    ) -> Result<(), DrawError> {
        let pipeline_handle = match phase {
            DrawPhase::Compositing => self.render_pipeline_regular,
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
