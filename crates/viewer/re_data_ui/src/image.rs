use egui::{NumExt as _, Vec2};

use re_renderer::renderer::ColormappedTexture;
use re_viewer_context::{
    ColormapWithRange, ImageInfo, ImageStatsCache, UiLayout, ViewerContext,
    gpu_bridge::{self, image_to_gpu},
};

/// Show a button letting the user copy the image
pub fn copy_image_button_ui(ui: &mut egui::Ui, image: &ImageInfo, data_range: egui::Rangef) {
    if ui
        .button("Copy image")
        .on_hover_text("Copy image to system clipboard")
        .clicked()
    {
        if let Some(rgba) = image.to_rgba8_image(data_range.into()) {
            let egui_image = egui::ColorImage::from_rgba_unmultiplied(
                [rgba.width() as _, rgba.height() as _],
                bytemuck::cast_slice(rgba.as_raw()),
            );
            ui.ctx().copy_image(egui_image);
        } else {
            re_log::error!("Invalid image");
        }
    }
}

/// Show the given image with an appropriate size.
///
/// For segmentation images, the annotation context is looked up.
pub fn image_preview_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
    image: &ImageInfo,
    colormap_with_range: Option<&ColormapWithRange>,
) -> Option<()> {
    let image_stats = ctx
        .store_context
        .caches
        .entry(|c: &mut ImageStatsCache| c.entry(image));
    let annotations = crate::annotations(ctx, query, entity_path);
    let debug_name = entity_path.to_string();
    let texture = image_to_gpu(
        ctx.render_ctx(),
        &debug_name,
        image,
        &image_stats,
        &annotations,
        colormap_with_range,
    )
    .ok()?;
    texture_preview_ui(ctx.render_ctx(), ui, ui_layout, &debug_name, texture);
    Some(())
}

/// Show the given texture with an appropriate size.
pub fn texture_preview_ui(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    debug_name: &str,
    texture: ColormappedTexture,
) -> egui::Response {
    if ui_layout.is_single_line() {
        let preview_size = Vec2::splat(ui.available_height());
        ui.allocate_ui_with_layout(
            preview_size,
            egui::Layout::centered_and_justified(egui::Direction::TopDown),
            |ui| {
                ui.set_min_size(preview_size);

                match show_image_preview(render_ctx, ui, texture.clone(), debug_name, preview_size)
                {
                    Ok(response) => response.on_hover_ui(|ui| {
                        // Show larger image on hover.
                        let hover_size = Vec2::splat(400.0);
                        show_image_preview(render_ctx, ui, texture, debug_name, hover_size).ok();
                    }),
                    Err((response, err)) => response.on_hover_text(err.to_string()),
                }
            },
        )
        .inner
    } else {
        // TODO(emilk): we should limit the HEIGHT primarily,
        // since if the image uses up too much vertical space,
        // it is really annoying in the selection panel.
        let size_range = if ui_layout == UiLayout::Tooltip {
            egui::Rangef::new(64.0, 128.0)
        } else {
            egui::Rangef::new(240.0, 320.0)
        };
        let preview_size = Vec2::splat(
            size_range
                .clamp(ui.available_width())
                .at_most(16.0 * texture.texture.width().max(texture.texture.height()) as f32),
        );
        show_image_preview(render_ctx, ui, texture, debug_name, preview_size).unwrap_or_else(
            |(response, err)| {
                re_log::warn_once!("Failed to show texture {debug_name}: {err}");
                response
            },
        )
    }
}

/// Shows preview of an image.
///
/// Displays the image at the desired size, without overshooting it, and preserving aspect ration.
///
/// Extremely small images will be stretched on their thin axis to make them visible.
/// This does not preserve aspect ratio, but we only stretch it to a very thin size, so it is fine.
///
/// Returns error if the image could not be rendered.
fn show_image_preview(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    colormapped_texture: ColormappedTexture,
    debug_name: &str,
    desired_size: egui::Vec2,
) -> Result<egui::Response, (egui::Response, anyhow::Error)> {
    fn texture_size(colormapped_texture: &ColormappedTexture) -> Vec2 {
        let [w, h] = colormapped_texture.width_height();
        egui::vec2(w as f32, h as f32)
    }

    const MIN_SIZE: f32 = 2.0;

    let texture_size = texture_size(&colormapped_texture);

    let scaled_size = largest_size_that_fits_in(texture_size.x / texture_size.y, desired_size);

    // Don't allow images so thin that we cannot see them:
    let scaled_size = scaled_size.max(Vec2::splat(MIN_SIZE));

    let (response, painter) = ui.allocate_painter(scaled_size, egui::Sense::hover());

    // Place it in the center:
    let texture_rect_on_screen = egui::Rect::from_center_size(response.rect.center(), scaled_size);

    if let Err(err) = gpu_bridge::render_image(
        render_ctx,
        &painter,
        texture_rect_on_screen,
        colormapped_texture,
        egui::TextureOptions {
            magnification: egui::TextureFilter::Nearest,
            minification: egui::TextureFilter::Linear,
            ..Default::default()
        },
        debug_name.into(),
    ) {
        let color = ui.visuals().error_fg_color;
        painter.text(
            response.rect.left_top(),
            egui::Align2::LEFT_TOP,
            "🚫",
            egui::FontId::default(),
            color,
        );
        Err((response, err))
    } else {
        Ok(response)
    }
}

fn largest_size_that_fits_in(aspect_ratio: f32, max_size: Vec2) -> Vec2 {
    if aspect_ratio < max_size.x / max_size.y {
        // A thin image in a landscape frame
        egui::vec2(aspect_ratio * max_size.y, max_size.y)
    } else {
        // A wide image in a portrait frame
        egui::vec2(max_size.x, max_size.x / aspect_ratio)
    }
}
