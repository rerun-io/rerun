mod generic_skybox;
pub use generic_skybox::GenericSkyboxDrawData;

mod lines;
pub use lines::{gpu_data::LineVertex, LineBatchInfo, LineDrawData, LineStripFlags, LineStripInfo};

mod point_cloud;
pub use point_cloud::{
    PointCloudBatchFlags, PointCloudBatchInfo, PointCloudDrawData, PointCloudDrawDataError,
    PointCloudVertex,
};

mod test_triangle;
pub use test_triangle::TestTriangleDrawData;

mod rectangles;
pub use rectangles::{RectangleDrawData, TextureFilterMag, TextureFilterMin, TexturedRect};

mod volumes;
pub use self::volumes::{Volume, VolumeDrawData, VolumeRenderer};

mod mesh_renderer;
pub(crate) use mesh_renderer::MeshRenderer;
pub use mesh_renderer::{MeshDrawData, MeshInstance};

pub mod compositor;

use crate::{
    context::{RenderContext, SharedRendererData},
    wgpu_resources::WgpuResourcePools,
    FileResolver, FileSystem,
};

/// GPU sided data used by a [`Renderer`] to draw things to the screen.
///
/// Valid only for the frame in which it was created (typically uses temp allocations!)
/// TODO(andreas): Add a mechanism to validate this.
pub trait DrawData {
    type Renderer: Renderer<RendererDrawData = Self>;
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
    // TODO(andreas): The harder part is that some of those might need to share them with others!
    //                E.g. Shadow Mapping! Conceivable that there are special traits for those (distinguish "ShadowMappingAwareRenderer")
    // fn record_custom_passes<'a>(
    //     &self,
    //     pools: &'a WgpuResourcePools,
    //     pass: &mut wgpu::CommandEncoder,
    //     draw_data: &Self::DrawData,
    // ) -> anyhow::Result<()> {
    // }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &Self::RendererDrawData,
    ) -> anyhow::Result<()>;

    /// Relative location in the rendering process when this renderer should be executed.
    /// TODO(andreas): We might want to take [`DrawData`] into account for this.
    ///                But this touches on the [`Renderer::draw`] method might be split in the future, which haven't designed yet.
    fn draw_order() -> u32 {
        DrawOrder::Opaque as u32
    }
}

/// Assigns rough meaning to draw sorting indices
#[allow(dead_code)]
#[repr(u32)]
enum DrawOrder {
    /// Opaque objects, performing reads/writes to the depth buffer.
    /// Typically they are order independent, so everything uses this same index.
    Opaque = 30000,

    /// Transparent objects. Each draw typically gets its own sorting index.
    Transparent = 50000,

    /// Backgrounds should always be rendered last.
    Background = 70000,

    /// Postprocessing effects that are applied before the final tonemapping step.
    Postprocess = 90000,
}
