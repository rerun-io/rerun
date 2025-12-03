use smallvec::smallvec;

use super::{
    DrawData, DrawError, DrawPhase, GpuRenderPipelinePoolAccessor, RenderContext, Renderer,
};
use crate::renderer::{DrawDataDrawable, DrawInstruction, DrawableCollectionViewInfo};
use crate::view_builder::ViewBuilder;
use crate::wgpu_resources::{GpuRenderPipelineHandle, PipelineLayoutDesc, RenderPipelineDesc};
use crate::{DrawableCollector, include_shader_module};

pub struct TestTriangle {
    render_pipeline: GpuRenderPipelineHandle,
}

#[derive(Clone)]
pub struct TestTriangleDrawData;

impl DrawData for TestTriangleDrawData {
    type Renderer = TestTriangle;

    fn collect_drawables(
        &self,
        _view_info: &DrawableCollectionViewInfo,
        collector: &mut DrawableCollector<'_>,
    ) {
        collector.add_drawable(
            DrawPhase::Opaque,
            DrawDataDrawable {
                distance_sort_key: 0.0,
                draw_data_payload: 0,
            },
        );
    }
}

impl TestTriangleDrawData {
    pub fn new(_ctx: &RenderContext) -> Self {
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
        draw_instructions: &[DrawInstruction<'_, Self::RendererDrawData>],
    ) -> Result<(), DrawError> {
        let pipeline = render_pipelines.get(self.render_pipeline)?;
        pass.set_pipeline(pipeline);
        for DrawInstruction { .. } in draw_instructions {
            pass.draw(0..3, 0..1);
        }

        Ok(())
    }
}
