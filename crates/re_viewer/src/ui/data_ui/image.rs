use egui::Vec2;
use itertools::Itertools as _;

use re_log_types::{
    component_types::{ClassId, TensorDataMeaning},
    ClassicTensor,
};

use crate::misc::{
    caches::{TensorImageView, TensorStats},
    ViewerContext,
};

use super::{DataUi, UiVerbosity};

pub fn format_tensor_shape_single_line(
    shape: &[re_log_types::component_types::TensorDimension],
) -> String {
    format!("[{}]", shape.iter().join(", "))
}

impl DataUi for re_log_types::component_types::Tensor {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        ClassicTensor::from(self).data_ui(ctx, ui, verbosity, query);
    }
}

impl DataUi for ClassicTensor {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: crate::ui::UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        let tensor_view = ctx.cache.image.get_view(self, ctx.render_ctx);
        let tensor_stats = ctx.cache.tensor_stats.get(&self.id());

        match verbosity {
            UiVerbosity::Small | UiVerbosity::MaxHeight(_) => {
                ui.horizontal_centered(|ui| {
                    if let Some(retained_img) = tensor_view.retained_img {
                        let max_height = match verbosity {
                            UiVerbosity::Small => 24.0,
                            UiVerbosity::All | UiVerbosity::Reduced => 128.0,
                            UiVerbosity::MaxHeight(height) => height,
                        };
                        retained_img
                            .show_max_size(ui, Vec2::new(4.0 * max_height, max_height))
                            .on_hover_ui(|ui| {
                                retained_img.show_max_size(ui, Vec2::splat(400.0));
                            });
                    }

                    ui.label(format!(
                        "{} x {}",
                        self.dtype(),
                        format_tensor_shape_single_line(self.shape())
                    ))
                    .on_hover_ui(|ui| tensor_dtype_and_shape_ui(ctx.re_ui, ui, self, tensor_stats));
                });
            }

            UiVerbosity::All | UiVerbosity::Reduced => {
                ui.vertical(|ui| {
                    ui.set_min_width(100.0);
                    tensor_dtype_and_shape_ui(ctx.re_ui, ui, self, tensor_stats);

                    if let Some(retained_img) = tensor_view.retained_img {
                        let max_size = ui
                            .available_size()
                            .min(retained_img.size_vec2())
                            .min(egui::vec2(150.0, 300.0));
                        let response = retained_img.show_max_size(ui, max_size);

                        let image_rect = response.rect;

                        if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
                            show_zoomed_image_region_tooltip(
                                ui,
                                response,
                                &tensor_view,
                                image_rect,
                                pointer_pos,
                                None,
                            );
                        }
                    }

                    if let Some(dynamic_img) = tensor_view.dynamic_img {
                        // TODO(emilk): support copying and saving images on web
                        #[cfg(not(target_arch = "wasm32"))]
                        ui.horizontal(|ui| image_options(ui, self, dynamic_img));

                        // TODO(emilk): support histograms of non-RGB images too
                        if let image::DynamicImage::ImageRgb8(rgb_image) = dynamic_img {
                            ui.collapsing("Histogram", |ui| {
                                histogram_ui(ui, rgb_image);
                            });
                        }
                    }
                });
            }
        }
    }
}

pub fn tensor_dtype_and_shape_ui_grid_contents(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    tensor: &ClassicTensor,
    tensor_stats: Option<&TensorStats>,
) {
    re_ui
        .grid_left_hand_label(ui, "Data type")
        .on_hover_text("Data type used for all individual elements within the tensor.");
    ui.label(tensor.dtype().to_string());
    ui.end_row();

    re_ui
        .grid_left_hand_label(ui, "Shape")
        .on_hover_text("Extent of every dimension.");
    ui.vertical(|ui| {
        // For unnamed tensor dimension more than a single line usually doesn't make sense!
        // But what if some are named and some are not?
        // -> If more than 1 is named, make it a column!
        if tensor.shape().iter().filter(|d| d.name.is_some()).count() > 1 {
            for dim in tensor.shape() {
                ui.label(dim.to_string());
            }
        } else {
            ui.label(format_tensor_shape_single_line(tensor.shape()));
        }
    });
    ui.end_row();

    if let Some(TensorStats {
        range: Some((min, max)),
    }) = tensor_stats
    {
        ui.label("Data range")
            .on_hover_text("All values of the tensor range within these bounds.");
        ui.monospace(format!(
            "[{} - {}]",
            re_format::format_f64(*min),
            re_format::format_f64(*max)
        ));
        ui.end_row();
    }
}

pub fn tensor_dtype_and_shape_ui(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    tensor: &ClassicTensor,
    tensor_stats: Option<&TensorStats>,
) {
    egui::Grid::new("tensor_dtype_and_shape_ui")
        .num_columns(2)
        .show(ui, |ui| {
            tensor_dtype_and_shape_ui_grid_contents(re_ui, ui, tensor, tensor_stats);
        });
}

fn show_zoomed_image_region_tooltip(
    parent_ui: &mut egui::Ui,
    response: egui::Response,
    tensor_view: &TensorImageView<'_, '_>,
    image_rect: egui::Rect,
    pointer_pos: egui::Pos2,
    meter: Option<f32>,
) -> egui::Response {
    response
        .on_hover_cursor(egui::CursorIcon::Crosshair)
        .on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(320.0);
            ui.horizontal(|ui| {
                let Some(dynamic_img) = tensor_view.dynamic_img else { return };
                let w = dynamic_img.width() as _;
                let h = dynamic_img.height() as _;

                use egui::NumExt;

                let center = [
                    (egui::remap(pointer_pos.x, image_rect.x_range(), 0.0..=w as f32) as isize)
                        .at_most(w),
                    (egui::remap(pointer_pos.y, image_rect.y_range(), 0.0..=h as f32) as isize)
                        .at_most(h),
                ];
                show_zoomed_image_region_area_outline(parent_ui, tensor_view, center, image_rect);
                show_zoomed_image_region(ui, tensor_view, center, meter);
            });
        })
}

// Show the surrounding pixels:
const ZOOMED_IMAGE_TEXEL_RADIUS: isize = 10;

pub fn show_zoomed_image_region_area_outline(
    ui: &mut egui::Ui,
    tensor_view: &TensorImageView<'_, '_>,
    [center_x, center_y]: [isize; 2],
    image_rect: egui::Rect,
) {
    let Some(dynamic_img) = tensor_view.dynamic_img else { return };

    use egui::{pos2, remap, Color32, Rect};

    let w = dynamic_img.width() as _;
    let h = dynamic_img.height() as _;

    // Show where on the original image the zoomed-in region is at:
    let left = (center_x - ZOOMED_IMAGE_TEXEL_RADIUS) as f32;
    let right = (center_x + ZOOMED_IMAGE_TEXEL_RADIUS) as f32;
    let top = (center_y - ZOOMED_IMAGE_TEXEL_RADIUS) as f32;
    let bottom = (center_y + ZOOMED_IMAGE_TEXEL_RADIUS) as f32;

    let left = remap(left, 0.0..=w, image_rect.x_range());
    let right = remap(right, 0.0..=w, image_rect.x_range());
    let top = remap(top, 0.0..=h, image_rect.y_range());
    let bottom = remap(bottom, 0.0..=h, image_rect.y_range());

    let rect = Rect::from_min_max(pos2(left, top), pos2(right, bottom));
    // TODO(emilk): use `parent_ui.painter()` and put it in a high Z layer, when https://github.com/emilk/egui/issues/1516 is done
    let painter = ui.ctx().debug_painter();
    painter.rect_stroke(rect, 0.0, (2.0, Color32::BLACK));
    painter.rect_stroke(rect, 0.0, (1.0, Color32::WHITE));
}

/// `meter`: iff this is a depth map, how long is one meter?
pub fn show_zoomed_image_region(
    tooltip_ui: &mut egui::Ui,
    tensor_view: &TensorImageView<'_, '_>,
    image_position: [isize; 2],
    meter: Option<f32>,
) {
    let Some(dynamic_img) = tensor_view.dynamic_img else { return };

    use egui::{color_picker, pos2, remap, Color32, Mesh, Rect};

    const POINTS_PER_TEXEL: f32 = 5.0;
    let size = Vec2::splat((ZOOMED_IMAGE_TEXEL_RADIUS * 2 + 1) as f32 * POINTS_PER_TEXEL);

    let (_id, zoom_rect) = tooltip_ui.allocate_space(size);
    let painter = tooltip_ui.painter();

    painter.rect_filled(zoom_rect, 0.0, tooltip_ui.visuals().extreme_bg_color);

    let mut mesh = Mesh::default();
    let mut center_texel_rect = None;
    for dx in -ZOOMED_IMAGE_TEXEL_RADIUS..=ZOOMED_IMAGE_TEXEL_RADIUS {
        for dy in -ZOOMED_IMAGE_TEXEL_RADIUS..=ZOOMED_IMAGE_TEXEL_RADIUS {
            let x = image_position[0] + dx;
            let y = image_position[1] + dy;
            let color = get_pixel(dynamic_img, [x, y]);
            if let Some(color) = color {
                let image::Rgba([r, g, b, a]) = color;
                let color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);

                if color != Color32::TRANSPARENT {
                    let tr = ZOOMED_IMAGE_TEXEL_RADIUS as f32;
                    let left = remap(dx as f32, -tr..=(tr + 1.0), zoom_rect.x_range());
                    let right = remap((dx + 1) as f32, -tr..=(tr + 1.0), zoom_rect.x_range());
                    let top = remap(dy as f32, -tr..=(tr + 1.0), zoom_rect.y_range());
                    let bottom = remap((dy + 1) as f32, -tr..=(tr + 1.0), zoom_rect.y_range());
                    let rect = Rect {
                        min: pos2(left, top),
                        max: pos2(right, bottom),
                    };
                    mesh.add_colored_rect(rect, color);

                    if dx == 0 && dy == 0 {
                        center_texel_rect = Some(rect);
                    }
                }
            }
        }
    }

    painter.add(mesh);

    if let Some(center_texel_rect) = center_texel_rect {
        painter.rect_stroke(center_texel_rect, 0.0, (2.0, Color32::BLACK));
        painter.rect_stroke(center_texel_rect, 0.0, (1.0, Color32::WHITE));
    }

    if let Some(color) = get_pixel(dynamic_img, image_position) {
        tooltip_ui.separator();
        let (x, y) = (image_position[0] as _, image_position[1] as _);

        tooltip_ui.vertical(|ui| {
            egui::Grid::new("hovered pixel properties").show(ui, |ui| {
                ui.label("Position:");
                ui.label(format!("{}, {}", image_position[0], image_position[1]));
                ui.end_row();

                if tensor_view.tensor.num_dim() == 2 {
                    if let Some(raw_value) = tensor_view.tensor.get(&[y, x]) {
                        if let (TensorDataMeaning::ClassId, annotations, Some(u16_val)) = (
                            tensor_view.tensor.meaning(),
                            tensor_view.annotations,
                            raw_value.try_as_u16(),
                        ) {
                            ui.label("Label:");
                            ui.label(
                                annotations
                                    .class_description(Some(ClassId(u16_val)))
                                    .annotation_info()
                                    .label(None)
                                    .unwrap_or_default(),
                            );
                            ui.end_row();
                        };
                    }
                }
                if let Some(meter) = meter {
                    // This is a depth map
                    if let Some(raw_value) = tensor_view.tensor.get(&[y, x]) {
                        let raw_value = raw_value.as_f64();
                        let meters = raw_value / meter as f64;
                        ui.label("Depth:");
                        if meters < 1.0 {
                            ui.monospace(format!("{:.1} mm", meters * 1e3));
                        } else {
                            ui.monospace(format!("{meters:.3} m"));
                        }
                    }
                }
            });

            use image::DynamicImage;

            let image::Rgba([r, g, b, a]) = color;

            let text = match dynamic_img {
                DynamicImage::ImageLuma8(_) => {
                    format!("L: {r}")
                }

                DynamicImage::ImageLuma16(image) => {
                    let l = image.get_pixel(x as _, y as _)[0];
                    format!("L: {} ({:.5})", l, l as f32 / 65535.0)
                }

                DynamicImage::ImageLumaA8(_) | DynamicImage::ImageLumaA16(_) => {
                    format!("L: {r}, A: {a}")
                }

                DynamicImage::ImageRgb8(_)
                | DynamicImage::ImageRgb16(_)
                | DynamicImage::ImageRgb32F(_) => {
                    // TODO(emilk): show 16-bit and 32f values differently
                    format!("R: {r}, G: {g}, B: {b}, #{r:02X}{g:02X}{b:02X}")
                }

                DynamicImage::ImageRgba8(_)
                | DynamicImage::ImageRgba16(_)
                | DynamicImage::ImageRgba32F(_) => {
                    // TODO(emilk): show 16-bit and 32f values differently
                    format!("R: {r}, G: {g}, B: {b}, A: {a}, #{r:02X}{g:02X}{b:02X}{a:02X}")
                }

                _ => {
                    re_log::warn_once!("Unknown image color type: {:?}", dynamic_img.color());
                    format!("R: {r}, G: {g}, B: {b}, A: {a}, #{r:02X}{g:02X}{b:02X}{a:02X}")
                }
            };
            ui.label(text);

            color_picker::show_color(
                ui,
                Color32::from_rgba_unmultiplied(r, g, b, a),
                Vec2::splat(ui.available_height()),
            );
        });
    }
}

fn get_pixel(image: &image::DynamicImage, [x, y]: [isize; 2]) -> Option<image::Rgba<u8>> {
    use image::GenericImageView;

    if x < 0 || y < 0 || image.width() <= x as u32 || image.height() <= y as u32 {
        None
    } else {
        Some(image.get_pixel(x as u32, y as u32))
    }
}

fn histogram_ui(ui: &mut egui::Ui, rgb_image: &image::RgbImage) -> egui::Response {
    crate::profile_function!();

    let mut histograms = [[0_u64; 256]; 3];
    {
        // TODO(emilk): this is slow, so cache the results!
        crate::profile_scope!("build");
        for pixel in rgb_image.pixels() {
            for c in 0..3 {
                histograms[c][pixel[c] as usize] += 1;
            }
        }
    }

    use egui::plot::{Bar, BarChart, Legend, Plot};
    use egui::Color32;

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

    crate::profile_scope!("show");
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

#[cfg(not(target_arch = "wasm32"))]
fn image_options(
    ui: &mut egui::Ui,
    tensor: &re_log_types::ClassicTensor,
    dynamic_image: &image::DynamicImage,
) {
    // TODO(emilk): support copying images on web

    #[cfg(not(target_arch = "wasm32"))]
    if ui.button("Click to copy image").clicked() {
        let rgba = dynamic_image.to_rgba8();
        crate::misc::Clipboard::with(|clipboard| {
            clipboard.set_image(
                [rgba.width() as _, rgba.height() as _],
                bytemuck::cast_slice(rgba.as_raw()),
            );
        });
    }

    // TODO(emilk): support saving images on web
    #[cfg(not(target_arch = "wasm32"))]
    if ui.button("Save image…").clicked() {
        use re_log_types::TensorDataStore;

        match &tensor.data {
            TensorDataStore::Dense(_) => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("image.png")
                    .save_file()
                {
                    match dynamic_image.save(&path) {
                        // TODO(emilk): show a popup instead of logging result
                        Ok(()) => {
                            re_log::info!("Image saved to {path:?}");
                        }
                        Err(err) => {
                            re_log::error!("Failed saving image to {path:?}: {err}");
                        }
                    }
                }
            }
            TensorDataStore::Jpeg(bytes) => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("image.jpg")
                    .save_file()
                {
                    match write_binary(&path, bytes) {
                        Ok(()) => {
                            re_log::info!("Image saved to {path:?}");
                        }
                        Err(err) => {
                            re_log::error!(
                                "Failed saving image to {path:?}: {}",
                                re_error::format(&err)
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn write_binary(path: &std::path::PathBuf, data: &[u8]) -> anyhow::Result<()> {
    use std::io::Write as _;
    Ok(std::fs::File::create(path)?.write_all(data)?)
}
