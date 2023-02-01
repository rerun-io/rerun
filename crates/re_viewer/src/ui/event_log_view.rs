use itertools::Itertools as _;

use re_arrow_store::{LatestAtQuery, TimeInt};
use re_format::format_number;
use re_log_types::{
    msg_bundle::MsgBundle, BeginRecordingMsg, EntityPathOpMsg, LogMsg, RecordingInfo,
};

use crate::{UiVerbosity, ViewerContext};

use super::data_ui::DataUi;

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

fn row_height(_msg: &LogMsg) -> f32 {
    // TODO(emilk): make rows with images (tensors) higher!
    re_ui::ReUi::table_line_height()
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
                is_official_example,
            } = info;

            row.col(|ui| {
                ctx.msg_id_button(ui, *msg_id);
            });
            row.col(|ui| {
                ui.monospace("BeginRecordingMsg");
                ui.label(format!("Source: {recording_source}"));
                ui.label(format!("Official example: {is_official_example}"));
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
        LogMsg::EntityPathOpMsg(msg) => {
            let EntityPathOpMsg {
                msg_id,
                time_point,
                path_op,
            } = msg;

            row.col(|ui| {
                ctx.msg_id_button(ui, *msg_id);
            });
            row.col(|ui| {
                ui.monospace("EntityPathOpMsg");
            });
            for timeline in ctx.log_db.timelines() {
                row.col(|ui| {
                    if let Some(value) = time_point.get(timeline) {
                        ctx.time_button(ui, timeline, *value);
                    }
                });
            }
            row.col(|ui| {
                ctx.entity_path_button(ui, None, path_op.entity_path());
            });
            row.col(|ui| {
                let timeline = *ctx.rec_cfg.time_ctrl.timeline();
                let query = LatestAtQuery::new(
                    timeline,
                    time_point.get(&timeline).copied().unwrap_or(TimeInt::MAX),
                );
                path_op.data_ui(ctx, ui, UiVerbosity::All, &query);
            });
        }
        LogMsg::ArrowMsg(msg) => match MsgBundle::try_from(msg) {
            Ok(MsgBundle {
                msg_id,
                entity_path,
                time_point,
                components,
            }) => {
                row.col(|ui| {
                    ctx.msg_id_button(ui, msg_id);
                });
                row.col(|ui| {
                    ui.monospace("ArrowMsg");
                });
                for timeline in ctx.log_db.timelines() {
                    row.col(|ui| {
                        if let Some(value) = time_point.get(timeline) {
                            ctx.time_button(ui, timeline, *value);
                        }
                    });
                }
                row.col(|ui| {
                    ctx.entity_path_button(ui, None, &entity_path);
                });

                row.col(|ui| {
                    let timeline = *ctx.rec_cfg.time_ctrl.timeline();
                    let query = LatestAtQuery::new(
                        timeline,
                        time_point.get(&timeline).copied().unwrap_or(TimeInt::MAX),
                    );
                    components.data_ui(ctx, ui, UiVerbosity::MaxHeight(row_height), &query);
                });
            }
            Err(err) => {
                re_log::error_once!("Bad arrow payload: {err}",);
                row.col(|ui| {
                    ui.label("Bad Arrow Payload".to_owned());
                });
            }
        },
        LogMsg::Goodbye(msg_id) => {
            row.col(|ui| {
                ctx.msg_id_button(ui, *msg_id);
            });
            row.col(|ui| {
                ui.monospace("Goodbye");
            });
        }
    }
}
