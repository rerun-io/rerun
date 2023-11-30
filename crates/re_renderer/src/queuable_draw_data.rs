use crate::{
    context::Renderers,
    draw_phases::DrawPhase,
    renderer::{DrawData, DrawError, Renderer},
    wgpu_resources::GpuRenderPipelinePoolAccessor,
};

#[derive(thiserror::Error, Debug)]
pub enum QueueableDrawDataError {
    #[error("Failed to retrieve renderer of type {0}")]
    FailedToRetrieveRenderer(&'static str),

    #[error(transparent)]
    DrawError(#[from] DrawError),

    #[error("Mismatching draw data type, expected {0}")]
    UnexpectedDrawDataType(&'static str),
}

type DrawFn = dyn for<'a, 'b> Fn(
        &Renderers,
        &'b GpuRenderPipelinePoolAccessor<'b>,
        DrawPhase,
        &'a mut wgpu::RenderPass<'b>,
        &'b dyn std::any::Any,
    ) -> Result<(), QueueableDrawDataError>
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
            draw_func: Box::new(move |renderers, gpu_resources, phase, pass, draw_data| {
                let renderer = renderers.get::<D::Renderer>().ok_or(
                    QueueableDrawDataError::FailedToRetrieveRenderer(std::any::type_name::<
                        D::Renderer,
                    >()),
                )?;
                let draw_data = draw_data.downcast_ref::<D>().ok_or(
                    QueueableDrawDataError::UnexpectedDrawDataType(std::any::type_name::<D>()),
                )?;
                renderer
                    .draw(gpu_resources, phase, pass, draw_data)
                    .map_err(QueueableDrawDataError::from)
            }),
            draw_data: Box::new(draw_data),
            renderer_name: std::any::type_name::<D::Renderer>(),
            participated_phases: D::Renderer::participated_phases(),
        }
    }
}
