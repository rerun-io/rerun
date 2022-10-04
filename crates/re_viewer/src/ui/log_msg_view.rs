use crate::ViewerContext;
use re_data_store::{InstanceId, InstanceProps, LogMessage};
use re_log_types::*;

// -----------------------------------------------------------------------------

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct StateLogMsg {
    /// What the mouse is hovering (from previous frame)
    #[serde(skip)]
    hovered_instance: Option<InstanceId>,
}

pub(crate) fn show(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut StateLogMsg,
    space: Option<&ObjPath>,
    objects: &re_data_store::Objects<'_>,
) {
    crate::profile_function!();

    let messages = objects.log_message.iter().collect::<Vec<_>>();

    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
        ui.label(format!("{} log messages", objects.log_message.len()));
        ui.separator();
        log_table(ctx, ui, &messages);
    });
}

fn log_table(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    messages: &[(&InstanceProps, &LogMessage)],
) {
    egui::ScrollArea::horizontal()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            use egui_extras::Size;
            egui_extras::TableBuilder::new(ui)
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .resizable(true)
                .columns(
                    Size::initial(200.0).at_least(100.0),
                    ctx.log_db.time_points.0.len(),
                ) // time(s)
                .column(Size::initial(60.0).at_least(60.0)) // path
                .column(Size::initial(60.0).at_least(60.0)) // level
                .column(Size::remainder().at_least(180.0)) // text
                .header(20.0, |mut header| {
                    for time_source in ctx.log_db.time_points.0.keys() {
                        header.col(|ui| {
                            ui.heading(time_source.name().as_str());
                        });
                    }
                    header.col(|ui| {
                        ui.heading("Path");
                    });
                    header.col(|ui| {
                        ui.heading("Level");
                    });
                    header.col(|ui| {
                        ui.heading("Message");
                    });
                })
                .body(|body| {
                    const ROW_HEIGHT: f32 = 18.0;
                    body.rows(ROW_HEIGHT, messages.len(), |index, mut row| {
                        let (props, msg) = messages[index];

                        let inner = ctx.log_db.get_log_msg(props.msg_id);
                        if inner.is_none() {
                            // TODO: add error message inline? log warning? assert?
                            // TODO: crash if dev build?
                            return;
                        }

                        let inner = match inner.unwrap() {
                            LogMsg::DataMsg(inner) => inner,
                            _ => unreachable!("LogMessage must be logged as data"),
                        };

                        // time(s)
                        for time_source in ctx.log_db.time_points.0.keys() {
                            row.col(|ui| {
                                if let Some(value) = inner.time_point.0.get(time_source) {
                                    ctx.time_button(ui, time_source, *value);
                                }
                            });
                        }

                        // path
                        row.col(|ui| {
                            ui.label(props.obj_path.to_string());
                        });

                        // level
                        row.col(|ui| {
                            ui.label(
                                msg.level
                                    .map_or_else(|| "-".to_owned(), |lvl| lvl.to_string()),
                            );
                        });

                        // text
                        row.col(|ui| {
                            ui.label(msg.text.to_owned());
                        });
                    });
                });
        });
}
