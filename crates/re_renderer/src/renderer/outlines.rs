//! Outlines
//!
//! TODO: How do they work, how are they configured. What's going on!

use crate::{
    context::SharedRendererData, wgpu_resources::WgpuResourcePools, FileResolver, FileSystem,
    RenderContext,
};

use super::{DrawData, DrawPhase, Renderer};

pub struct OutlinesRenderer {}

#[derive(Clone)]
pub struct OutlinesDrawData {}

impl DrawData for OutlinesDrawData {
    type Renderer = OutlinesRenderer;
}

impl OutlinesDrawData {
    pub fn new(ctx: &mut RenderContext) -> Self {
        ctx.renderers.write().get_or_create::<_, OutlinesRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.gpu_resources,
            &ctx.device,
            &mut ctx.resolver,
        );

        OutlinesDrawData {}
    }
}

impl Renderer for OutlinesRenderer {
    type RendererDrawData = OutlinesDrawData;

    fn participated_phases() -> &'static [DrawPhase] {
        &[DrawPhase::Compositing]
    }

    fn create_renderer<Fs: FileSystem>(
        _shared_data: &SharedRendererData,
        _pools: &mut WgpuResourcePools,
        _device: &wgpu::Device,
        _resolver: &mut FileResolver<Fs>,
    ) -> Self {
        OutlinesRenderer {}
    }

    fn draw<'a>(
        &self,
        _pools: &'a WgpuResourcePools,
        _phase: DrawPhase,
        _pass: &mut wgpu::RenderPass<'a>,
        _draw_data: &OutlinesDrawData,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
