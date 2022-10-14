use crate::context::RenderContext;

pub trait Renderer {
    fn new(ctx: &mut RenderContext, device: &wgpu::Device) -> Self;
}

// TODO(andreas) What purpose does this trait actually serve? It's always fully generic, so all it does is establishing a pattern
pub trait RendererImpl<DrawInput, DrawData>
where
    Self: Renderer,
{
    fn build_draw_data(
        &self,
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        draw_input: &DrawInput,
    ) -> DrawData;

    fn draw<'a>(
        &self,
        ctx: &'a RenderContext,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &DrawData,
    ) -> anyhow::Result<()>;
}
