use std::num::NonZeroU64;

use rerun::external::{
    glam,
    re_renderer::{
        self,
        external::{smallvec::smallvec, wgpu},
    },
};

/// Implements a simple custom [`re_renderer::renderer::Renderer`] for drawing some shader defined 3D ??TODO??.
pub struct CustomRenderer {
    bind_group_layout: re_renderer::GpuBindGroupLayoutHandle,
    render_pipeline: re_renderer::GpuRenderPipelineHandle,
}

mod gpu_data {
    use rerun::external::re_renderer::wgpu_buffer_types;

    /// Keep in sync with [`UniformBuffer`] in `custom.wgsl`
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct UniformBuffer {
        pub world_from_obj: wgpu_buffer_types::Mat4,
        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 4],
    }
}
/// GPU draw data for drawing ??TODO?? instances using [`CustomRenderer`].
///
/// Note that a single draw data is used for many instances of the same drawable.
#[derive(Clone)]
pub struct CustomDrawData {
    /// Bindgroup per instance.
    ///
    /// It is much more efficient to batch everything in a single draw call by using instancing
    /// or other more dynamic buffer access. However, for simplicity, we draw each instance individually
    /// with a separate bind group here.
    bind_groups: Vec<re_renderer::GpuBindGroup>,
}

impl re_renderer::renderer::DrawData for CustomDrawData {
    type Renderer = CustomRenderer;
}

impl CustomDrawData {
    pub fn new(ctx: &re_renderer::RenderContext) -> Self {
        let _ = ctx.renderer::<CustomRenderer>(); // TODO(andreas): This line ensures that the renderer exists. Currently this needs to be done ahead of time, but should be fully automatic!
        Self {
            bind_groups: Vec::new(),
        }
    }

    /// Adds an instance to this draw data.
    pub fn add(
        &mut self,
        ctx: &re_renderer::RenderContext,
        world_from_obj: glam::Affine3A,
        label: &str,
    ) {
        let renderer = ctx.renderer::<CustomRenderer>();

        // See `CustomRenderer::bind_groups`: It would be much more efficient to batch instances,
        // but for simplicity we create fresh buffers here for each instance.
        let bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &re_renderer::BindGroupDesc {
                label: label.into(),
                entries: smallvec![re_renderer::create_and_fill_uniform_buffer(
                    ctx,
                    label.into(),
                    gpu_data::UniformBuffer {
                        world_from_obj: world_from_obj.into(),
                        end_padding: Default::default(),
                    },
                )],
                layout: renderer.bind_group_layout,
            },
        );
        self.bind_groups.push(bind_group);
    }
}

impl re_renderer::renderer::Renderer for CustomRenderer {
    type RendererDrawData = CustomDrawData;

    fn create_renderer(ctx: &re_renderer::RenderContext) -> Self {
        let shader_modules = &ctx.gpu_resources.shader_modules;
        let shader_module = shader_modules.get_or_create(
            ctx,
            &re_renderer::include_shader_module!("../shader/custom.wgsl"),
        );

        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &re_renderer::BindGroupLayoutDesc {
                label: "CustomRenderer::bind_group_layout".into(),
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(
                            std::mem::size_of::<gpu_data::UniformBuffer>() as _,
                        ),
                    },
                    count: None,
                }],
            },
        );

        let pipeline_layout = ctx.gpu_resources.pipeline_layouts.get_or_create(
            ctx,
            &re_renderer::PipelineLayoutDesc {
                label: "CustomRenderer".into(),
                entries: vec![ctx.global_bindings.layout, bind_group_layout],
            },
        );

        let render_pipeline = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &re_renderer::RenderPipelineDesc {
                label: "CustomRenderer::main".into(),
                pipeline_layout,
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: shader_module,
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(
                    re_renderer::ViewBuilder::MAIN_TARGET_COLOR_FORMAT.into()
                )],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: re_renderer::ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE,
                multisample: re_renderer::ViewBuilder::main_target_default_msaa_state(
                    ctx.render_config(),
                    false,
                ),
            },
        );

        Self {
            bind_group_layout,
            render_pipeline,
        }
    }

    fn draw(
        &self,
        render_pipelines: &re_renderer::GpuRenderPipelinePoolAccessor<'_>,
        _phase: re_renderer::DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        _draw_data: &CustomDrawData,
    ) -> Result<(), re_renderer::renderer::DrawError> {
        let pipeline = render_pipelines.get(self.render_pipeline)?;
        pass.set_pipeline(pipeline);

        for bind_group in &_draw_data.bind_groups {
            pass.set_bind_group(1, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        Ok(())
    }

    fn participated_phases() -> &'static [re_renderer::DrawPhase] {
        &[
            re_renderer::DrawPhase::Opaque,
            // TODO(andreas): Demonstrate how to render the outline layer.
            //re_renderer::DrawPhase::OutlineMask,
            // TODO(andreas): Demonstrate how to render the picking layer.
            //re_renderer::DrawPhase::PickingLayer,
        ]
    }
}
