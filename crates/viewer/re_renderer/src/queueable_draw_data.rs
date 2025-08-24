use crate::{
    context::Renderers,
    draw_phases::DrawPhase,
    renderer::{DrawData, DrawError, Renderer as _},
    wgpu_resources::GpuRenderPipelinePoolAccessor,
};

#[derive(thiserror::Error, Debug)]
pub enum QueueableDrawDataError {
    #[error("Failed to retrieve renderer of type {0}")]
    FailedToRetrieveRenderer(&'static str),

    #[error(transparent)]
    DrawError(#[from] DrawError),
}

pub trait TypeErasedDrawData {
    fn draw(
        &self,
        renderers: &Renderers,
        gpu_resources: &GpuRenderPipelinePoolAccessor<'_>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
    ) -> Result<(), QueueableDrawDataError>;

    fn renderer_name(&self) -> &'static str;

    fn participated_phases(&self) -> &'static [DrawPhase];
}

impl<D: DrawData + 'static> TypeErasedDrawData for D {
    fn draw(
        &self,
        renderers: &Renderers,
        gpu_resources: &GpuRenderPipelinePoolAccessor<'_>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
    ) -> Result<(), QueueableDrawDataError> {
        let renderer = renderers.get::<D::Renderer>().ok_or(
            QueueableDrawDataError::FailedToRetrieveRenderer(std::any::type_name::<D::Renderer>()),
        )?;

        renderer
            .draw(gpu_resources, phase, pass, self)
            .map_err(QueueableDrawDataError::from)
    }

    fn renderer_name(&self) -> &'static str {
        std::any::type_name::<D::Renderer>()
    }

    fn participated_phases(&self) -> &'static [DrawPhase] {
        D::Renderer::participated_phases()
    }
}

/// Type erased draw data that can be submitted directly to the view builder.
pub struct QueueableDrawData(Box<dyn TypeErasedDrawData + Send + Sync>);

impl<D: TypeErasedDrawData + DrawData + Sync + Send + 'static> From<D> for QueueableDrawData {
    fn from(draw_data: D) -> Self {
        Self(Box::new(draw_data))
    }
}

impl std::ops::Deref for QueueableDrawData {
    type Target = dyn TypeErasedDrawData;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}
