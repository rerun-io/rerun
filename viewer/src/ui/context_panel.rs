use crate::misc::image_cache::to_rgba_unultiplied;

use log_types::{Data, LogMsg, ObjectPath};

use crate::{LogDb, Preview, Selection, ViewerContext};

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct ContextPanel {}

impl ContextPanel {
    pub fn ui(&mut self, log_db: &LogDb, context: &mut ViewerContext, ui: &mut egui::Ui) {
        crate::profile_function!();

        ui.heading("Selection");
        ui.separator();

        match &context.selection {
            Selection::None => {
                ui.weak("(nothing)");
            }
            Selection::LogId(log_id) => {
                // ui.label(format!("Selected log_id: {:?}", log_id));
                ui.label("Selected a specific log message");

                let msg = if let Some(msg) = log_db.get_msg(log_id) {
                    msg
                } else {
                    tracing::warn!("Unknown log_id selected. Resetting selection");
                    context.selection = Selection::None;
                    return;
                };

                self.view_log_msg(log_db, context, ui, msg);
            }
            Selection::Space(space) => {
                let space = space.clone();
                ui.label(format!("Selected space: {}", space));
                ui.small("Showing latest versions of each object.")
                    .on_hover_text("Latest by the current time, that is");
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    let mut messages = context.time_control.selected_messages(log_db);
                    messages.retain(|msg| msg.space.as_ref() == Some(&space));
                    crate::log_table_view::message_table(log_db, context, ui, &messages);
                });
            }
        }
    }

    #[allow(clippy::unused_self)]
    fn view_log_msg(
        &mut self,
        log_db: &LogDb,
        context: &mut ViewerContext,
        ui: &mut egui::Ui,
        msg: &LogMsg,
    ) {
        show_detailed_log_msg(context, ui, msg);

        ui.separator();

        let messages = context.time_control.selected_messages(log_db);

        let mut parent_path = msg.object_path.0.clone();
        parent_path.pop();

        let sibling_messages: Vec<&LogMsg> = messages
            .iter()
            .copied()
            .filter(|other_msg| other_msg.object_path.0.starts_with(&parent_path))
            .collect();

        ui.label(format!("{}:", ObjectPath(parent_path.clone())));

        if true {
            ui.indent("siblings", |ui| {
                egui::Grid::new("siblings").striped(true).show(ui, |ui| {
                    for msg in sibling_messages {
                        let child_path =
                            ObjectPath(msg.object_path.0[parent_path.len()..].to_vec());
                        ui.label(child_path.to_string());
                        crate::space_view::ui_data(context, ui, &msg.id, &msg.data, Preview::Small);
                        ui.end_row();
                    }
                });
            });
        } else {
            crate::log_table_view::message_table(log_db, context, ui, &sibling_messages);
        }
    }
}

pub(crate) fn show_detailed_log_msg(context: &mut ViewerContext, ui: &mut egui::Ui, msg: &LogMsg) {
    let LogMsg {
        id,
        time_point,
        object_path,
        space,
        data,
    } = msg;

    let is_image = matches!(msg.data, Data::Image(_));

    egui::Grid::new("fields")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            ui.monospace("object_path:");
            ui.label(format!("{object_path}"));
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
        let egui_image = context.image_cache.get(id, image);
        let max_size = ui.available_size().min(egui_image.size_vec2());
        egui_image.show_max_size(ui, max_size);

        // TODO: support copying and saving images on web
        #[cfg(not(target_arch = "wasm32"))]
        ui.horizontal(|ui| image_options(ui, image));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn image_options(ui: &mut egui::Ui, image: &log_types::Image) {
    // TODO: support copying images on web
    #[cfg(not(target_arch = "wasm32"))]
    if ui.button("Click to copy image").clicked() {
        crate::Clipboard::with(|clipboard| match to_rgba_unultiplied(image) {
            Ok(([w, h], rgba)) => clipboard.set_image([w as _, h as _], &rgba),
            Err(err) => {
                tracing::error!("Failed to copy image: {}", err);
            }
        });
    }

    // TODO: support saving images on web

    #[cfg(not(target_arch = "wasm32"))]
    if ui.button("Save image…").clicked() {
        match image.format {
            log_types::ImageFormat::Jpeg => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("image.jpg")
                    .save_file()
                {
                    match write_binary(&path, &image.data) {
                        Ok(()) => {
                            tracing::info!("Image saved to {:?}", path);
                        }
                        Err(err) => {
                            tracing::error!("Failed saving image to {:?}: {}", path, err);
                        }
                    }
                }
            }
            _ => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("image.png")
                    .save_file()
                {
                    if let Some(image) = to_image_image(image) {
                        match image.save(&path) {
                            // TODO: show a popup instead of logging result
                            Ok(()) => {
                                tracing::info!("Image saved to {:?}", path);
                            }
                            Err(err) => {
                                tracing::error!("Failed saving image to {:?}: {}", path, err);
                            }
                        }
                    } else {
                        tracing::warn!("Failed to create image. Very weird");
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

#[cfg(not(target_arch = "wasm32"))]
fn to_image_image(image: &log_types::Image) -> Option<image::DynamicImage> {
    let [w, h] = image.size;
    match image.format {
        log_types::ImageFormat::Luminance8 => image::GrayImage::from_raw(w, h, image.data.clone())
            .map(image::DynamicImage::ImageLuma8),
        log_types::ImageFormat::Rgba8 => image::RgbaImage::from_raw(w, h, image.data.clone())
            .map(image::DynamicImage::ImageRgba8),
        log_types::ImageFormat::Jpeg => {
            let ([w, h], rgba) = to_rgba_unultiplied(image).ok()?;
            Some(image::DynamicImage::ImageRgba8(image::RgbaImage::from_raw(
                w, h, rgba,
            )?))
        }
    }
}
