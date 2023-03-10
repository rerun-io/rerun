mod generic_skybox;
pub use generic_skybox::GenericSkyboxDrawData;

mod lines;
pub use lines::{gpu_data::LineVertex, LineBatchInfo, LineDrawData, LineStripFlags, LineStripInfo};

mod point_cloud;
pub use point_cloud::{
    PointCloudBatchFlags, PointCloudBatchInfo, PointCloudDrawData, PointCloudDrawDataError,
    PointCloudVertex,
};

mod depth_cloud;
pub use self::depth_cloud::{
    DepthCloud, DepthCloudDepthData, DepthCloudDrawData, DepthCloudRenderer,
};

mod test_triangle;
pub use test_triangle::TestTriangleDrawData;

mod rectangles;
pub use rectangles::{RectangleDrawData, TextureFilterMag, TextureFilterMin, TexturedRect};

mod mesh_renderer;
pub(crate) use mesh_renderer::MeshRenderer;
pub use mesh_renderer::{MeshDrawData, MeshInstance};

pub mod compositor;

mod outlines;
pub(crate) use outlines::OutlineMaskProcessor;
pub use outlines::{OutlineConfig, OutlineMaskPreference};

use crate::{
    context::{RenderContext, SharedRendererData},
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
    fn participated_phases() -> &'static [DrawPhase] {
        &[DrawPhase::Opaque]
    }
}

/// Determines a (very rough) order of rendering and describes the active [`wgpu::RenderPass`].
///
/// Currently we do not support sorting *within* a rendering phase!
/// See [#702](https://github.com/rerun-io/rerun/issues/702)
/// Within a phase `DrawData` are drawn in the order they are submitted in.
#[derive(Debug, enumset::EnumSetType)]
pub enum DrawPhase {
    /// Opaque objects, performing reads/writes to the depth buffer.
    ///
    /// Typically they are order independent, so everything uses this same index.
    Opaque,

    /// Background, rendering where depth wasn't written.
    Background,

    /// Render mask for things that should get outlines.
    OutlineMask,

    /// Drawn when compositing with the main target.
    Compositing,
}

/// Gets or creates a vertex shader module for drawing a screen filling triangle.
fn screen_triangle_vertex_shader<Fs: FileSystem>(
    pools: &mut WgpuResourcePools,
    device: &wgpu::Device,
    resolver: &mut FileResolver<Fs>,
) -> crate::wgpu_resources::GpuShaderModuleHandle {
    pools.shader_modules.get_or_create(
        device,
        resolver,
        &crate::wgpu_resources::ShaderModuleDesc {
            label: "screen_triangle (vertex)".into(),
            source: crate::include_file!("../../shader/screen_triangle.wgsl"),
        },
    )
}
