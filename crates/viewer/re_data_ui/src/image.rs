use egui::{Color32, NumExt as _, Vec2};

use re_renderer::renderer::ColormappedTexture;
use re_types::{
    components::ClassId, datatypes::ColorModel, image::ImageKind, tensor_data::TensorElement,
};
use re_viewer_context::{
    gpu_bridge::{self, image_to_gpu},
    Annotations, ImageInfo, ImageStats, ImageStatsCache, UiLayout, ViewerContext,
};

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
) -> Option<()> {
    let render_ctx = ctx.render_ctx?;
    let image_stats = ctx.cache.entry(|c: &mut ImageStatsCache| c.entry(image));
    let annotations = crate::annotations(ctx, query, entity_path);
    let debug_name = entity_path.to_string();
    let texture = image_to_gpu(render_ctx, &debug_name, image, &image_stats, &annotations).ok()?;
    texture_preview_ui(render_ctx, ui, ui_layout, entity_path, texture);
    Some(())
}

/// Show the given texture with an appropriate size.
fn texture_preview_ui(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    entity_path: &re_log_types::EntityPath,
    texture: ColormappedTexture,
) {
    if ui_layout.is_single_line() {
        let preview_size = Vec2::splat(ui.available_height());
        let debug_name = entity_path.to_string();
        ui.allocate_ui_with_layout(
            preview_size,
            egui::Layout::centered_and_justified(egui::Direction::TopDown),
            |ui| {
                ui.set_min_size(preview_size);

                match show_image_preview(render_ctx, ui, texture.clone(), &debug_name, preview_size)
                {
                    Ok(response) => response.on_hover_ui(|ui| {
                        // Show larger image on hover.
                        let hover_size = Vec2::splat(400.0);
                        show_image_preview(render_ctx, ui, texture, &debug_name, hover_size).ok();
                    }),
                    Err((response, err)) => response.on_hover_text(err.to_string()),
                }
            },
        );
    } else {
        let size_range = if ui_layout == UiLayout::Tooltip {
            egui::Rangef::new(64.0, 128.0)
        } else {
            egui::Rangef::new(240.0, 640.0)
        };
        let preview_size = Vec2::splat(
            size_range
                .clamp(ui.available_width())
                .at_most(16.0 * texture.texture.width().max(texture.texture.height()) as f32),
        );
        let debug_name = entity_path.to_string();
        show_image_preview(render_ctx, ui, texture, &debug_name, preview_size).unwrap_or_else(
            |(response, err)| {
                re_log::warn_once!("Failed to show texture {entity_path}: {err}");
                response
            },
        );
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
        debug_name,
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

// Show the surrounding pixels:
const ZOOMED_IMAGE_TEXEL_RADIUS: isize = 10;

pub fn show_zoomed_image_region_area_outline(
    egui_ctx: &egui::Context,
    ui_clip_rect: egui::Rect,
    image_resolution: egui::Vec2,
    [center_x, center_y]: [isize; 2],
    image_rect: egui::Rect,
) {
    use egui::{pos2, remap, Rect};

    let width = image_resolution.x;
    let height = image_resolution.y;

    // Show where on the original image the zoomed-in region is at:
    // The area shown is ZOOMED_IMAGE_TEXEL_RADIUS _surrounding_ the center.
    // Since the center is the top-left/rounded-down, coordinate, we need to add 1 to right/bottom.
    let left = (center_x - ZOOMED_IMAGE_TEXEL_RADIUS) as f32;
    let right = (center_x + ZOOMED_IMAGE_TEXEL_RADIUS + 1) as f32;
    let top = (center_y - ZOOMED_IMAGE_TEXEL_RADIUS) as f32;
    let bottom = (center_y + ZOOMED_IMAGE_TEXEL_RADIUS + 1) as f32;

    let left = remap(left, 0.0..=width, image_rect.x_range());
    let right = remap(right, 0.0..=width, image_rect.x_range());
    let top = remap(top, 0.0..=height, image_rect.y_range());
    let bottom = remap(bottom, 0.0..=height, image_rect.y_range());

    let sample_rect = Rect::from_min_max(pos2(left, top), pos2(right, bottom));
    // TODO(emilk): use `parent_ui.painter()` and put it in a high Z layer, when https://github.com/emilk/egui/issues/1516 is done
    let painter = egui_ctx.debug_painter().with_clip_rect(ui_clip_rect);
    painter.rect_stroke(sample_rect, 0.0, (2.0, Color32::BLACK));
    painter.rect_stroke(sample_rect, 0.0, (1.0, Color32::WHITE));
}

/// `meter`: iff this is a depth map, how long is one meter?
#[allow(clippy::too_many_arguments)]
pub fn show_zoomed_image_region(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    image: &ImageInfo,
    image_stats: &ImageStats,
    annotations: &Annotations,
    meter: Option<f32>,
    debug_name: &str,
    center_texel: [isize; 2],
) {
    let texture =
        match gpu_bridge::image_to_gpu(render_ctx, debug_name, image, image_stats, annotations) {
            Ok(texture) => texture,
            Err(err) => {
                ui.label(format!("Error: {err}"));
                return;
            }
        };

    if let Err(err) = try_show_zoomed_image_region(
        render_ctx,
        ui,
        image,
        texture,
        annotations,
        meter,
        debug_name,
        center_texel,
    ) {
        ui.label(format!("Error: {err}"));
    }
}

/// `meter`: iff this is a depth map, how long is one meter?
#[allow(clippy::too_many_arguments)]
fn try_show_zoomed_image_region(
    render_ctx: &re_renderer::RenderContext,
    ui: &mut egui::Ui,
    image: &ImageInfo,
    texture: ColormappedTexture,
    annotations: &Annotations,
    meter: Option<f32>,
    debug_name: &str,
    center_texel: [isize; 2],
) -> anyhow::Result<()> {
    let (width, height) = (image.width(), image.height());

    const POINTS_PER_TEXEL: f32 = 5.0;
    let size = Vec2::splat(((ZOOMED_IMAGE_TEXEL_RADIUS * 2 + 1) as f32) * POINTS_PER_TEXEL);

    let (_id, zoom_rect) = ui.allocate_space(size);
    let painter = ui.painter();

    painter.rect_filled(zoom_rect, 0.0, ui.visuals().extreme_bg_color);

    let center_of_center_texel = egui::vec2(
        (center_texel[0] as f32) + 0.5,
        (center_texel[1] as f32) + 0.5,
    );

    // Paint the zoomed in region:
    {
        let image_rect_on_screen = egui::Rect::from_min_size(
            zoom_rect.center() - POINTS_PER_TEXEL * center_of_center_texel,
            POINTS_PER_TEXEL * egui::vec2(width as f32, height as f32),
        );

        gpu_bridge::render_image(
            render_ctx,
            &painter.with_clip_rect(zoom_rect),
            image_rect_on_screen,
            texture.clone(),
            egui::TextureOptions::NEAREST,
            debug_name,
        )?;
    }

    // Outline the center texel, to indicate which texel we're printing the values of:
    {
        let center_texel_rect =
            egui::Rect::from_center_size(zoom_rect.center(), Vec2::splat(POINTS_PER_TEXEL));
        painter.rect_stroke(center_texel_rect.expand(1.0), 0.0, (1.0, Color32::BLACK));
        painter.rect_stroke(center_texel_rect, 0.0, (1.0, Color32::WHITE));
    }

    let [x, y] = center_texel;
    if 0 <= x && (x as u32) < width && 0 <= y && (y as u32) < height {
        ui.separator();

        ui.vertical(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

            image_pixel_value_ui(ui, image, annotations, [x as _, y as _], meter);

            // Show a big sample of the color of the middle texel:
            let (rect, _) =
                ui.allocate_exact_size(Vec2::splat(ui.available_height()), egui::Sense::hover());
            // Position texture so that the center texel is at the center of the rect:
            let zoom = rect.width();
            let image_rect_on_screen = egui::Rect::from_min_size(
                rect.center() - zoom * center_of_center_texel,
                zoom * egui::vec2(width as f32, height as f32),
            );
            gpu_bridge::render_image(
                render_ctx,
                &ui.painter().with_clip_rect(rect),
                image_rect_on_screen,
                texture,
                egui::TextureOptions::NEAREST,
                debug_name,
            )
        })
        .inner?;
    }
    Ok(())
}

fn image_pixel_value_ui(
    ui: &mut egui::Ui,
    image: &ImageInfo,
    annotations: &Annotations,
    [x, y]: [u32; 2],
    meter: Option<f32>,
) {
    egui::Grid::new("hovered pixel properties").show(ui, |ui| {
        ui.label("Position:");
        ui.label(format!("{x}, {y}"));
        ui.end_row();

        // Check for annotations on any single-channel image
        if image.kind == ImageKind::Segmentation {
            if let Some(raw_value) = image.get_xyc(x, y, 0) {
                if let (ImageKind::Segmentation, Some(u16_val)) =
                    (image.kind, raw_value.try_as_u16())
                {
                    ui.label("Label:");
                    ui.label(
                        annotations
                            .resolved_class_description(Some(ClassId::from(u16_val)))
                            .annotation_info()
                            .label(None)
                            .unwrap_or_else(|| u16_val.to_string()),
                    );
                    ui.end_row();
                };
            }
        }
        if let Some(meter) = meter {
            // This is a depth map
            if let Some(raw_value) = image.get_xyc(x, y, 0) {
                let raw_value = raw_value.as_f64();
                let meters = raw_value / (meter as f64);
                ui.label("Depth:");
                if meters < 1.0 {
                    ui.monospace(format!("{:.1} mm", meters * 1e3));
                } else {
                    ui.monospace(format!("{meters:.3} m"));
                }
            }
        }
    });

    let text = match image.kind {
        ImageKind::Segmentation | ImageKind::Depth => {
            image.get_xyc(x, y, 0).map(|v| format!("Val: {v}"))
        }

        ImageKind::Color => match image.color_model() {
            ColorModel::L => image.get_xyc(x, y, 0).map(|v| format!("L: {v}")),

            ColorModel::RGB => {
                if let Some([r, g, b]) = {
                    if let [Some(r), Some(g), Some(b)] = [
                        image.get_xyc(x, y, 0),
                        image.get_xyc(x, y, 1),
                        image.get_xyc(x, y, 2),
                    ] {
                        Some([r, g, b])
                    } else {
                        None
                    }
                } {
                    match (r, g, b) {
                        (TensorElement::U8(r), TensorElement::U8(g), TensorElement::U8(b)) => {
                            Some(format!("R: {r}, G: {g}, B: {b}, #{r:02X}{g:02X}{b:02X}"))
                        }
                        _ => Some(format!("R: {r}, G: {g}, B: {b}")),
                    }
                } else {
                    None
                }
            }

            ColorModel::RGBA => {
                if let (Some(r), Some(g), Some(b), Some(a)) = (
                    image.get_xyc(x, y, 0),
                    image.get_xyc(x, y, 1),
                    image.get_xyc(x, y, 2),
                    image.get_xyc(x, y, 3),
                ) {
                    match (r, g, b, a) {
                        (
                            TensorElement::U8(r),
                            TensorElement::U8(g),
                            TensorElement::U8(b),
                            TensorElement::U8(a),
                        ) => Some(format!(
                            "R: {r}, G: {g}, B: {b}, A: {a}, #{r:02X}{g:02X}{b:02X}{a:02X}"
                        )),
                        _ => Some(format!("R: {r}, G: {g}, B: {b}, A: {a}")),
                    }
                } else {
                    None
                }
            }
        },
    };

    if let Some(text) = text {
        ui.label(text);
    } else {
        ui.label("No Value");
    }
}
