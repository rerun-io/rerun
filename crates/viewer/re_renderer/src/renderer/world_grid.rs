use crate::{
    allocator::create_and_fill_uniform_buffer,
    draw_phases::DrawPhase,
    include_shader_module,
    wgpu_resources::{
        BindGroupDesc, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor, PipelineLayoutDesc,
        RenderPipelineDesc,
    },
    ViewBuilder,
};

use super::{DrawData, DrawError, RenderContext, Renderer};
use crate::Rgba;

use smallvec::smallvec;

/// Configuration for the world grid renderer.
pub struct WorldGridConfiguration {
    /// The color of the grid lines.
    pub color: Rgba,

    /// The plane in which the grid lines are drawn.
    pub plane: re_math::Plane3,

    /// How far apart the closest sets of lines are.
    pub spacing: f32,

    /// How thick the lines are in UI units.
    pub thickness_ui: f32,
}

mod gpu_data {
    use crate::wgpu_buffer_types;

    /// Keep in sync with `world_grid.wgsl`
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct WorldGridUniformBuffer {
        pub color: wgpu_buffer_types::Vec4,

        /// Plane equation, normal + distance.
        pub plane: wgpu_buffer_types::Vec4,

        /// How far apart the closest sets of lines are.
        pub spacing: f32,

        /// Radius of the lines in UI units.
        pub thickness_ui: f32,

        pub _padding: [f32; 2],
        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 3],
    }
}

pub struct WorldGridRenderer {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

/// Draw data for a world grid renderer.
#[derive(Clone)]
pub struct WorldGridDrawData {
    bind_group: GpuBindGroup,
}

impl DrawData for WorldGridDrawData {
    type Renderer = WorldGridRenderer;
}

impl WorldGridDrawData {
    pub fn new(ctx: &RenderContext, config: &WorldGridConfiguration) -> Self {
        let world_grid_renderer = ctx.renderer::<WorldGridRenderer>();

        let uniform_buffer_binding = create_and_fill_uniform_buffer(
            ctx,
            "WorldGridDrawData".into(),
            gpu_data::WorldGridUniformBuffer {
                color: config.color.into(),
                plane: config.plane.as_vec4().into(),
                spacing: config.spacing,
                thickness_ui: config.thickness_ui,
                _padding: Default::default(),
                end_padding: Default::default(),
            },
        );

        Self {
            bind_group: ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &ctx.gpu_resources,
                &BindGroupDesc {
                    label: "WorldGrid".into(),
                    entries: smallvec![uniform_buffer_binding],
                    layout: world_grid_renderer.bind_group_layout,
                },
            ),
        }
    }
}

impl Renderer for WorldGridRenderer {
    type RendererDrawData = WorldGridDrawData;

    fn create_renderer(ctx: &RenderContext) -> Self {
        re_tracing::profile_function!();

        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "WorldGrid::bind_group_layout".into(),
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
                            gpu_data::WorldGridUniformBuffer,
                        >()
                            as _),
                    },
                    count: None,
                }],
            },
        );

        let shader_module = ctx
            .gpu_resources
            .shader_modules
            .get_or_create(ctx, &include_shader_module!("../../shader/world_grid.wgsl"));
        let render_pipeline = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "WorldGridDrawData::render_pipeline_regular".into(),
                pipeline_layout: ctx.gpu_resources.pipeline_layouts.get_or_create(
                    ctx,
                    &PipelineLayoutDesc {
                        label: "WorldGrid".into(),
                        entries: vec![ctx.global_bindings.layout, bind_group_layout],
                    },
                ),
                vertex_entrypoint: "main_vs".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "main_fs".into(),
                fragment_handle: shader_module,
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(wgpu::ColorTargetState {
                    format: ViewBuilder::MAIN_TARGET_COLOR_FORMAT,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                primitive: wgpu::PrimitiveState {
                    // drawn as a (close to) infinite quad
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: ViewBuilder::MAIN_TARGET_DEPTH_FORMAT,
                    depth_compare: wgpu::CompareFunction::GreaterEqual,
                    depth_write_enabled: false,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: ViewBuilder::MAIN_TARGET_DEFAULT_MSAA_STATE,
            },
        );
        Self {
            render_pipeline,
            bind_group_layout,
        }
    }

    fn draw(
        &self,
        render_pipelines: &GpuRenderPipelinePoolAccessor<'_>,
        _phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        draw_data: &WorldGridDrawData,
    ) -> Result<(), DrawError> {
        let pipeline = render_pipelines.get(self.render_pipeline)?;

        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, &draw_data.bind_group, &[]);
        pass.draw(0..4, 0..1);

        Ok(())
    }

    fn participated_phases() -> &'static [DrawPhase] {
        &[DrawPhase::Transparent]
    }
}
