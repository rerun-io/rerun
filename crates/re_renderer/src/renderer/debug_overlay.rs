use crate::{
    allocator::create_and_fill_uniform_buffer,
    context::SharedRendererData,
    draw_phases::DrawPhase,
    include_file,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, GpuTexture, PipelineLayoutDesc, RenderPipelineDesc,
        ShaderModuleDesc, WgpuResourcePools,
    },
};

use super::{DrawData, FileResolver, FileSystem, RenderContext, Renderer};

use smallvec::smallvec;

mod gpu_data {
    use crate::wgpu_buffer_types;

    /// Keep in sync with `debug_overlay.wgsl`
    #[repr(C, align(256))]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct DebugOverlayUniformBuffer {
        pub screen_resolution: wgpu_buffer_types::Vec2,
        pub position_in_pixel: wgpu_buffer_types::Vec2,
        pub extent_in_pixel: wgpu_buffer_types::Vec2RowPadded,
        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 2],
    }
}

pub struct DebugOverlayRenderer {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

#[derive(Clone)]
pub struct DebugOverlayDrawData {
    bind_group: GpuBindGroup,
}

impl DrawData for DebugOverlayDrawData {
    type Renderer = DebugOverlayRenderer;
}

impl DebugOverlayDrawData {
    pub fn new(
        ctx: &mut RenderContext,
        debug_texture: &GpuTexture,
        screen_resolution: glam::UVec2,
        position_in_pixel: glam::UVec2,
        extent_in_pixel: glam::UVec2,
    ) -> Self {
        let mut renderers = ctx.renderers.write();
        let debug_overlay = renderers.get_or_create::<_, DebugOverlayRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.gpu_resources,
            &ctx.device,
            &mut ctx.resolver,
        );

        let uniform_buffer_binding = create_and_fill_uniform_buffer(
            ctx,
            "DebugOverlayDrawData".into(),
            gpu_data::DebugOverlayUniformBuffer {
                screen_resolution: screen_resolution.as_vec2().into(),
                position_in_pixel: position_in_pixel.as_vec2().into(),
                extent_in_pixel: extent_in_pixel.as_vec2().into(),
                end_padding: Default::default(),
            },
        );

        DebugOverlayDrawData {
            bind_group: ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &ctx.gpu_resources,
                &BindGroupDesc {
                    label: "DebugOverlay".into(),
                    entries: smallvec![
                        uniform_buffer_binding,
                        BindGroupEntry::DefaultTextureView(debug_texture.handle),
                    ],
                    layout: debug_overlay.bind_group_layout,
                },
            ),
        }
    }
}

impl Renderer for DebugOverlayRenderer {
    type RendererDrawData = DebugOverlayDrawData;

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        let bind_group_layout = pools.bind_group_layouts.get_or_create(
            device,
            &BindGroupLayoutDesc {
                label: "DebugOverlay::bind_group_layout".into(),
                entries: vec![
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
                                gpu_data::DebugOverlayUniformBuffer,
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
                ],
            },
        );

        let shader_module = pools.shader_modules.get_or_create(
            device,
            resolver,
            &ShaderModuleDesc {
                label: "DebugOverlay".into(),
                source: include_file!("../../shader/debug_overlay.wgsl"),
            },
        );
        let render_pipeline = pools.render_pipelines.get_or_create(
            device,
            &(RenderPipelineDesc {
                label: "DebugOverlayDrawData::render_pipeline_regular".into(),
                pipeline_layout: pools.pipeline_layouts.get_or_create(
                    device,
                    &PipelineLayoutDesc {
                        label: "DebugOverlay".into(),
                        entries: vec![shared_data.global_bindings.layout, bind_group_layout],
                    },
                    &pools.bind_group_layouts,
                ),
                vertex_entrypoint: "main_vs".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "main_fs".into(),
                fragment_handle: shader_module,
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(shared_data.config.output_format_color.into())],
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            }),
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );
        DebugOverlayRenderer {
            render_pipeline,
            bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        _phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &'a DebugOverlayDrawData,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;

        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, &draw_data.bind_group, &[]);
        pass.draw(0..4, 0..1);

        Ok(())
    }

    fn participated_phases() -> &'static [DrawPhase] {
        &[DrawPhase::Compositing]
    }
}
