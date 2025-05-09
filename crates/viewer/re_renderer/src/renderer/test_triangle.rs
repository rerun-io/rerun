use smallvec::smallvec;

use crate::{
    include_shader_module,
    view_builder::ViewBuilder,
    wgpu_resources::{GpuRenderPipelineHandle, PipelineLayoutDesc, RenderPipelineDesc},
};

use super::{
    DrawData, DrawError, DrawPhase, GpuRenderPipelinePoolAccessor, RenderContext, Renderer,
};

pub struct TestTriangle {
    render_pipeline: GpuRenderPipelineHandle,
}

#[derive(Clone)]
pub struct TestTriangleDrawData;

impl DrawData for TestTriangleDrawData {
    type Renderer = TestTriangle;
}

impl TestTriangleDrawData {
    pub fn new(ctx: &RenderContext) -> Self {
        std::mem::drop(ctx.renderer::<TestTriangle>()); // TODO(andreas): This line ensures that the renderer exists. Currently this needs to be done ahead of time, but should be fully automatic!
        Self {}
    }
}

impl Renderer for TestTriangle {
    type RendererDrawData = TestTriangleDrawData;

    fn create_renderer(ctx: &RenderContext) -> Self {
        let shader_modules = &ctx.gpu_resources.shader_modules;
        let render_pipeline = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "TestTriangle::render_pipeline".into(),
                pipeline_layout: ctx.gpu_resources.pipeline_layouts.get_or_create(
                    ctx,
                    &PipelineLayoutDesc {
                        label: "global only".into(),
                        entries: vec![ctx.global_bindings.layout],
                    },
                ),
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: shader_modules.get_or_create(
                    ctx,
                    &include_shader_module!("../../shader/test_triangle.wgsl"),
                ),
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: shader_modules.get_or_create(
                    ctx,
                    &include_shader_module!("../../shader/test_triangle.wgsl"),
                ),
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(ViewBuilder::MAIN_TARGET_COLOR_FORMAT.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: ViewBuilder::MAIN_TARGET_DEPTH_FORMAT,
                    depth_compare: wgpu::CompareFunction::Always,
                    depth_write_enabled: true, // writes some depth for testing
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: ViewBuilder::main_target_default_msaa_state(
                    ctx.render_config(),
                    false,
                ),
            },
        );

        Self { render_pipeline }
    }

    fn draw(
        &self,
        render_pipelines: &GpuRenderPipelinePoolAccessor<'_>,
        _phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        _draw_data: &TestTriangleDrawData,
    ) -> Result<(), DrawError> {
        let pipeline = render_pipelines.get(self.render_pipeline)?;
        pass.set_pipeline(pipeline);
        pass.draw(0..3, 0..1);
        Ok(())
    }

    fn participated_phases() -> &'static [DrawPhase] {
        &[DrawPhase::Opaque]
    }
}
