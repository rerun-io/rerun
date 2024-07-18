use egui::{Color32, Vec2};
use itertools::Itertools as _;

use re_chunk_store::RowId;
use re_log_types::EntityPath;
use re_renderer::renderer::ColormappedTexture;
use re_types::components::{ClassId, ColorModel};
use re_types::datatypes::{TensorData, TensorDimension};
use re_types::tensor_data::{TensorDataMeaning, TensorElement};
use re_ui::UiExt as _;
use re_viewer_context::{
    gpu_bridge, Annotations, ImageInfo, TensorStats, TensorStatsCache, UiLayout, ViewerContext,
};

use super::EntityDataUi;

pub fn format_tensor_shape_single_line(shape: &[TensorDimension]) -> String {
    const MAX_SHOWN: usize = 4; // should be enough for width/height/depth and then some!
    let iter = shape.iter().take(MAX_SHOWN);
    let labelled = iter.clone().any(|dim| dim.name.is_some());
    let shapes = iter
        .map(|dim| {
            format!(
                "{}{}",
                dim.size,
                if let Some(name) = &dim.name {
                    format!(" ({name})")
                } else {
                    String::new()
                }
            )
        })
        .join(if labelled { " × " } else { "×" });
    format!(
        "{shapes}{}",
        if shape.len() > MAX_SHOWN {
            if labelled {
                " × …"
            } else {
                "×…"
            }
        } else {
            ""
        }
    )
}

impl EntityDataUi for re_types::components::TensorData {
    fn entity_data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        entity_path: &EntityPath,
        query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        re_tracing::profile_function!();

        // TODO(#5607): what should happen if the promise is still pending?
        let tensor_data_row_id = ctx
            .recording()
            .latest_at_component::<Self>(entity_path, query)
            .map_or(RowId::ZERO, |tensor| tensor.index.1);

        tensor_ui(ctx, ui, ui_layout, tensor_data_row_id, &self.0);
    }
}

pub fn tensor_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    tensor_data_row_id: RowId,
    tensor: &TensorData,
) {
    // See if we can convert the tensor to a GPU texture.
    // Even if not, we will show info about the tensor.
    let tensor_stats = ctx
        .cache
        .entry(|c: &mut TensorStatsCache| c.entry(tensor_data_row_id, tensor));

    match ui_layout {
        UiLayout::List => {
            ui.horizontal(|ui| {
                let shape = match tensor.image_height_width_channels() {
                    Some([h, w, c]) => vec![
                        TensorDimension::height(h),
                        TensorDimension::width(w),
                        TensorDimension::depth(c),
                    ],
                    None => tensor.shape.clone(),
                };
                let text = format!(
                    "{}, {}",
                    tensor.dtype(),
                    format_tensor_shape_single_line(&shape)
                );
                ui_layout.label(ui, text).on_hover_ui(|ui| {
                    tensor_summary_ui(ui, tensor, &tensor_stats);
                });
            });
        }

        UiLayout::SelectionPanelFull | UiLayout::SelectionPanelLimitHeight | UiLayout::Tooltip => {
            ui.vertical(|ui| {
                ui.set_min_width(100.0);
                tensor_summary_ui(ui, tensor, &tensor_stats);
            });
        }
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
#[allow(dead_code)] // TODO(#6891): use again when we can view image archetypes in the selection view
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
        egui::TextureOptions::LINEAR,
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

pub fn tensor_summary_ui_grid_contents(
    ui: &mut egui::Ui,
    tensor: &TensorData,
    tensor_stats: &TensorStats,
) {
    let TensorData { shape, buffer: _ } = tensor;

    ui.grid_left_hand_label("Data type")
        .on_hover_text("Data type used for all individual elements within the tensor");
    ui.label(tensor.dtype().to_string());
    ui.end_row();

    ui.grid_left_hand_label("Shape")
        .on_hover_text("Extent of every dimension");
    ui.vertical(|ui| {
        // For unnamed tensor dimension more than a single line usually doesn't make sense!
        // But what if some are named and some are not?
        // -> If more than 1 is named, make it a column!
        if shape.iter().filter(|d| d.name.is_some()).count() > 1 {
            for dim in shape {
                ui.label(dim.to_string());
            }
        } else {
            ui.label(format_tensor_shape_single_line(shape));
        }
    });
    ui.end_row();

    let TensorStats {
        range,
        finite_range,
    } = tensor_stats;

    if let Some((min, max)) = range {
        ui.label("Data range")
            .on_hover_text("All values of the tensor range within these bounds");
        ui.monospace(format!(
            "[{} - {}]",
            re_format::format_f64(*min),
            re_format::format_f64(*max)
        ));
        ui.end_row();
    }
    // Show finite range only if it is different from the actual range.
    if let (true, Some((min, max))) = (range != finite_range, finite_range) {
        ui.label("Finite data range").on_hover_text(
            "The finite values (ignoring all NaN & -Inf/+Inf) of the tensor range within these bounds"
        );
        ui.monospace(format!(
            "[{} - {}]",
            re_format::format_f64(*min),
            re_format::format_f64(*max)
        ));
        ui.end_row();
    }
}

pub fn tensor_summary_ui(ui: &mut egui::Ui, tensor: &TensorData, tensor_stats: &TensorStats) {
    egui::Grid::new("tensor_summary_ui")
        .num_columns(2)
        .show(ui, |ui| {
            tensor_summary_ui_grid_contents(ui, tensor, tensor_stats);
        });
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
    tensor_stats: &TensorStats,
    annotations: &Annotations,
    meter: Option<f32>,
    debug_name: &str,
    center_texel: [isize; 2],
) {
    let texture =
        match gpu_bridge::image_to_gpu(render_ctx, debug_name, image, tensor_stats, annotations) {
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
        if image.meaning == TensorDataMeaning::ClassId {
            if let Some(raw_value) = image.get_xyc(x, y, 0) {
                if let (TensorDataMeaning::ClassId, Some(u16_val)) =
                    (image.meaning, raw_value.try_as_u16())
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

    let text = match image.meaning {
        TensorDataMeaning::ClassId | TensorDataMeaning::Depth => {
            image.get_xyc(x, y, 0).map(|v| format!("Val: {v}"))
        }

        TensorDataMeaning::Unknown => match image.color_model() {
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

#[allow(dead_code)] // TODO(#6891): use again when we can view image archetypes in the selection view
fn rgb8_histogram_ui(ui: &mut egui::Ui, rgb: &[u8]) -> egui::Response {
    re_tracing::profile_function!();

    let mut histograms = [[0_u64; 256]; 3];
    {
        // TODO(emilk): this is slow, so cache the results!
        re_tracing::profile_scope!("build");
        for pixel in rgb.chunks_exact(3) {
            for c in 0..3 {
                histograms[c][pixel[c] as usize] += 1;
            }
        }
    }

    use egui_plot::{Bar, BarChart, Legend, Plot};

    let names = ["R", "G", "B"];
    let colors = [Color32::RED, Color32::GREEN, Color32::BLUE];

    let charts = histograms
        .into_iter()
        .enumerate()
        .map(|(component, histogram)| {
            let fill = colors[component].linear_multiply(0.5);

            BarChart::new(
                histogram
                    .into_iter()
                    .enumerate()
                    .map(|(i, count)| {
                        Bar::new(i as _, count as _)
                            .width(0.9)
                            .fill(fill)
                            .vertical()
                            .stroke(egui::Stroke::NONE)
                    })
                    .collect(),
            )
            .color(colors[component])
            .name(names[component])
        })
        .collect_vec();

    re_tracing::profile_scope!("show");
    Plot::new("rgb_histogram")
        .legend(Legend::default())
        .height(200.0)
        .show_axes([false; 2])
        .show(ui, |plot_ui| {
            for chart in charts {
                plot_ui.bar_chart(chart);
            }
        })
        .response
}

#[allow(dead_code)] // TODO(#6891): use again when we can view image archetypes in the selection view
#[cfg(not(target_arch = "wasm32"))]
fn copy_and_save_image_ui(ui: &mut egui::Ui, tensor: &TensorData, _encoded_tensor: &TensorData) {
    ui.horizontal(|ui| {
        if tensor.could_be_dynamic_image() && ui.button("Click to copy image").clicked() {
            match tensor.to_dynamic_image() {
                Ok(dynamic_image) => {
                    let rgba = dynamic_image.to_rgba8();
                    re_viewer_context::Clipboard::with(|clipboard| {
                        clipboard.set_image(
                            [rgba.width() as _, rgba.height() as _],
                            bytemuck::cast_slice(rgba.as_raw()),
                        );
                    });
                }
                Err(err) => {
                    re_log::error!("Failed to convert tensor to image: {err}");
                }
            }
        }

        if ui.button("Save image…").clicked() {
            match tensor.to_dynamic_image() {
                Ok(dynamic_image) => {
                    save_image(&dynamic_image);
                }
                Err(err) => {
                    re_log::error!("Failed to convert tensor to image: {err}");
                }
            }
        }
    });
}

#[allow(dead_code)] // TODO(#6891): use again when we can view image archetypes in the selection view
#[cfg(not(target_arch = "wasm32"))]
fn save_image(dynamic_image: &image::DynamicImage) {
    if let Some(path) = rfd::FileDialog::new()
        .set_file_name("image.png")
        .save_file()
    {
        match dynamic_image.save(&path) {
            Ok(()) => {
                re_log::info!("Image saved to {path:?}");
            }
            Err(err) => {
                re_log::error!("Failed saving image to {path:?}: {err}");
            }
        }
    }
}
