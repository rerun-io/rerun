use re_data_store::log_db::LogDb;
use re_log_types::LogMsg;

use crate::{data_ui::*, misc::PathBrowser, ui::Blueprint, Preview, Selection, ViewerContext};

use super::SpaceView;

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
        path_browser: &mut PathBrowser,
    ) {
        let shortcut = crate::ui::kb_shortcuts::TOGGLE_SELECTION_PANEL;
        blueprint.selection_panel_expanded ^= egui_ctx.input_mut().consume_shortcut(&shortcut);

        let panel_frame = ctx.design_tokens.panel_frame(egui_ctx);

        let collapsed = egui::SidePanel::right("selection_view_collapsed")
            .resizable(false)
            .frame(panel_frame)
            .default_width(16.0);
        let expanded = egui::SidePanel::right("selection_view_expanded")
            .resizable(true)
            .frame(panel_frame);

        egui::SidePanel::show_animated_between(
            egui_ctx,
            blueprint.selection_panel_expanded,
            collapsed,
            expanded,
            |ui: &mut egui::Ui, expansion: f32| {
                if expansion < 1.0 {
                    // Collapsed, or animating:
                    if ui
                        .small_button("⏴")
                        .on_hover_text(format!(
                            "Expand Selection View ({})",
                            egui_ctx.format_shortcut(&shortcut)
                        ))
                        .clicked()
                    {
                        blueprint.selection_panel_expanded = true;
                    }
                } else {
                    // Expanded:
                    if ui
                        .small_button("⏵")
                        .on_hover_text(format!(
                            "Collapse Selection View ({})",
                            egui_ctx.format_shortcut(&shortcut)
                        ))
                        .clicked()
                    {
                        blueprint.selection_panel_expanded = false;
                    }

                    ui.separator();

                    if let Some(new_selection) = path_browser.show(ctx, ui) {
                        ctx.rec_cfg.selection = new_selection.selection;
                    }

                    self.contents(ui, ctx, blueprint);
                }
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
                    LogMsg::PathOpMsg(msg) => {
                        show_path_op_msg(ctx, ui, msg);
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
            Selection::SpaceView(space_view_id) => {
                if let Some(space_view) = blueprint.viewport.get_space_view_mut(space_view_id) {
                    ui.label("SpaceView");
                    ui_space_view(ctx, ui, space_view);
                } else {
                    ctx.rec_cfg.selection = Selection::None;
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
    egui::Grid::new("space_view").striped(true).show(ui, |ui| {
        ui.label("Name:");
        ui.label(&space_view.name);
        ui.end_row();

        ui.label("Path:");
        ctx.obj_path_button(ui, &space_view.space_path);
        ui.end_row();
    });

    ui.separator();

    use super::space_view::ViewCategory;
    match space_view.category {
        ViewCategory::ThreeD => {
            ui.label("3D view.");
            super::view_3d::show_settings_ui(ctx, ui, &mut space_view.view_state.state_3d);
        }
        ViewCategory::Tensor => {
            if let Some(state_tensor) = &mut space_view.view_state.state_tensor {
                ui.label("Tensor view.");
                state_tensor.ui(ui);
            }
        }
        ViewCategory::TwoD | ViewCategory::Text | ViewCategory::Plot => {}
    }
}
