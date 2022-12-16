use re_renderer::{
    renderer::{GenericSkyboxDrawData, MeshDrawData, RectangleDrawData},
    view_builder::{TargetConfiguration, ViewBuilder},
    RenderContext,
};

use super::scene::SceneSpatialPrimitives;

pub fn get_viewport(clip_rect: egui::Rect, pixels_from_point: f32) -> [u32; 2] {
    let min = (clip_rect.min.to_vec2() * pixels_from_point).round();
    let max = (clip_rect.max.to_vec2() * pixels_from_point).round();
    let resolution = max - min;
    [resolution.x as u32, resolution.y as u32]
}

pub fn create_scene_paint_callback(
    render_ctx: &mut RenderContext,
    target_config: TargetConfiguration,
    clip_rect: egui::Rect,
    primitives: &SceneSpatialPrimitives,
    background: &ScreenBackground,
) -> anyhow::Result<egui::PaintCallback> {
    let pixels_from_point = target_config.pixels_from_point;
    let (command_buffer, view_builder) =
        create_and_fill_view_builder(render_ctx, target_config, primitives, background)?;
    Ok(renderer_paint_callback(
        command_buffer,
        view_builder,
        clip_rect,
        pixels_from_point,
    ))
}

pub enum ScreenBackground {
    GenericSkybox,
    ClearColor(re_renderer::Rgba),
}

fn create_and_fill_view_builder(
    render_ctx: &mut RenderContext,
    target_config: TargetConfiguration,
    primitives: &SceneSpatialPrimitives,
    background: &ScreenBackground,
) -> anyhow::Result<(wgpu::CommandBuffer, ViewBuilder)> {
    let mut view_builder = ViewBuilder::default();
    view_builder.setup_view(render_ctx, target_config)?;

    view_builder
        .queue_draw(&MeshDrawData::new(render_ctx, &primitives.mesh_instances()).unwrap())
        .queue_draw(&primitives.line_strips.to_draw_data(render_ctx))
        .queue_draw(&primitives.points.to_draw_data(render_ctx)?)
        .queue_draw(&RectangleDrawData::new(
            render_ctx,
            &primitives.textured_rectangles,
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

    Ok((command_buffer, view_builder))
}

fn renderer_paint_callback(
    command_buffer: wgpu::CommandBuffer,
    view_builder: ViewBuilder,
    clip_rect: egui::Rect,
    pixels_from_point: f32,
) -> egui::PaintCallback {
    // egui paint callback are copyable / not a FnOnce (this in turn is because egui primitives can be callbacks and are copyable)
    let command_buffer = std::sync::Arc::new(egui::mutex::Mutex::new(Some(command_buffer)));
    let view_builder = std::sync::Arc::new(egui::mutex::Mutex::new(Some(view_builder)));

    let screen_position = (clip_rect.min.to_vec2() * pixels_from_point).round();
    let screen_position = glam::vec2(screen_position.x, screen_position.y);

    egui::PaintCallback {
        rect: clip_rect,
        callback: std::sync::Arc::new(
            egui_wgpu::CallbackFn::new()
                .prepare(
                    move |_device, _queue, _encoder, _paint_callback_resources| {
                        let mut command_buffer = command_buffer.lock();
                        vec![std::mem::replace(&mut *command_buffer, None)
                            .expect("egui_wgpu prepare callback called more than once")]
                    },
                )
                .paint(move |_info, render_pass, paint_callback_resources| {
                    crate::profile_scope!("paint");
                    // TODO(andreas): This should work as well but doesn't work in the 3d view.
                    //                  Looks like a bug in egui, but unclear what's going on.
                    //let clip_rect = info.clip_rect_in_pixels();

                    let ctx = paint_callback_resources.get().unwrap();
                    let mut view_builder = view_builder.lock();
                    std::mem::replace(&mut *view_builder, None)
                        .expect("egui_wgpu paint callback called more than once")
                        .composite(ctx, render_pass, screen_position)
                        .unwrap();
                }),
        ),
    }
}
