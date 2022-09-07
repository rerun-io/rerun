use itertools::Itertools as _;
use re_log_types::*;

use crate::{Preview, ViewerContext};

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct LogTableView {}

impl LogTableView {
    #[allow(clippy::unused_self)]
    pub fn ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        crate::profile_function!();

        ui.label(format!("{} log lines", ctx.log_db.len()));
        ui.separator();

        let messages = {
            crate::profile_scope!("Collecting messages");
            ctx.log_db.chronological_log_messages().collect_vec()
        };

        egui::ScrollArea::horizontal()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                message_table(ctx, ui, &messages);
            });
    }
}

pub(crate) fn message_table(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, messages: &[&LogMsg]) {
    crate::profile_function!();

    use egui_extras::Size;

    egui_extras::TableBuilder::new(ui)
        .striped(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .resizable(true)
        .columns(
            Size::initial(200.0).at_least(100.0),
            ctx.log_db.time_points.0.len(),
        )
        .column(Size::initial(300.0).at_least(60.0)) // message type
        .column(Size::initial(300.0).at_least(120.0)) // path
        .column(Size::remainder().at_least(180.0)) // payload
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.heading("Message Type");
            });
            for time_source in ctx.log_db.time_points.0.keys() {
                header.col(|ui| {
                    ui.heading(time_source.name().as_str());
                });
            }
            header.col(|ui| {
                ui.heading("Path");
            });
            header.col(|ui| {
                ui.heading("Payload");
            });
        })
        .body(|body| {
            // for MANY messages, `heterogeneous_rows` is too slow. TODO(emilk): how many?
            if messages.len() < 10_000_000 {
                body.heterogeneous_rows(
                    messages.iter().copied().map(row_height),
                    |index, mut row| {
                        let msg = messages[index];
                        table_row(ctx, &mut row, msg, row_height(msg));
                    },
                );
            } else {
                const ROW_HEIGHT: f32 = 18.0;
                body.rows(ROW_HEIGHT, messages.len(), |index, mut row| {
                    table_row(ctx, &mut row, messages[index], ROW_HEIGHT);
                });
            }
        });
}

fn row_height(msg: &LogMsg) -> f32 {
    match msg {
        LogMsg::DataMsg(msg) if msg.data.data_type() == DataType::Tensor => 48.0,
        _ => 18.0,
    }
}

fn table_row(
    ctx: &mut ViewerContext<'_>,
    row: &mut egui_extras::TableRow<'_, '_>,
    msg: &LogMsg,
    row_height: f32,
) {
    match msg {
        LogMsg::BeginRecordingMsg(msg) => {
            let BeginRecordingMsg { msg_id: _, info } = msg;
            let RecordingInfo {
                recording_id,
                started,
                recording_source,
            } = info;

            row.col(|ui| {
                ui.monospace("BeginRecordingMsg");
                ui.label(format!("Source: {recording_source}"));
            });
            for _ in ctx.log_db.time_points.0.keys() {
                row.col(|ui| {
                    ui.label("-");
                });
            }
            row.col(|ui| {
                ui.label(started.format());
            });
            row.col(|ui| {
                ui.monospace(format!("{recording_id:?}"));
            });
        }
        LogMsg::TypeMsg(msg) => {
            let TypeMsg {
                msg_id: _,
                type_path,
                object_type,
            } = msg;

            row.col(|ui| {
                ui.monospace("TypeMsg");
            });
            for _ in ctx.log_db.time_points.0.keys() {
                row.col(|ui| {
                    ui.label("-");
                });
            }
            row.col(|ui| {
                ctx.type_path_button(ui, type_path);
            });
            row.col(|ui| {
                ui.monospace(format!("{object_type:?}"));
            });
        }
        LogMsg::DataMsg(msg) => {
            let DataMsg {
                msg_id,
                time_point,
                data_path,
                data,
            } = msg;

            row.col(|ui| {
                ui.monospace("DataMsg");
            });
            for time_source in ctx.log_db.time_points.0.keys() {
                row.col(|ui| {
                    if let Some(value) = time_point.0.get(time_source) {
                        ctx.time_button(ui, time_source, *value);
                    }
                });
            }
            row.col(|ui| {
                ctx.data_path_button(ui, data_path);
            });
            row.col(|ui| {
                crate::data_ui::ui_logged_data(
                    ctx,
                    ui,
                    msg_id,
                    data,
                    Preview::Specific(row_height),
                );
            });
        }
    }
}
