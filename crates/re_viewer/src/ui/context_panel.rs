use itertools::Itertools as _;

use re_data_store::{ObjPath, ObjTypePath};
use re_log_types::{Data, DataMsg, DataPath, LogMsg, LoggedData, MsgId};

use crate::{LogDb, Preview, Selection, ViewerContext};

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct ContextPanel {}

impl ContextPanel {
    #[allow(clippy::unused_self)]
    pub fn ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        crate::profile_function!();

        ui.horizontal(|ui| {
            ui.heading("Selection");

            if ctx.rec_cfg.selection.is_some() && ui.small_button("Deselect").clicked() {
                ctx.rec_cfg.selection = Selection::None;
            }
        });

        ui.separator();

        match &ctx.rec_cfg.selection.clone() {
            Selection::None => {
                ui.weak("(nothing)");
            }
            Selection::MsgId(msg_id) => {
                // ui.label(format!("Selected msg_id: {:?}", msg_id));
                ui.label("Selected a specific log message");

                let msg = if let Some(msg) = ctx.log_db.get_log_msg(msg_id) {
                    msg
                } else {
                    tracing::warn!("Unknown msg_id selected. Resetting selection");
                    ctx.rec_cfg.selection = Selection::None;
                    return;
                };

                match msg {
                    LogMsg::BeginRecordingMsg(msg) => {
                        crate::ui::space_view::show_begin_recording_msg(ui, msg);
                    }
                    LogMsg::TypeMsg(msg) => {
                        crate::ui::space_view::show_type_msg(ctx, ui, msg);
                    }
                    LogMsg::DataMsg(msg) => {
                        show_detailed_data_msg(ctx, ui, msg);
                        ui.separator();
                        view_object(ctx, ui, &msg.data_path.obj_path, Preview::Medium);
                    }
                }
            }
            Selection::ObjTypePath(obj_type_path) => {
                ui.label(format!("Selected object type path: {}", obj_type_path));
            }
            Selection::ObjPath(obj_path) => {
                ui.label(format!("Selected object: {}", obj_path));
                ui.horizontal(|ui| {
                    ui.label("Type path:");
                    ctx.type_path_button(ui, obj_path.obj_type_path());
                });
                ui.horizontal(|ui| {
                    ui.label("Object type:");
                    ui.label(obj_type_name(ctx.log_db, obj_path.obj_type_path()));
                });
                ui.separator();
                view_object(ctx, ui, obj_path, Preview::Medium);
            }
            Selection::DataPath(data_path) => {
                ui.label(format!("Selected data path: {}", data_path));
                ui.horizontal(|ui| {
                    ui.label("Object path:");
                    ctx.obj_path_button(ui, &data_path.obj_path);
                });
                ui.horizontal(|ui| {
                    ui.label("Type path:");
                    ctx.type_path_button(ui, data_path.obj_path.obj_type_path());
                });
                ui.horizontal(|ui| {
                    ui.label("Object type:");
                    ui.label(obj_type_name(
                        ctx.log_db,
                        data_path.obj_path.obj_type_path(),
                    ));
                });

                ui.separator();

                view_data(ctx, ui, data_path);
            }
            Selection::Space(space) => {
                let space = space.clone();
                ui.label(format!("Selected space: {}", space));
                // I really don't know what we should show here.
            }
        }
    }
}

pub(crate) fn view_object(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    obj_path: &ObjPath,
    preview: Preview,
) -> Option<()> {
    let (_, store) = ctx.log_db.data_store.get(ctx.rec_cfg.time_ctrl.source())?;
    let time_query = ctx.rec_cfg.time_ctrl.time_query()?;
    let obj_store = store.get(obj_path.obj_type_path())?;

    egui::Grid::new("object")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            for (field_name, data_store) in obj_store.iter() {
                ctx.data_path_button_to(
                    ui,
                    field_name.to_string(),
                    &DataPath::new(obj_path.clone(), *field_name),
                );

                let (_times, msg_ids, data_vec) =
                    data_store.query_object(obj_path.index_path().clone(), &time_query);

                if data_vec.len() == 1 {
                    let data = data_vec.last().unwrap();
                    let msg_id = &msg_ids[0];
                    crate::space_view::ui_data(ctx, ui, msg_id, &data, preview);
                } else {
                    ui.label(format!("{} x {:?}", data_vec.len(), data_vec.data_type()));
                }

                ui.end_row();
            }
        });

    Some(())
}

fn view_data(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, data_path: &DataPath) -> Option<()> {
    let obj_path = data_path.obj_path();
    let field_name = data_path.field_name();

    let (_, store) = ctx.log_db.data_store.get(ctx.rec_cfg.time_ctrl.source())?;
    let time_query = ctx.rec_cfg.time_ctrl.time_query()?;
    let obj_store = store.get(obj_path.obj_type_path())?;
    let data_store = obj_store.get_field(field_name)?;

    let (_times, msg_ids, data_vec) =
        data_store.query_object(obj_path.index_path().clone(), &time_query);

    if data_vec.len() == 1 {
        let data = data_vec.last().unwrap();
        let msg_id = &msg_ids[0];
        show_detailed_data(ctx, ui, msg_id, &data);
    } else {
        ui.label(format!("{} x {:?}", data_vec.len(), data_vec.data_type()));
    }

    Some(())
}

pub(crate) fn show_detailed_data(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    msg_id: &MsgId,
    data: &Data,
) {
    if let Data::Tensor(tensor) = data {
        show_tensor(ctx, ui, msg_id, tensor);
    } else {
        crate::space_view::ui_data(ctx, ui, msg_id, data, Preview::Medium);
    }
}

pub(crate) fn show_detailed_data_msg(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    msg: &DataMsg,
) {
    let DataMsg {
        msg_id,
        time_point,
        data_path,
        data,
    } = msg;

    let is_image = matches!(msg.data, LoggedData::Single(Data::Tensor(_)));

    egui::Grid::new("fields")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            ui.monospace("data_path:");
            ctx.data_path_button(ui, data_path);
            ui.end_row();
            ui.monospace("object type path:");
            ctx.type_path_button(ui, data_path.obj_path.obj_type_path());
            ui.end_row();

            ui.monospace("time_point:");
            crate::space_view::ui_time_point(ctx, ui, time_point);
            ui.end_row();

            if !is_image {
                ui.monospace("data:");
                crate::space_view::ui_logged_data(ctx, ui, msg_id, data, Preview::Medium);
                ui.end_row();
            }
        });

    if let LoggedData::Single(Data::Tensor(tensor)) = &msg.data {
        show_tensor(ctx, ui, msg_id, tensor);
    }
}

fn show_tensor(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    msg_id: &MsgId,
    tensor: &re_log_types::Tensor,
) {
    let (dynamic_image, egui_image) = ctx.cache.image.get_pair(msg_id, tensor);
    let max_size = ui.available_size().min(egui_image.size_vec2());
    let response = egui_image.show_max_size(ui, max_size);

    let image_rect = response.rect;

    if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
        show_zoomed_image_region_tooltip(
            ui,
            response,
            tensor,
            dynamic_image,
            image_rect,
            pointer_pos,
            None,
        );
    }

    // TODO(emilk): support copying and saving images on web
    #[cfg(not(target_arch = "wasm32"))]
    ui.horizontal(|ui| image_options(ui, tensor, dynamic_image));

    // TODO(emilk): support histograms of non-RGB images too
    if let image::DynamicImage::ImageRgb8(rgb_image) = dynamic_image {
        ui.collapsing("Histogram", |ui| {
            histogram_ui(ui, rgb_image);
        });
    }
}

pub fn show_zoomed_image_region_tooltip(
    parent_ui: &mut egui::Ui,
    response: egui::Response,
    tensor: &re_log_types::Tensor,
    dynamic_image: &image::DynamicImage,
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
                    tensor,
                    dynamic_image,
                    image_rect,
                    pointer_pos,
                    meter,
                );
            });
        })
}

/// meter: iff this is a depth map, how long is one meter?
fn show_zoomed_image_region(
    parent_ui: &mut egui::Ui,
    tooltip_ui: &mut egui::Ui,
    tensor: &re_log_types::Tensor,
    dynamic_image: &image::DynamicImage,
    image_rect: egui::Rect,
    pointer_pos: egui::Pos2,
    meter: Option<f32>,
) {
    use egui::*;
    use image::GenericImageView as _;

    let (_id, zoom_rect) = tooltip_ui.allocate_space(vec2(192.0, 192.0));
    let w = dynamic_image.width() as _;
    let h = dynamic_image.height() as _;
    let center_x =
        (remap(pointer_pos.x, image_rect.x_range(), 0.0..=(w as f32)).floor() as isize).at_most(w);
    let center_y =
        (remap(pointer_pos.y, image_rect.y_range(), 0.0..=(h as f32)).floor() as isize).at_most(h);

    // Show the surrounding pixels:
    let texel_radius = 12;

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

    painter.add(mesh);

    if let Some(center_texel_rect) = center_texel_rect {
        painter.rect_stroke(center_texel_rect, 0.0, (2.0, Color32::BLACK));
        painter.rect_stroke(center_texel_rect, 0.0, (1.0, Color32::WHITE));
    }

    if let Some(color) = get_pixel(dynamic_image, [center_x, center_y]) {
        tooltip_ui.separator();
        let (x, y) = (center_x as _, center_y as _);

        tooltip_ui.vertical(|ui| {
            if tensor.num_dim() == 2 {
                if let Some(raw_value) = tensor.get(&[y, x]) {
                    ui.monospace(format!("Raw value: {}", raw_value.as_f64()));
                }
            } else if tensor.num_dim() == 3 {
                let mut s = "Raw values:".to_owned();
                for c in 0..tensor.shape[2] {
                    if let Some(raw_value) = tensor.get(&[y, x, c]) {
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
                if let Some(raw_value) = tensor.get(&[y, x]) {
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

                let text = match dynamic_image {
                    DynamicImage::ImageLuma8(_) => {
                        format!("L: {}", r)
                    }

                    DynamicImage::ImageLuma16(image) => {
                        let l = image.get_pixel(x as _, y as _)[0];
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
    tensor: &re_log_types::Tensor,
    dynamic_image: &image::DynamicImage,
) {
    // TODO(emilk): support copying images on web

    use re_log_types::TensorDataStore;
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

    // TODO(emilk): support saving images on web
    #[cfg(not(target_arch = "wasm32"))]
    if ui.button("Save image…").clicked() {
        match &tensor.data {
            TensorDataStore::Dense(_) => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("image.png")
                    .save_file()
                {
                    match dynamic_image.save(&path) {
                        // TODO(emilk): show a popup instead of logging result
                        Ok(()) => {
                            tracing::info!("Image saved to {path:?}");
                        }
                        Err(err) => {
                            tracing::error!("Failed saving image to {path:?}: {err}");
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
                            tracing::info!("Image saved to {path:?}");
                        }
                        Err(err) => {
                            tracing::error!(
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

fn obj_type_name(log_db: &LogDb, obj_type_path: &ObjTypePath) -> String {
    if let Some(typ) = log_db.object_types.get(obj_type_path) {
        format!("{typ:?}")
    } else {
        "<UNKNOWN>".to_owned()
    }
}
