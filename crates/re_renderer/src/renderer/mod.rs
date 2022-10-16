pub(crate) mod test_triangle;
pub(crate) mod tonemapper;

use crate::{context::RenderContextConfig, resource_pools::WgpuResourcePools};

/// TODO: clever documentation something something amazing immutable
pub(crate) trait Renderer {
    type PrepareData;
    type DrawData;

    fn new(
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
