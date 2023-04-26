use re_renderer::{
    renderer::{DepthCloudDrawData, GenericSkyboxDrawData, MeshDrawData, RectangleDrawData},
    view_builder::ViewBuilder,
    RenderContext,
};

use super::scene::SceneSpatialPrimitives;

pub enum ScreenBackground {
    GenericSkybox,
    ClearColor(re_renderer::Rgba),
}

pub fn fill_view_builder(
    render_ctx: &mut RenderContext,
    view_builder: &mut ViewBuilder,
    primitives: SceneSpatialPrimitives,
    background: &ScreenBackground,
) -> anyhow::Result<wgpu::CommandBuffer> {
    crate::profile_function!();

    view_builder
        .queue_draw(&DepthCloudDrawData::new(
            render_ctx,
            &primitives.depth_clouds,
        )?)
        .queue_draw(&MeshDrawData::new(
            render_ctx,
            &primitives.mesh_instances(),
        )?)
        .queue_draw(&primitives.line_strips.to_draw_data(render_ctx)?)
        .queue_draw(&primitives.points.to_draw_data(render_ctx)?)
        .queue_draw(&RectangleDrawData::new(
            render_ctx,
            &primitives
                .images
                .iter()
                .map(|image| image.textured_rect.clone())
                .collect::<Vec<_>>(),
        )?);

    if matches!(background, ScreenBackground::GenericSkybox) {
        view_builder.queue_draw(&GenericSkyboxDrawData::new(render_ctx));
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
