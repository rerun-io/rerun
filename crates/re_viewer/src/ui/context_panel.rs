use re_data_store::log_db::LogDb;
use re_log_types::LogMsg;

use crate::{data_ui::*, Preview, Selection, ViewerContext};

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
                    re_log::warn!("Unknown msg_id selected. Resetting selection");
                    ctx.rec_cfg.selection = Selection::None;
                    return;
                };

                match msg {
                    LogMsg::BeginRecordingMsg(msg) => {
                        show_begin_recording_msg(ui, msg);
                    }
                    LogMsg::TypeMsg(msg) => {
                        show_type_msg(ctx, ui, msg);
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
            Selection::Instance(instance_id) => {
                ui.label(format!("Selected object: {}", instance_id));
                ui.horizontal(|ui| {
                    ui.label("Type path:");
                    ctx.type_path_button(ui, instance_id.obj_path.obj_type_path());
                });
                ui.horizontal(|ui| {
                    ui.label("Object type:");
                    ui.label(obj_type_name(
                        ctx.log_db,
                        instance_id.obj_path.obj_type_path(),
                    ));
                });
                ui.separator();
                view_instance(ctx, ui, instance_id, Preview::Medium);
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

fn obj_type_name(log_db: &LogDb, obj_type_path: &ObjTypePath) -> String {
    if let Some(typ) = log_db.obj_types.get(obj_type_path) {
        format!("{typ:?}")
    } else {
        "<UNKNOWN>".to_owned()
    }
}
