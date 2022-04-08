use eframe::egui;
use log_types::{LogMsg, ObjectPath};

use crate::{LogDb, Preview, Selection, ViewerContext};

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct ContextPanel {}

impl ContextPanel {
    pub fn ui(&mut self, log_db: &LogDb, context: &mut ViewerContext, ui: &mut egui::Ui) {
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
                egui::ScrollArea::both().show(ui, |ui| {
                    let mut messages = context.time_control.latest_of_each_object_vec(log_db);
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
        crate::space_view::show_log_msg(context, ui, msg, Preview::Medium);

        let messages = context.time_control.latest_of_each_object_vec(log_db);

        ui.separator();

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
