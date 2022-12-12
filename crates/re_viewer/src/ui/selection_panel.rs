use re_data_store::{log_db::LogDb, ObjectProps};
use re_log_types::{LogMsg, ObjTypePath, TimeType};

use crate::{
    data_ui::{
        show_arrow_msg, show_begin_recording_msg, show_detailed_data_msg, show_path_op_msg,
        show_type_msg, view_data, view_instance, view_object,
    },
    ui::{view_text, Blueprint, SpaceView},
    Preview, Selection, ViewerContext,
};

// ---

/// The "Selection View" side-bar.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct SelectionPanel {}

impl SelectionPanel {
    #[allow(clippy::unused_self)]
    pub fn show_panel(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        egui_ctx: &egui::Context,
        blueprint: &mut Blueprint,
    ) {
        let panel = egui::SidePanel::right("selection_view")
            .resizable(true)
            .frame(ctx.re_ui.panel_frame());

        panel.show_animated(
            egui_ctx,
            blueprint.selection_panel_expanded,
            |ui: &mut egui::Ui| {
                if let Some(selection) = ctx.selection_history.selection_ui(ui, blueprint) {
                    ctx.set_selection(selection);
                }

                self.contents(ui, ctx, blueprint);
            },
        );
    }

    #[allow(clippy::unused_self)]
    fn contents(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut ViewerContext<'_>,
        blueprint: &mut Blueprint,
    ) {
        crate::profile_function!();

        ui.separator();

        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                self.inner_ui(ctx, blueprint, ui);
            });
    }

    #[allow(clippy::unused_self)]
    fn inner_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        blueprint: &mut Blueprint,
        ui: &mut egui::Ui,
    ) {
        match ctx.selection() {
            Selection::None => {
                ui.weak("(nothing)");
            }
            Selection::MsgId(msg_id) => {
                // ui.label(format!("Selected msg_id: {:?}", msg_id));
                ui.label("Selected a specific log message");

                let msg = if let Some(msg) = ctx.log_db.get_log_msg(&msg_id) {
                    msg
                } else {
                    re_log::warn!("Unknown msg_id selected. Resetting selection");
                    ctx.clear_selection();
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
                    LogMsg::PathOpMsg(msg) => {
                        show_path_op_msg(ctx, ui, msg);
                    }
                    LogMsg::ArrowMsg(msg) => show_arrow_msg(ctx, ui, msg, Preview::Medium),
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
                view_instance(ctx, ui, &instance_id, Preview::Medium);
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

                view_data(ctx, ui, &data_path);
            }
            Selection::Space(space) => {
                ui.label(format!("Selected space: {}", space));
                // I really don't know what we should show here.
            }
            Selection::SpaceView(space_view_id) => {
                if let Some(space_view) = blueprint.viewport.space_view(&space_view_id) {
                    ui.heading("SpaceView");
                    ui.add_space(4.0);

                    if ui.button("Remove from Viewport").clicked() {
                        blueprint.viewport.remove(&space_view_id);
                        blueprint.viewport.mark_user_interaction();
                        ctx.clear_selection();
                    } else {
                        if ui.button("Clone Space View").clicked() {
                            blueprint.viewport.add_space_view(space_view.clone());
                            blueprint.viewport.mark_user_interaction();
                        }

                        if let Some(space_view) = blueprint.viewport.space_view_mut(&space_view_id)
                        {
                            ui.add_space(4.0);
                            ui_space_view(ctx, ui, space_view);
                        }
                    }
                } else {
                    ctx.clear_selection();
                }
            }
            Selection::SpaceViewObjPath(space_view_id, obj_path) => {
                if let Some(space_view) = blueprint.viewport.space_view_mut(&space_view_id) {
                    egui::Grid::new("space_view_id_obj_path")
                        .striped(re_ui::ReUi::striped())
                        .show(ui, |ui| {
                            ui.label("Space View:");
                            ctx.space_view_button_to(ui, &space_view.name, space_view_id);
                            ui.end_row();

                            ui.label("Object Path:");
                            ctx.obj_path_button(ui, &obj_path);
                            ui.end_row();
                        });

                    let mut props = space_view.obj_tree_properties.projected.get(&obj_path);
                    obj_props_ui(ctx, ui, &mut props);
                    space_view
                        .obj_tree_properties
                        .individual
                        .set(obj_path, props);
                } else {
                    ctx.clear_selection();
                }
            }
        }
    }
}

fn obj_type_name(log_db: &LogDb, obj_type_path: &ObjTypePath) -> String {
    if let Some(typ) = log_db.obj_db.types.get(obj_type_path) {
        format!("{typ:?}")
    } else {
        "<UNKNOWN>".to_owned()
    }
}

fn ui_space_view(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, space_view: &mut SpaceView) {
    egui::Grid::new("space_view")
        .striped(re_ui::ReUi::striped())
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut space_view.name);
            ui.end_row();

            ui.label("Displayed Root Path:");
            ctx.obj_path_button(ui, &space_view.root_path);
            ui.end_row();

            ui.label("Reference Space Path:");
            ctx.obj_path_button(ui, &space_view.reference_space_path);
            ui.end_row();
        });

    ui.separator();

    use super::space_view::ViewCategory;
    match space_view.category {
        ViewCategory::Spatial => {
            ui.strong("Spatial view");
            space_view
                .view_state
                .state_spatial
                .show_settings_ui(ctx, ui);
        }
        ViewCategory::Tensor => {
            if let Some(state_tensor) = &mut space_view.view_state.state_tensor {
                ui.strong("Tensor view");
                state_tensor.ui(ui);
            }
        }
        ViewCategory::Text => {
            ui.strong("Text view");
            ui.add_space(4.0);
            view_text::text_filters_ui(ui, &mut space_view.view_state.state_text);
        }
        ViewCategory::Plot => {}
    }
}

fn obj_props_ui(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, obj_props: &mut ObjectProps) {
    use egui::NumExt;

    let ObjectProps {
        visible,
        visible_history,
    } = obj_props;

    ui.checkbox(visible, "Visible");

    ui.horizontal(|ui| {
        ui.label("Visible history:");
        match ctx.rec_cfg.time_ctrl.timeline().typ() {
            TimeType::Time => {
                let mut time_sec = visible_history.nanos as f32 / 1e9;
                let speed = (time_sec * 0.05).at_least(0.01);
                ui.add(
                    egui::DragValue::new(&mut time_sec)
                        .clamp_range(0.0..=f32::INFINITY)
                        .speed(speed)
                        .suffix("s"),
                )
                .on_hover_text("Include this much history of the object in the Space View");
                visible_history.nanos = (time_sec * 1e9).round() as _;
            }
            TimeType::Sequence => {
                let speed = (visible_history.sequences as f32 * 0.05).at_least(1.0);
                ui.add(
                    egui::DragValue::new(&mut visible_history.sequences)
                        .clamp_range(0.0..=f32::INFINITY)
                        .speed(speed),
                )
                .on_hover_text("Include this much history of the object in the Space View");
            }
        }
    });
}
