use itertools::Itertools;
use log_types::{Data, LogMsg, ObjectPath};

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

                show_detailed_log_msg(context, ui, msg);
                ui.separator();
                self.view_log_msg_siblings(log_db, context, ui, msg);
            }
            Selection::ObjectPath(object_path) => {
                ui.label(format!("Selected object: {}", object_path));

                ui.separator();

                let mut messages = context
                    .time_control
                    .selected_messages_for_object(log_db, object_path);
                messages.sort_by_key(|msg| &msg.time_point);

                if context.time_control.is_time_filter_active() {
                    ui.label(format!("Viewing {} selected message(s):", messages.len()));
                } else {
                    ui.label("Viewing latest message:");
                }

                if messages.is_empty() {
                    // nothing to see here
                } else if messages.len() == 1 {
                    // probably viewing the latest message of this object path
                    show_detailed_log_msg(context, ui, messages[0]);
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
        msg: &LogMsg,
    ) {
        crate::profile_function!();
        let messages = context.time_control.selected_messages(log_db);

        let mut parent_path = msg.object_path.0.clone();
        parent_path.pop();

        let mut sibling_messages: Vec<&LogMsg> = messages
            .iter()
            .copied()
            .filter(|other_msg| other_msg.object_path.0.starts_with(&parent_path))
            .collect();

        sibling_messages.sort_by_key(|msg| &msg.time_point);

        ui.label(format!("{}:", ObjectPath(parent_path.clone())));

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
                        let relative_path =
                            ObjectPath(msg.object_path.0[parent_path.len()..].to_vec());
                        context.object_path_button_to(
                            ui,
                            relative_path.to_string(),
                            &msg.object_path,
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
            context.object_path_button(ui, object_path);
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
        let (dynamic_image, egui_image) = context.image_cache.get_pair(id, image);
        let max_size = ui.available_size().min(egui_image.size_vec2());
        egui_image.show_max_size(ui, max_size);

        // TODO: support copying and saving images on web
        #[cfg(not(target_arch = "wasm32"))]
        ui.horizontal(|ui| image_options(ui, image, dynamic_image));

        // TODO: support histograms of non-RGB images too
        if let image::DynamicImage::ImageRgb8(rgb_image) =
            context.image_cache.get_dynamic_image(&msg.id, image)
        {
            ui.collapsing("Histogram", |ui| {
                histogram_ui(ui, rgb_image);
            });
        }
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

    let names = ["Red", "Green", "Blue"];
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
            ImageFormat::Luminance8 | ImageFormat::Rgba8 => {
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
