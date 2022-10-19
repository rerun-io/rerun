mod generic_skybox;
mod test_triangle;
pub(crate) mod tonemapper;

use anyhow::Context;
pub use generic_skybox::GenericSkyboxDrawData;
pub use test_triangle::TestTriangleDrawData;

use crate::{
    context::{RenderContext, SharedRendererData},
    resource_pools::WgpuResourcePools,
};

/// GPU sided data used by a [`Renderer`] to draw things to the screen.
pub trait DrawData {
    /// Internal draw method called by [`crate::view_builder::ViewBuilder`]
    fn draw<'a>(
        &self,
        ctx: &'a RenderContext,
        pass: &mut wgpu::RenderPass<'a>,
    ) -> anyhow::Result<()>;
}

/// Internal implementation utility that redirects [`DrawData::draw`] to the correct [`Renderer`]
pub(crate) trait DrawDataImpl {
    type Renderer: Renderer<D = Self> + Send + Sync;
}

impl<T> DrawData for T
where
    T: DrawDataImpl + 'static,
{
    fn draw<'a>(
        &self,
        ctx: &'a RenderContext,
        pass: &mut wgpu::RenderPass<'a>,
    ) -> anyhow::Result<()> {
        ctx.renderers
            .get::<T::Renderer>()
            .context("Renderer type was not yet instantiated. Creation of the respective DrawData should have made sure of that.")?
            .draw(&ctx.resource_pools, pass, self)
    }
}

/// A Renderer encapsulate the knowledge of how to render a certain kind of primitives.
///
/// It is an immutable, long-lived datastructure that only holds onto resources that will be needed
/// for each of its [`Renderer::draw`] invocations.
/// Any data that might be different per specific [`Renderer::draw`] invocation is stored in [`DrawData`].
pub(crate) trait Renderer {
    type D: DrawData;

    fn create_renderer(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
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
        draw_data: &Self::D,
    ) -> anyhow::Result<()>;
}
