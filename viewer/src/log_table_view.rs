use eframe::egui;

use log_types::*;

use crate::{viewer_context::ViewerContext, LogDb, Preview};

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct LogTableView {}

impl LogTableView {
    #[allow(clippy::unused_self)]
    pub fn ui(&mut self, log_db: &LogDb, context: &mut ViewerContext, ui: &mut egui::Ui) {
        ui.label(format!("{} log lines", log_db.messages.len()));
        ui.separator();

        let mut messages: Vec<&LogMsg> = log_db.messages.values().collect();
        messages.sort_by_key(|key| (&key.time_point, key.id));

        message_table(log_db, context, ui, &messages);
    }
}

pub(crate) fn message_table(
    log_db: &LogDb,
    context: &mut ViewerContext,
    ui: &mut egui::Ui,
    messages: &[&LogMsg],
) {
    use egui_extras::{Size, TableBuilder};

    let row_height = 48.0;

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .columns(
            Size::initial(200.0).at_least(100.0),
            log_db.time_points.0.len(),
        )
        .column(Size::initial(300.0).at_least(100.0)) // path
        .column(Size::initial(200.0).at_least(100.0)) // space
        .column(Size::remainder().at_least(180.0)) // data
        .header(20.0, |mut header| {
            for time_source in log_db.time_points.0.keys() {
                header.col(|ui| {
                    ui.heading(time_source);
                });
            }
            header.col(|ui| {
                ui.heading("Path");
            });
            header.col(|ui| {
                ui.heading("Space");
            });
            header.col(|ui| {
                ui.heading("Data");
            });
        })
        .body(|body| {
            body.rows(row_height, messages.len(), |index, mut row| {
                let msg = &messages[index];

                let LogMsg {
                    id,
                    time_point,
                    object_path,
                    space,
                    data,
                } = msg;

                for time_source in log_db.time_points.0.keys() {
                    row.col(|ui| {
                        if let Some(value) = time_point.0.get(time_source) {
                            ui.label(value.to_string());
                        }
                    });
                }
                row.col(|ui| {
                    ui.label(format!("{object_path}"));
                });
                row.col(|ui| {
                    if let Some(space) = space {
                        context.space_button(ui, space);
                    }
                });
                row.col(|ui| {
                    crate::space_view::ui_data(
                        context,
                        ui,
                        id,
                        data,
                        Preview::Specific(row_height),
                    );
                });
            });
        });
}
