use egui::mutex::Mutex;
use re_renderer::{
    renderer::{DepthCloudDrawData, GenericSkyboxDrawData, MeshDrawData, RectangleDrawData},
    view_builder::ViewBuilder,
    GpuReadbackIdentifier, RenderContext,
};

use crate::ui::space_view::ScreenshotMode;

use super::scene::SceneSpatialPrimitives;

pub fn get_viewport(clip_rect: egui::Rect, pixels_from_point: f32) -> [u32; 2] {
    let min = (clip_rect.min.to_vec2() * pixels_from_point).round();
    let max = (clip_rect.max.to_vec2() * pixels_from_point).round();
    let resolution = max - min;
    [resolution.x as u32, resolution.y as u32]
}

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
        .queue_draw(&DepthCloudDrawData::new(render_ctx, &primitives.depth_clouds).unwrap())
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

    Ok(command_buffer)
}

slotmap::new_key_type! { pub struct ViewBuilderHandle; }

type ViewBuilderMap = slotmap::SlotMap<ViewBuilderHandle, ViewBuilder>;

pub fn renderer_paint_callback(
    render_ctx: &mut RenderContext,
    command_buffer: wgpu::CommandBuffer,
    view_builder: ViewBuilder,
    clip_rect: egui::Rect,
    pixels_from_point: f32,
) -> egui::PaintCallback {
    crate::profile_function!();

    // egui paint callback are copyable / not a FnOnce (this in turn is because egui primitives can be callbacks and are copyable)
    let command_buffer = std::sync::Arc::new(Mutex::new(Some(command_buffer)));

    let composition_view_builder_map = render_ctx
        .active_frame
        .per_frame_data_helper
        .entry::<ViewBuilderMap>()
        .or_insert_with(Default::default);
    let view_builder_handle = composition_view_builder_map.insert(view_builder);

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

                    let ctx = paint_callback_resources.get::<RenderContext>().unwrap();
                    ctx.active_frame
                        .per_frame_data_helper
                        .get::<ViewBuilderMap>()
                        .unwrap()[view_builder_handle]
                        .composite(ctx, render_pass, screen_position)
                        .expect("Failed compositing view builder with main target.");
                }),
        ),
    }
}
