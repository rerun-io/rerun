use anyhow::Context;

use crate::{
    draw_phases::DrawPhase,
    renderer::{DrawData, Renderer},
    RenderContext,
};

type DrawFn = dyn for<'a, 'b> Fn(
        &'b RenderContext,
        DrawPhase,
        &'a mut wgpu::RenderPass<'b>,
        &'b dyn std::any::Any,
    ) -> anyhow::Result<()>
    + Sync
    + Send;

/// Type erased draw data that can be submitted directly to the view builder.
pub struct QueueableDrawData {
    pub(crate) draw_func: Box<DrawFn>,
    pub(crate) draw_data: Box<dyn std::any::Any + std::marker::Send + std::marker::Sync>,
    pub(crate) renderer_name: &'static str,
    pub(crate) participated_phases: &'static [DrawPhase],
}

impl<D: DrawData + Sync + Send + 'static> From<D> for QueueableDrawData {
    fn from(draw_data: D) -> Self {
        QueueableDrawData {
            draw_func: Box::new(move |ctx, phase, pass, draw_data| {
                let renderers = ctx.renderers.read();
                let renderer = renderers
                    .get::<D::Renderer>()
                    .context("failed to retrieve renderer")?;
                let draw_data = draw_data
                    .downcast_ref::<D>()
                    .expect("passed wrong type of draw data");
                renderer.draw(&ctx.gpu_resources, phase, pass, draw_data)
            }),
            draw_data: Box::new(draw_data),
            renderer_name: std::any::type_name::<D::Renderer>(),
            participated_phases: D::Renderer::participated_phases(),
        }
    }
}
