mod generic_skybox;
pub use generic_skybox::GenericSkyboxDrawData;

mod lines;
pub use lines::{
    gpu_data::LineVertex, LineBatchInfo, LineDrawData, LineDrawDataError, LineStripFlags,
    LineStripInfo,
};

mod point_cloud;
pub use point_cloud::{
    PointCloudBatchFlags, PointCloudBatchInfo, PointCloudDrawData, PointCloudDrawDataError,
    PointCloudVertex,
};

mod depth_cloud;
pub use self::depth_cloud::{
    DepthCloud, DepthCloudDepthData, DepthCloudDrawData, DepthCloudRenderer, DepthClouds,
};

mod test_triangle;
pub use test_triangle::TestTriangleDrawData;

mod rectangles;
pub use rectangles::{RectangleDrawData, TextureFilterMag, TextureFilterMin, TexturedRect};

mod mesh_renderer;
pub(crate) use mesh_renderer::MeshRenderer;
pub use mesh_renderer::{MeshDrawData, MeshInstance};

mod compositor;
pub(crate) use compositor::CompositorDrawData;

mod debug_overlay;
pub use debug_overlay::{DebugOverlayDrawData, DebugOverlayRenderer};

use crate::{
    context::{RenderContext, SharedRendererData},
    draw_phases::DrawPhase,
    include_shader_module,
    wgpu_resources::WgpuResourcePools,
    FileResolver, FileSystem,
};

/// GPU sided data used by a [`Renderer`] to draw things to the screen.
///
/// Valid only for the frame in which it was created (typically uses temp allocations!).
/// TODO(andreas): Add a mechanism to validate this.
pub trait DrawData {
    type Renderer: Renderer<RendererDrawData = Self> + Send + Sync;
}

/// A Renderer encapsulate the knowledge of how to render a certain kind of primitives.
///
/// It is an immutable, long-lived datastructure that only holds onto resources that will be needed
/// for each of its [`Renderer::draw`] invocations.
/// Any data that might be different per specific [`Renderer::draw`] invocation is stored in [`DrawData`].
pub trait Renderer {
    type RendererDrawData: DrawData;

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self;

    // TODO(andreas): Some Renderers need to create their own passes, need something like this for that.

    /// Called once per phase given by [`Renderer::participated_phases`].
    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &'a Self::RendererDrawData,
    ) -> anyhow::Result<()>;

    /// Combination of flags indicating in which phases [`Renderer::draw`] should be called.
    fn participated_phases() -> &'static [DrawPhase];
}

/// Gets or creates a vertex shader module for drawing a screen filling triangle.
pub fn screen_triangle_vertex_shader<Fs: FileSystem>(
    pools: &mut WgpuResourcePools,
    device: &wgpu::Device,
    resolver: &mut FileResolver<Fs>,
) -> crate::wgpu_resources::GpuShaderModuleHandle {
    pools.shader_modules.get_or_create(
        device,
        resolver,
        &include_shader_module!("../../shader/screen_triangle.wgsl"),
    )
}
