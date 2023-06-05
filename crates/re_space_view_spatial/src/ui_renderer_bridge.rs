use re_renderer::{renderer::GenericSkyboxDrawData, view_builder::ViewBuilder, RenderContext};

pub enum ScreenBackground {
    GenericSkybox,
    ClearColor(re_renderer::Rgba),
}

pub fn fill_view_builder(
    render_ctx: &mut RenderContext,
    view_builder: &mut ViewBuilder,
    background: &ScreenBackground,
) -> anyhow::Result<wgpu::CommandBuffer> {
    re_tracing::profile_function!();

    if matches!(background, ScreenBackground::GenericSkybox) {
        view_builder.queue_draw(GenericSkyboxDrawData::new(render_ctx));
    }

    let command_buffer = view_builder.draw(
        render_ctx,
        match background {
            ScreenBackground::GenericSkybox => re_renderer::Rgba::TRANSPARENT,
            ScreenBackground::ClearColor(c) => *c,
        },
    )?;

    Ok(command_buffer)
}
