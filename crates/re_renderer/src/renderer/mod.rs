mod generic_skybox;
pub use generic_skybox::GenericSkyboxDrawData;

mod lines;
pub use lines::{LineBatchInfo, LineDrawData, LineDrawDataError, LineStripFlags};

mod point_cloud;
pub use point_cloud::{
    PointCloudBatchFlags, PointCloudBatchInfo, PointCloudDrawData, PointCloudDrawDataError,
};

mod depth_cloud;
pub use self::depth_cloud::{DepthCloud, DepthCloudDrawData, DepthCloudRenderer, DepthClouds};

mod test_triangle;
pub use test_triangle::TestTriangleDrawData;

mod rectangles;
pub use rectangles::{
    ColorMapper, ColormappedTexture, RectangleDrawData, RectangleOptions, ShaderDecoding,
    TextureFilterMag, TextureFilterMin, TexturedRect,
};

mod mesh_renderer;
pub(crate) use mesh_renderer::MeshRenderer;
pub use mesh_renderer::{MeshDrawData, MeshInstance};

mod compositor;
pub(crate) use compositor::CompositorDrawData;

mod debug_overlay;
pub use debug_overlay::{DebugOverlayDrawData, DebugOverlayError, DebugOverlayRenderer};

pub mod gpu_data {
    pub use super::lines::gpu_data::{LineStripInfo, LineVertex};
    pub use super::point_cloud::gpu_data::PositionRadius;
}

use crate::{
    context::RenderContext,
    draw_phases::DrawPhase,
    include_shader_module,
    wgpu_resources::{GpuRenderPipelinePoolAccessor, PoolError},
};

/// GPU sided data used by a [`Renderer`] to draw things to the screen.
///
/// Valid only for the frame in which it was created (typically uses temp allocations!).
pub trait DrawData {
    type Renderer: Renderer<RendererDrawData = Self> + Send + Sync;
}

#[derive(thiserror::Error, Debug)]
pub enum DrawError {
    #[error(transparent)]
    Pool(#[from] PoolError),
}

/// A Renderer encapsulate the knowledge of how to render a certain kind of primitives.
///
/// It is an immutable, long-lived datastructure that only holds onto resources that will be needed
/// for each of its [`Renderer::draw`] invocations.
/// Any data that might be different per specific [`Renderer::draw`] invocation is stored in [`DrawData`].
pub trait Renderer {
    type RendererDrawData: DrawData;

    fn create_renderer(ctx: &RenderContext) -> Self;

    // TODO(andreas): Some Renderers need to create their own passes, need something like this for that.

    /// Called once per phase given by [`Renderer::participated_phases`].
    fn draw<'a>(
        &self,
        render_pipelines: &'a GpuRenderPipelinePoolAccessor<'a>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &'a Self::RendererDrawData,
    ) -> Result<(), DrawError>;

    /// Combination of flags indicating in which phases [`Renderer::draw`] should be called.
    fn participated_phases() -> &'static [DrawPhase];
}

/// Gets or creates a vertex shader module for drawing a screen filling triangle.
pub fn screen_triangle_vertex_shader(
    ctx: &RenderContext,
) -> crate::wgpu_resources::GpuShaderModuleHandle {
    ctx.gpu_resources.shader_modules.get_or_create(
        ctx,
        &include_shader_module!("../../shader/screen_triangle.wgsl"),
    )
}
