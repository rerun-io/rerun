use re_data_store::{query_transform, ObjPath, ObjectProps};
use re_log_types::{LogMsg, TimeType};

use crate::{
    ui::{view_spatial::SpatialNavigationMode, Blueprint},
    Preview, Selection, ViewerContext,
};

use super::{data_ui::DataUi, space_view::ViewState};

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

        if !ctx.selection().is_valid(ctx, blueprint) {
            // TODO(emilk): also prune history
            ctx.clear_selection();
        }

        ui.separator();

        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let selection = ctx.selection();
                if selection == Selection::None {
                    ui.weak("(none)");
                    return;
                }

                selection_ui(ui, ctx, blueprint, &selection);

                ui.separator();

                ui.collapsing("Data", |ui| {
                    data_ui(ui, ctx, &selection, Preview::Medium);
                });

                ui.separator();

                ui.collapsing("Blueprint", |ui| {
                    blueprint_ui(ui, ctx, blueprint, &selection);
                });
            });
    }
}

/// What is selected? Not the contents, just the short id of it.
fn selection_ui(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    blueprint: &mut Blueprint,
    selection: &Selection,
) {
    match selection {
        Selection::None => {
            ui.weak("(nothing)");
        }
        Selection::MsgId(msg_id) => {
            ui.horizontal(|ui| {
                ui.label("Message:");
                ui.code(msg_id.to_string());
            });
        }
        Selection::Instance(instance_id) => {
            ui.horizontal(|ui| {
                ui.label("Instance:");
                ui.code(instance_id.to_string());
            });
        }
        Selection::DataPath(data_path) => {
            ui.horizontal(|ui| {
                ui.label("Data path:");
                ui.code(data_path.to_string());
            });
        }
        Selection::SpaceView(space_view_id) => {
            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                ui.horizontal(|ui| {
                    ui.label("Space view:");
                    ui.text_edit_singleline(&mut space_view.name);
                });
            }
        }
        Selection::SpaceViewObjPath(space_view_id, obj_path) => {
            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                egui::Grid::new("space_view_id_obj_path").show(ui, |ui| {
                    ui.label("Object Path:");
                    ctx.obj_path_button(ui, obj_path);
                    ui.end_row();

                    ui.weak("in");
                    ui.end_row();

                    ui.label("Space View:");
                    ctx.space_view_button_to(ui, &space_view.name, *space_view_id);
                    ui.end_row();
                });
            }
        }
        Selection::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                if let Some(group) = space_view
                    .data_blueprint
                    .get_group_mut(*data_blueprint_group_handle)
                {
                    egui::Grid::new("data_blueprint_group").show(ui, |ui| {
                        ui.label("Data Group:");
                        ui.text_edit_singleline(&mut group.display_name);
                        ui.end_row();

                        ui.weak("in");
                        ui.end_row();

                        ui.label("Space View:");
                        ctx.space_view_button_to(ui, &space_view.name, *space_view_id);
                        ui.end_row();
                    });
                }
            }
        }
    }
}

/// What is the data contained by this selection?
fn data_ui(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    selection: &Selection,
    preview: Preview,
) {
    match selection {
        Selection::None => {
            ui.weak("(nothing)");
        }
        Selection::MsgId(msg_id) => {
            if let Some(msg) = ctx.log_db.get_log_msg(msg_id) {
                match msg {
                    LogMsg::BeginRecordingMsg(msg) => msg.data_ui(ctx, ui, preview),
                    LogMsg::TypeMsg(msg) => msg.data_ui(ctx, ui, preview),
                    LogMsg::DataMsg(msg) => {
                        msg.detailed_data_ui(ctx, ui, preview);
                        ui.separator();
                        msg.data_path.obj_path.data_ui(ctx, ui, preview)
                    }
                    LogMsg::PathOpMsg(msg) => msg.data_ui(ctx, ui, preview),
                    LogMsg::ArrowMsg(msg) => msg.data_ui(ctx, ui, preview),
                };
            }
        }
        Selection::Instance(instance_id) => {
            instance_id.data_ui(ctx, ui, preview);
        }
        Selection::DataPath(data_path) => {
            ui.horizontal(|ui| {
                ui.label("Object path:");
                ctx.obj_path_button(ui, &data_path.obj_path);
            });
            data_path.data_ui(ctx, ui, preview);
        }
        Selection::SpaceView(_) => {}
        Selection::SpaceViewObjPath(_, obj_path) => {
            obj_path.data_ui(ctx, ui, preview);
        }
        Selection::DataBlueprintGroup(_, _) => {}
    }
}

/// What is the blueprint stuff for this selection?
fn blueprint_ui(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    blueprint: &mut Blueprint,
    selection: &Selection,
) {
    match selection {
        Selection::None | Selection::MsgId(_) | Selection::Instance(_) | Selection::DataPath(_) => {
        }

        Selection::SpaceView(space_view_id) => {
            if let Some(space_view) = blueprint.viewport.space_view(space_view_id) {
                if ui.button("Remove from Viewport").clicked() {
                    blueprint.viewport.remove(space_view_id);
                    blueprint.viewport.mark_user_interaction();
                    ctx.clear_selection();
                } else {
                    if ui.button("Clone Space View").clicked() {
                        blueprint.viewport.add_space_view(space_view.clone());
                        blueprint.viewport.mark_user_interaction();
                    }

                    if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                        ui.add_space(4.0);
                        space_view.selection_ui(ctx, ui);
                    }
                }
            }
        }

        Selection::SpaceViewObjPath(space_view_id, obj_path) => {
            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                let data_blueprint = space_view.data_blueprint.data_blueprints_individual();
                let mut props = data_blueprint.get(obj_path);
                obj_props_ui(ctx, ui, Some(obj_path), &mut props, &space_view.view_state);
                data_blueprint.set(obj_path.clone(), props);
            }
        }

        Selection::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                if let Some(group) = space_view
                    .data_blueprint
                    .get_group_mut(*data_blueprint_group_handle)
                {
                    ui.strong("Group");
                    ui.add_space(4.0);

                    group.selection_ui(ctx, ui);

                    ui.separator();

                    obj_props_ui(
                        ctx,
                        ui,
                        None,
                        &mut group.properties_individual,
                        &space_view.view_state,
                    );

                    ui.separator();
                } else {
                    ctx.clear_selection();
                }
            }
        }
    }
}

fn obj_props_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    obj_path: Option<&ObjPath>,
    obj_props: &mut ObjectProps,
    view_state: &ViewState,
) {
    use egui::NumExt;

    let ObjectProps {
        visible,
        visible_history,
        interactive,
        ..
    } = obj_props;

    ui.checkbox(visible, "Visible");
    ui.checkbox(interactive, "Interactive")
        .on_hover_text("If disabled, the object will not react to any mouse interaction");

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

    if view_state.state_spatial.nav_mode == SpatialNavigationMode::ThreeD {
        if let Some(obj_path) = obj_path {
            let timeline = ctx.rec_cfg.time_ctrl.timeline();
            let query_time = ctx.rec_cfg.time_ctrl.time_i64();
            if let Some(re_log_types::Transform::Pinhole(pinhole)) =
                query_transform(&ctx.log_db.obj_db, timeline, obj_path, query_time)
            {
                ui.horizontal(|ui| {
                    ui.label("Image plane distance:");
                    let mut distance = obj_props.pinhole_image_plane_distance(&pinhole);
                    let speed = (distance * 0.05).at_least(0.01);
                    if ui
                        .add(
                            egui::DragValue::new(&mut distance)
                                .clamp_range(0.0..=f32::INFINITY)
                                .speed(speed),
                        )
                        .on_hover_text("Controls how far away the image plane is.")
                        .changed()
                    {
                        obj_props.set_pinhole_image_plane_distance(distance);
                    }
                });
            }
        }
    }
}
