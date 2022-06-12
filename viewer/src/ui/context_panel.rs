use itertools::Itertools;
use log_types::{Data, DataMsg};

use crate::{LogDb, Preview, Selection, ViewerContext};

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct ContextPanel {}

impl ContextPanel {
    pub fn ui(&mut self, log_db: &LogDb, context: &mut ViewerContext, ui: &mut egui::Ui) {
        crate::profile_function!();

        ui.horizontal(|ui| {
            ui.heading("Selection");

            if context.selection.is_some() && ui.small_button("Deselect").clicked() {
                context.selection = Selection::None;
            }
        });

        ui.separator();

        match &context.selection.clone() {
            Selection::None => {
                ui.weak("(nothing)");
            }
            Selection::LogId(log_id) => {
                // ui.label(format!("Selected log_id: {:?}", log_id));
                ui.label("Selected a specific log message");

                let msg = if let Some(msg) = log_db.get_data_msg(log_id) {
                    msg
                } else {
                    tracing::warn!("Unknown log_id selected. Resetting selection");
                    context.selection = Selection::None;
                    return;
                };

                show_detailed_data_msg(context, ui, msg);
                ui.separator();
                self.view_log_msg_siblings(log_db, context, ui, msg);
            }
            Selection::ObjTypePath(obj_type_path) => {
                ui.label(format!("Selected object type path: {}", obj_type_path));
            }
            Selection::ObjPath(obj_path) => {
                ui.label(format!("Selected object: {}", obj_path));
                ui.horizontal(|ui| {
                    ui.label("Type path:");
                    context.type_path_button(ui, obj_path.obj_type_path());
                });
                // TODO: show object contents
            }
            Selection::DataPath(data_path) => {
                ui.label(format!("Selected data path: {}", data_path));
                ui.horizontal(|ui| {
                    ui.label("Object path:");
                    context.obj_path_button(ui, &data_path.obj_path);
                });
                ui.horizontal(|ui| {
                    ui.label("Type path:");
                    context.type_path_button(ui, data_path.obj_path.obj_type_path());
                });

                ui.separator();

                let mut messages = context
                    .time_control
                    .selected_messages_for_data(log_db, data_path);
                messages.sort_by_key(|msg| &msg.time_point);

                if context.time_control.is_time_filter_active() {
                    ui.label(format!("Viewing {} selected message(s):", messages.len()));
                } else {
                    ui.label("Viewing latest message:");
                }

                if messages.is_empty() {
                    // nothing to see here
                } else if messages.len() == 1 {
                    // probably viewing the latest message of this data path
                    show_detailed_data_msg(context, ui, messages[0]);
                } else {
                    crate::log_table_view::message_table(log_db, context, ui, &messages);
                }
            }
            Selection::Space(space) => {
                let space = space.clone();
                ui.label(format!("Selected space: {}", space));
                ui.small("Showing latest versions of each object.")
                    .on_hover_text("Latest by the current time, that is");
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    let mut messages = context.time_control.selected_messages(log_db);
                    messages.retain(|msg| msg.space.as_ref() == Some(&space));

                    messages.sort_by_key(|msg| &msg.time_point);
                    crate::log_table_view::message_table(log_db, context, ui, &messages);
                });
            }
        }
    }

    #[allow(clippy::unused_self)]
    fn view_log_msg_siblings(
        &mut self,
        log_db: &LogDb,
        context: &mut ViewerContext,
        ui: &mut egui::Ui,
        msg: &DataMsg,
    ) {
        crate::profile_function!();
        let messages = context.time_control.selected_messages(log_db);

        let obj_path = msg.data_path.obj_path.clone();

        let mut sibling_messages: Vec<&DataMsg> = messages
            .iter()
            .copied()
            .filter(|other_msg| other_msg.data_path.obj_path == obj_path)
            .collect();

        sibling_messages.sort_by_key(|msg| &msg.time_point);

        ui.label(format!("{}:", obj_path));

        use egui_extras::Size;
        egui_extras::TableBuilder::new(ui)
            .striped(true)
            .cell_layout(egui::Layout::left_to_right().with_cross_align(egui::Align::Center))
            .resizable(true)
            .column(Size::initial(120.0).at_least(100.0)) // relative path
            .column(Size::remainder().at_least(180.0)) // data
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.heading("Relative path");
                });
                header.col(|ui| {
                    ui.heading("Data");
                });
            })
            .body(|body| {
                const ROW_HEIGHT: f32 = 24.0;
                body.rows(ROW_HEIGHT, sibling_messages.len(), |index, mut row| {
                    let msg = sibling_messages[index];

                    row.col(|ui| {
                        context.data_path_button_to(
                            ui,
                            msg.data_path.field_name.as_str(),
                            &msg.data_path,
                        );
                    });
                    row.col(|ui| {
                        crate::space_view::ui_data(
                            context,
                            ui,
                            &msg.id,
                            &msg.data,
                            Preview::Specific(ROW_HEIGHT),
                        );
                    });
                });
            });
    }
}

pub(crate) fn show_detailed_data_msg(
    context: &mut ViewerContext,
    ui: &mut egui::Ui,
    msg: &DataMsg,
) {
    let DataMsg {
        id,
        time_point,
        data_path,
        space,
        data,
    } = msg;

    let is_image = matches!(msg.data, Data::Image(_));

    egui::Grid::new("fields")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            ui.monospace("data_path:");
            context.data_path_button(ui, data_path);
            ui.end_row();
            ui.monospace("object type path:");
            context.type_path_button(ui, data_path.obj_path.obj_type_path());
            ui.end_row();

            ui.monospace("time_point:");
            crate::space_view::ui_time_point(context, ui, time_point);
            ui.end_row();

            ui.monospace("space:");
            if let Some(space) = space {
                context.space_button(ui, space);
            }
            ui.end_row();

            if !is_image {
                ui.monospace("data:");
                crate::space_view::ui_data(context, ui, id, data, Preview::Medium);
                ui.end_row();
            }
        });

    if let Data::Image(image) = &msg.data {
        show_image(context, ui, msg, image);
    }
}

fn show_image(
    context: &mut ViewerContext,
    ui: &mut egui::Ui,
    msg: &DataMsg,
    image: &log_types::Image,
) {
    let (dynamic_image, egui_image) = context.image_cache.get_pair(&msg.id, image);
    let max_size = ui.available_size().min(egui_image.size_vec2());
    let response = egui_image.show_max_size(ui, max_size);

    let image_rect = response.rect;

    response
        .on_hover_cursor(egui::CursorIcon::ZoomIn)
        .on_hover_ui_at_pointer(|ui| {
            if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
                ui.horizontal(|ui| {
                    show_zoomed_image_region(ui, dynamic_image, image_rect, pointer_pos);
                });
            }
        });

    // TODO: support copying and saving images on web
    #[cfg(not(target_arch = "wasm32"))]
    ui.horizontal(|ui| image_options(ui, image, dynamic_image));

    // TODO: support histograms of non-RGB images too
    if let image::DynamicImage::ImageRgb8(rgb_image) = dynamic_image {
        ui.collapsing("Histogram", |ui| {
            histogram_ui(ui, rgb_image);
        });
    }
}

fn show_zoomed_image_region(
    ui: &mut egui::Ui,
    dynamic_image: &image::DynamicImage,
    image_rect: egui::Rect,
    pointer_pos: egui::Pos2,
) {
    use egui::*;
    use image::GenericImageView as _;

    let (_id, zoom_rect) = ui.allocate_space(vec2(192.0, 192.0));
    let w = dynamic_image.width() as _;
    let h = dynamic_image.height() as _;
    let center_x =
        (remap(pointer_pos.x, image_rect.x_range(), 0.0..=(w as f32)).floor() as isize).at_most(w);
    let center_y =
        (remap(pointer_pos.y, image_rect.y_range(), 0.0..=(h as f32)).floor() as isize).at_most(h);

    ui.painter()
        .rect_filled(zoom_rect, 0.0, ui.visuals().extreme_bg_color);

    // Show all the surrounding pixels:
    let texel_radius = 12;

    let mut mesh = Mesh::default();
    let mut center_texel_rect = None;
    for dx in -texel_radius..=texel_radius {
        for dy in -texel_radius..=texel_radius {
            let x = center_x + dx;
            let y = center_y + dy;
            let color = get_pixel(dynamic_image, [x, y]);
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

    ui.painter().add(mesh);

    if let Some(center_texel_rect) = center_texel_rect {
        ui.painter()
            .rect_stroke(center_texel_rect, 0.0, (2.0, Color32::BLACK));
        ui.painter()
            .rect_stroke(center_texel_rect, 0.0, (1.0, Color32::WHITE));
    }

    if let Some(color) = get_pixel(dynamic_image, [center_x, center_y]) {
        ui.separator();
        let (x, y) = (center_x as _, center_y as _);

        ui.vertical(|ui| {
            let image::Rgba([r, g, b, a]) = color;
            color_picker::show_color(
                ui,
                Color32::from_rgba_unmultiplied(r, g, b, a),
                Vec2::splat(64.0),
            );

            use image::DynamicImage;

            let text = match dynamic_image {
                DynamicImage::ImageLuma8(_) => {
                    format!("L: {}", r)
                }

                DynamicImage::ImageLuma16(image) => {
                    let l = image.get_pixel(x, y)[0];
                    format!("L: {} ({:.5})", l, l as f32 / 65535.0)
                }

                DynamicImage::ImageLumaA8(_) | DynamicImage::ImageLumaA16(_) => {
                    format!("L: {}\nA: {}", r, a)
                }

                DynamicImage::ImageRgb8(_)
                | DynamicImage::ImageBgr8(_)
                | DynamicImage::ImageRgb16(_) => {
                    format!(
                        "R: {}\nG: {}\nB: {}\n\n#{:02X}{:02X}{:02X}",
                        r, g, b, r, g, b
                    )
                }

                DynamicImage::ImageRgba8(_)
                | DynamicImage::ImageBgra8(_)
                | DynamicImage::ImageRgba16(_) => {
                    format!(
                        "R: {}\nG: {}\nB: {}\nA: {}\n\n#{:02X}{:02X}{:02X}{:02X}",
                        r, g, b, a, r, g, b, a
                    )
                }
            };

            ui.label(text);
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
        // TODO: this is slow, so cache the results!
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
                            .stroke(egui::Stroke::none())
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
    rr_image: &log_types::Image,
    dynamic_image: &image::DynamicImage,
) {
    // TODO: support copying images on web
    #[cfg(not(target_arch = "wasm32"))]
    if ui.button("Click to copy image").clicked() {
        let rgba = dynamic_image.to_rgba8();
        crate::Clipboard::with(|clipboard| {
            clipboard.set_image(
                [rgba.width() as _, rgba.height() as _],
                bytemuck::cast_slice(rgba.as_raw()),
            );
        });
    }

    // TODO: support saving images on web
    #[cfg(not(target_arch = "wasm32"))]
    if ui.button("Save image…").clicked() {
        use log_types::ImageFormat;
        match rr_image.format {
            ImageFormat::Jpeg => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("image.jpg")
                    .save_file()
                {
                    match write_binary(&path, &rr_image.data) {
                        Ok(()) => {
                            tracing::info!("Image saved to {:?}", path);
                        }
                        Err(err) => {
                            tracing::error!("Failed saving image to {:?}: {}", path, err);
                        }
                    }
                }
            }
            ImageFormat::Luminance8
            | ImageFormat::Luminance16
            | ImageFormat::Rgb8
            | ImageFormat::Rgba8 => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("image.png")
                    .save_file()
                {
                    match dynamic_image.save(&path) {
                        // TODO: show a popup instead of logging result
                        Ok(()) => {
                            tracing::info!("Image saved to {:?}", path);
                        }
                        Err(err) => {
                            tracing::error!("Failed saving image to {:?}: {}", path, err);
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
