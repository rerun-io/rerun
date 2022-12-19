use itertools::Itertools as _;

use re_log_types::{context::ClassId, TensorDataMeaning};

use crate::misc::{caches::TensorImageView, ViewerContext};

pub(crate) fn tensor_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    tensor: &re_log_types::Tensor,
) {
    let tensor_view = ctx.cache.image.get_view(tensor, ctx.render_ctx);

    let max_size = ui
        .available_size()
        .min(tensor_view.retained_img.size_vec2());
    let response = tensor_view.retained_img.show_max_size(ui, max_size);

    let image_rect = response.rect;

    if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
        show_zoomed_image_region_tooltip(ui, response, &tensor_view, image_rect, pointer_pos, None);
    }

    // TODO(emilk): support copying and saving images on web
    #[cfg(not(target_arch = "wasm32"))]
    ui.horizontal(|ui| image_options(ui, tensor, tensor_view.dynamic_img));

    // TODO(emilk): support histograms of non-RGB images too
    if let image::DynamicImage::ImageRgb8(rgb_image) = tensor_view.dynamic_img {
        ui.collapsing("Histogram", |ui| {
            histogram_ui(ui, rgb_image);
        });
    }
}

pub fn show_zoomed_image_region_tooltip(
    parent_ui: &mut egui::Ui,
    response: egui::Response,
    tensor_view: &TensorImageView<'_, '_>,
    image_rect: egui::Rect,
    pointer_pos: egui::Pos2,
    meter: Option<f32>,
) -> egui::Response {
    response
        .on_hover_cursor(egui::CursorIcon::ZoomIn)
        .on_hover_ui_at_pointer(|ui| {
            ui.horizontal(|ui| {
                show_zoomed_image_region(
                    parent_ui,
                    ui,
                    tensor_view,
                    image_rect,
                    pointer_pos,
                    meter,
                );
            });
        })
}

/// meter: iff this is a depth map, how long is one meter?
pub fn show_zoomed_image_region(
    parent_ui: &mut egui::Ui,
    tooltip_ui: &mut egui::Ui,
    tensor_view: &TensorImageView<'_, '_>,
    image_rect: egui::Rect,
    pointer_pos: egui::Pos2,
    meter: Option<f32>,
) {
    use egui::{color_picker, pos2, remap, Color32, Mesh, NumExt, Rect, Vec2};

    // Show the surrounding pixels:
    let texel_radius = 12;
    let size = Vec2::splat(128.0);

    let (_id, zoom_rect) = tooltip_ui.allocate_space(size);
    let w = tensor_view.dynamic_img.width() as _;
    let h = tensor_view.dynamic_img.height() as _;
    let center_x =
        (remap(pointer_pos.x, image_rect.x_range(), 0.0..=(w as f32)).floor() as isize).at_most(w);
    let center_y =
        (remap(pointer_pos.y, image_rect.y_range(), 0.0..=(h as f32)).floor() as isize).at_most(h);

    {
        // Show where on the original image the zoomed-in region is at:
        let left = (center_x - texel_radius) as f32;
        let right = (center_x + texel_radius) as f32;
        let top = (center_y - texel_radius) as f32;
        let bottom = (center_y + texel_radius) as f32;

        let left = remap(left, 0.0..=w as f32, image_rect.x_range());
        let right = remap(right, 0.0..=w as f32, image_rect.x_range());
        let top = remap(top, 0.0..=h as f32, image_rect.y_range());
        let bottom = remap(bottom, 0.0..=h as f32, image_rect.y_range());

        let rect = Rect::from_min_max(pos2(left, top), pos2(right, bottom));
        // TODO(emilk): use `parent_ui.painter()` and put it in a high Z layer, when https://github.com/emilk/egui/issues/1516 is done
        let painter = parent_ui.ctx().debug_painter();
        painter.rect_stroke(rect, 0.0, (2.0, Color32::BLACK));
        painter.rect_stroke(rect, 0.0, (1.0, Color32::WHITE));
    }

    let painter = tooltip_ui.painter();

    painter.rect_filled(zoom_rect, 0.0, tooltip_ui.visuals().extreme_bg_color);

    let mut mesh = Mesh::default();
    let mut center_texel_rect = None;
    for dx in -texel_radius..=texel_radius {
        for dy in -texel_radius..=texel_radius {
            let x = center_x + dx;
            let y = center_y + dy;
            let color = get_pixel(tensor_view.dynamic_img, [x, y]);
            if let Some(color) = color {
                let image::Rgba([r, g, b, a]) = color;
                let color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);

                if color != Color32::TRANSPARENT {
                    let tr = texel_radius as f32;
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

    if let Some(color) = get_pixel(tensor_view.dynamic_img, [center_x, center_y]) {
        tooltip_ui.separator();
        let (x, y) = (center_x as _, center_y as _);

        tooltip_ui.vertical(|ui| {
            if tensor_view.tensor.num_dim() == 2 {
                if let Some(raw_value) = tensor_view.tensor.get(&[y, x]) {
                    ui.monospace(format!("Raw value: {}", raw_value.as_f64()));

                    if let (TensorDataMeaning::ClassId, Some(annotations), Some(u16_val)) = (
                        tensor_view.tensor.meaning,
                        tensor_view.annotations,
                        raw_value.try_as_u16(),
                    ) {
                        ui.monospace(format!(
                            "Label: {}",
                            annotations
                                .class_description(Some(ClassId(u16_val)))
                                .annotation_info()
                                .label(None)
                                .unwrap_or_default()
                        ));
                    };
                }
            } else if tensor_view.tensor.num_dim() == 3 {
                let mut s = "Raw values:".to_owned();
                for c in 0..tensor_view.tensor.shape[2].size {
                    if let Some(raw_value) = tensor_view.tensor.get(&[y, x, c]) {
                        use std::fmt::Write as _;
                        write!(&mut s, " {}", raw_value.as_f64()).unwrap();
                    }
                }
                ui.monospace(s);
            }

            let image::Rgba([r, g, b, a]) = color;

            if meter.is_none() {
                color_picker::show_color(
                    ui,
                    Color32::from_rgba_unmultiplied(r, g, b, a),
                    Vec2::splat(64.0),
                );
            }

            if let Some(meter) = meter {
                // This is a depth map
                if let Some(raw_value) = tensor_view.tensor.get(&[y, x]) {
                    let raw_value = raw_value.as_f64();
                    let meters = raw_value / meter as f64;
                    if meters < 1.0 {
                        ui.monospace(format!("Depth: {:.1} mm", meters * 1e3));
                    } else {
                        ui.monospace(format!("Depth: {meters:.3} m"));
                    }
                }
            } else {
                use image::DynamicImage;

                let text = match tensor_view.dynamic_img {
                    DynamicImage::ImageLuma8(_) => {
                        format!("L: {}", r)
                    }

                    DynamicImage::ImageLuma16(image) => {
                        let l = image.get_pixel(x as _, y as _)[0];
                        format!("L: {} ({:.5})", l, l as f32 / 65535.0)
                    }

                    DynamicImage::ImageLumaA8(_) | DynamicImage::ImageLumaA16(_) => {
                        format!("L: {}, A: {}", r, a)
                    }

                    DynamicImage::ImageRgb8(_)
                    | DynamicImage::ImageRgb16(_)
                    | DynamicImage::ImageRgb32F(_) => {
                        // TODO(emilk): show 16-bit and 32f values differently
                        format!("R: {}, G: {}, B: {}\n#{:02X}{:02X}{:02X}", r, g, b, r, g, b)
                    }

                    DynamicImage::ImageRgba8(_)
                    | DynamicImage::ImageRgba16(_)
                    | DynamicImage::ImageRgba32F(_) => {
                        // TODO(emilk): show 16-bit and 32f values differently
                        format!(
                            "R: {}, G: {}, B: {}, A: {}\n#{:02X}{:02X}{:02X}{:02X}",
                            r, g, b, a, r, g, b, a
                        )
                    }

                    _ => {
                        re_log::warn_once!(
                            "Unknown image color type: {:?}",
                            tensor_view.dynamic_img.color()
                        );
                        format!(
                            "R: {}, G: {}, B: {}, A: {}\n#{:02X}{:02X}{:02X}{:02X}",
                            r, g, b, a, r, g, b, a
                        )
                    }
                };
                ui.label(text);
            }
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
    tensor: &re_log_types::Tensor,
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
