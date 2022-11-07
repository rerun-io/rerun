pub mod generic_skybox;
pub use generic_skybox::GenericSkyboxDrawable;

pub mod lines;
pub use lines::{LineDrawable, LineStrip, LineStripFlags};

pub mod point_cloud;
pub use point_cloud::{PointCloudDrawable, PointCloudPoint};

pub mod test_triangle;
pub use test_triangle::TestTriangleDrawable;

mod mesh_renderer;
pub(crate) use mesh_renderer::MeshRenderer;
pub use mesh_renderer::{MeshDrawable, MeshInstance};

pub mod tonemapper;

mod utils;

use crate::{
    context::{RenderContext, SharedRendererData},
    resource_pools::WgpuResourcePools,
    FileResolver, FileSystem,
};

/// GPU sided data used by a [`Renderer`] to draw things to the screen.
pub trait Drawable {
    type Renderer: Renderer<DrawData = Self>;
}

/// A Renderer encapsulate the knowledge of how to render a certain kind of primitives.
///
/// It is an immutable, long-lived datastructure that only holds onto resources that will be needed
/// for each of its [`Renderer::draw`] invocations.
/// Any data that might be different per specific [`Renderer::draw`] invocation is stored in [`Drawable`].
pub trait Renderer {
    type DrawData: Drawable;

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
        draw_data: &Self::DrawData,
    ) -> anyhow::Result<()>;

    /// Relative location in the rendering process when this renderer should be executed.
    /// TODO(andreas): We might want to take [`Drawable`] into account for this.
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
