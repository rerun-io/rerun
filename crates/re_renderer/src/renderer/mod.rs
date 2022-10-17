pub(crate) mod generic_skybox;
pub(crate) mod test_triangle;
pub(crate) mod tonemapper;

use crate::{context::RenderContextConfig, resource_pools::WgpuResourcePools};

/// A Renderer encapsulate the knowledge of how to render a certain kind of primitives.
///
/// It is an immutable, long-lived datastructure that only holds onto resources that will be needed
/// for each of its [`Renderer::draw`] invocations. (typically [`RenderPipeline`]s and [`BindGroupLayout`]s)
/// Any data that might be different per specific [`Renderer::draw`] invocation is stored in
/// [`Renderer::DrawData`] and created using [`Renderer::PrepareData`] by [`Renderer::prepare`].
pub(crate) trait Renderer {
    type PrepareData;
    type DrawData;

    fn create_renderer(
        ctx_config: &RenderContextConfig,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
    ) -> Self;

    fn prepare(
        &self,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        draw_input: &Self::PrepareData,
    ) -> Self::DrawData;

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
}
