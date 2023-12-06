use smallvec::smallvec;

use crate::{
    draw_phases::DrawPhase,
    include_shader_module,
    renderer::screen_triangle_vertex_shader,
    view_builder::ViewBuilder,
    wgpu_resources::{
        GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor, PipelineLayoutDesc,
        RenderPipelineDesc,
    },
};

use super::{DrawData, DrawError, RenderContext, Renderer};

/// Renders a generated skybox from a color gradient
///
/// Is not actually a skybox, but a fullscreen effect.
/// Should be rendered *last* to reduce amount of overdraw!
pub struct GenericSkybox {
    render_pipeline: GpuRenderPipelineHandle,
}

#[derive(Clone)]
pub struct GenericSkyboxDrawData {}

impl DrawData for GenericSkyboxDrawData {
    type Renderer = GenericSkybox;
}

impl GenericSkyboxDrawData {
    pub fn new(ctx: &RenderContext) -> Self {
        let _ = ctx.renderer::<GenericSkybox>(); // TODO(andreas): This line ensures that the renderer exists. Currently this needs to be done ahead of time, but should be fully automatic!
        GenericSkyboxDrawData {}
    }
}

impl Renderer for GenericSkybox {
    type RendererDrawData = GenericSkyboxDrawData;

    fn create_renderer(ctx: &RenderContext) -> Self {
        re_tracing::profile_function!();

        let vertex_handle = screen_triangle_vertex_shader(ctx);
        let render_pipeline = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "GenericSkybox::render_pipeline".into(),
                pipeline_layout: ctx.gpu_resources.pipeline_layouts.get_or_create(
                    ctx,
                    &PipelineLayoutDesc {
                        label: "GenericSkybox::render_pipeline".into(),
                        entries: vec![ctx.global_bindings.layout],
                    },
                ),

                vertex_entrypoint: "main".into(),
                vertex_handle,
                fragment_entrypoint: "main".into(),
                fragment_handle: ctx.gpu_resources.shader_modules.get_or_create(
                    ctx,
                    &include_shader_module!("../../shader/generic_skybox.wgsl"),
                ),
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(ViewBuilder::MAIN_TARGET_COLOR_FORMAT.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: ViewBuilder::MAIN_TARGET_DEPTH_FORMAT,
                    // Pass depth test only if the fragment hasn't been written to.
                    // This allows us to draw the skybox last which is much more efficient than using it as a clear pass!
                    depth_compare: wgpu::CompareFunction::Equal,
                    depth_write_enabled: false,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: ViewBuilder::MAIN_TARGET_DEFAULT_MSAA_STATE,
            },
        );
        GenericSkybox { render_pipeline }
    }

    fn draw<'a>(
        &self,
        render_pipelines: &'a GpuRenderPipelinePoolAccessor<'a>,
        _phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
        _draw_data: &GenericSkyboxDrawData,
    ) -> Result<(), DrawError> {
        re_tracing::profile_function!();

        let pipeline = render_pipelines.get(self.render_pipeline)?;

        pass.set_pipeline(pipeline);
        pass.draw(0..3, 0..1);

        Ok(())
    }

    fn participated_phases() -> &'static [DrawPhase] {
        &[DrawPhase::Background]
    }
}
