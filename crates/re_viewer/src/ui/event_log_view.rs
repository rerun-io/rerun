use itertools::Itertools as _;

use re_format::format_number;
use re_log_types::{
    ArrowMsg, BeginRecordingMsg, DataMsg, DataType, LogMsg, PathOpMsg, RecordingInfo, TypeMsg,
};

use crate::{Preview, ViewerContext};

/// An event log, a table of all log messages.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct EventLogView {}

impl EventLogView {
    #[allow(clippy::unused_self)]
    pub fn ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        crate::profile_function!();

        let messages = {
            crate::profile_scope!("Collecting messages");
            ctx.log_db.chronological_log_messages().collect_vec()
        };

        egui::Frame {
            inner_margin: re_ui::ReUi::view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| {
            ui.label(format!("{} log lines", format_number(ctx.log_db.len())));
            ui.separator();

            egui::ScrollArea::horizontal()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    message_table(ctx, ui, &messages);
                });
        });
    }
}

pub(crate) fn message_table(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, messages: &[&LogMsg]) {
    crate::profile_function!();

    use egui_extras::{Column, TableBuilder};

    TableBuilder::new(ui)
        .max_scroll_height(f32::INFINITY) // Fill up whole height
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .resizable(true)
        .column(Column::initial(100.0).at_least(50.0).clip(true)) // msg_id
        .column(Column::initial(130.0).at_least(50.0).clip(true)) // message type
        .columns(
            // timeline(s):
            Column::auto().clip(true).at_least(50.0),
            ctx.log_db.timelines().count(),
        )
        .column(Column::auto().clip(true).at_least(50.0)) // path
        .column(Column::remainder()) // payload
        .header(re_ui::ReUi::table_header_height(), |mut header| {
            re_ui::ReUi::setup_table_header(&mut header);
            header.col(|ui| {
                ui.strong("MsgID");
            });
            header.col(|ui| {
                ui.strong("Message Type");
            });
            for timeline in ctx.log_db.timelines() {
                header.col(|ui| {
                    ctx.timeline_button(ui, timeline);
                });
            }
            header.col(|ui| {
                ui.strong("Path");
            });
            header.col(|ui| {
                ui.strong("Payload");
            });
        })
        .body(|mut body| {
            re_ui::ReUi::setup_table_body(&mut body);

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
                let row_height = re_ui::ReUi::table_line_height();
                body.rows(row_height, messages.len(), |index, mut row| {
                    table_row(ctx, &mut row, messages[index], row_height);
                });
            }
        });
}

fn row_height(msg: &LogMsg) -> f32 {
    re_ui::ReUi::table_line_height()
        * match msg {
            LogMsg::DataMsg(msg) if msg.data.data_type() == DataType::Tensor => 3.0,
            _ => 1.0,
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
            let BeginRecordingMsg { msg_id, info } = msg;
            let RecordingInfo {
                application_id,
                recording_id,
                started,
                recording_source,
            } = info;

            row.col(|ui| {
                ui.label(msg_id.to_string());
            });
            row.col(|ui| {
                ui.monospace("BeginRecordingMsg");
                ui.label(format!("Source: {recording_source}"));
            });
            for _ in ctx.log_db.timelines() {
                row.col(|ui| {
                    ui.label("-");
                });
            }
            row.col(|ui| {
                ui.label(started.format());
            });
            row.col(|ui| {
                ui.monospace(format!("{application_id} - {recording_id:?}"));
            });
        }
        LogMsg::TypeMsg(msg) => {
            let TypeMsg {
                msg_id,
                type_path,
                obj_type,
            } = msg;

            row.col(|ui| {
                ui.label(msg_id.to_string());
            });
            row.col(|ui| {
                ui.monospace("TypeMsg");
            });
            for _ in ctx.log_db.timelines() {
                row.col(|ui| {
                    ui.label("-");
                });
            }
            row.col(|ui| {
                ctx.type_path_button(ui, type_path);
            });
            row.col(|ui| {
                ui.monospace(format!("{obj_type:?}"));
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
                ui.label(msg_id.to_string());
            });
            row.col(|ui| {
                ui.monospace("DataMsg");
            });
            for timeline in ctx.log_db.timelines() {
                row.col(|ui| {
                    if let Some(value) = time_point.get(timeline) {
                        ctx.time_button(ui, timeline, *value);
                    }
                });
            }
            row.col(|ui| {
                ctx.data_path_button(ui, data_path);
            });
            row.col(|ui| {
                crate::data_ui::logged_data_ui(ctx, ui, data, Preview::Specific(row_height));
            });
        }
        LogMsg::PathOpMsg(msg) => {
            let PathOpMsg {
                msg_id,
                time_point,
                path_op,
            } = msg;

            row.col(|ui| {
                ui.label(msg_id.to_string());
            });
            row.col(|ui| {
                ui.monospace("PathOpMsg");
            });
            for timeline in ctx.log_db.timelines() {
                row.col(|ui| {
                    if let Some(value) = time_point.get(timeline) {
                        ctx.time_button(ui, timeline, *value);
                    }
                });
            }
            row.col(|ui| {
                ctx.obj_path_button(ui, path_op.obj_path());
            });
            row.col(|ui| {
                crate::data_ui::path_op_ui(ctx, ui, path_op);
            });
        }
        LogMsg::ArrowMsg(msg) => {
            let ArrowMsg {
                msg_id,
                schema: _,
                chunk: _,
            } = msg;

            row.col(|ui| {
                ui.label(msg_id.to_string());
            });
            row.col(|ui| {
                ui.monospace("ArrowMsg");
            });
            //TODO(john) extract timelines
            for _timeline in ctx.log_db.timelines() {
                row.col(|ui| {
                    ui.label("-");
                });
            }
            //TODO(john) extract object path
            row.col(|ui| {
                ui.label("-");
            });

            row.col(|ui| {
                crate::data_ui::logged_arrow_data_ui(
                    ctx,
                    ui,
                    msg_id,
                    msg,
                    Preview::Specific(row_height),
                );
            });
        }
    }
}
