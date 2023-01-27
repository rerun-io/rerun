use re_arrow_store::TimeInt;
use re_data_store::{query_transform, ObjPath, ObjectProps};
use re_log_types::TimeType;

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
            .min_width(120.0)
            .default_width(250.0)
            .resizable(true)
            .frame(ctx.re_ui.panel_frame());

        panel.show_animated(
            egui_ctx,
            blueprint.selection_panel_expanded,
            |ui: &mut egui::Ui| {
                if let Some(selection) = ctx.rec_cfg.selection_state.selection_ui(ui, blueprint) {
                    ctx.set_multi_selection(selection.iter().cloned());
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
                if ctx.selection().is_empty() {
                    return;
                }

                let num_selections = ctx.selection().len();
                let selection = ctx.selection().to_vec();
                for (i, selection) in selection.iter().enumerate() {
                    ui.push_id(i, |ui| {
                        what_is_selected_ui(ui, ctx, blueprint, selection);

                        egui::CollapsingHeader::new("Data")
                            .default_open(true)
                            .show(ui, |ui| {
                                data_ui(ui, ctx, selection, Preview::Large);
                            });

                        egui::CollapsingHeader::new("Blueprint")
                            .default_open(true)
                            .show(ui, |ui| {
                                blueprint_ui(ui, ctx, blueprint, selection);
                            });

                        if num_selections > i + 1 {
                            ui.add(egui::Separator::default().spacing(12.0));
                        }
                    });
                }
            });
    }
}

/// What is selected? Not the contents, just the short id of it.
fn what_is_selected_ui(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    blueprint: &mut Blueprint,
    selection: &Selection,
) {
    match selection {
        Selection::MsgId(msg_id) => {
            ui.horizontal(|ui| {
                ui.label("Message ID:");
                ctx.msg_id_button(ui, *msg_id);
            });
        }
        Selection::DataPath(data_path) => {
            ui.horizontal(|ui| {
                ui.label("Data path:");
                ctx.data_path_button(ui, data_path);
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
        Selection::Instance(space_view_id, instance_id) => {
            egui::Grid::new("space_view_id_obj_path").show(ui, |ui| {
                if instance_id.instance_index.is_none() {
                    ui.label("Object Path:");
                } else {
                    ui.label("Instance:");
                }
                ctx.instance_id_button(ui, *space_view_id, instance_id);
                ui.end_row();

                if let Some(space_view_id) = space_view_id {
                    if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                        ui.label("in Space View:");
                        ctx.space_view_button_to(ui, &space_view.name, *space_view_id);
                        ui.end_row();
                    }
                }
            });
        }
        Selection::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                if let Some(group) = space_view
                    .data_blueprint
                    .group_mut(*data_blueprint_group_handle)
                {
                    egui::Grid::new("data_blueprint_group")
                        .num_columns(2)
                        .show(ui, |ui| {
                            ui.label("Data Group:");
                            ui.text_edit_singleline(&mut group.display_name);
                            ui.end_row();

                            ui.label("in Space View:");
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
        Selection::SpaceView(_) | Selection::DataBlueprintGroup(_, _) => {
            ui.weak("(nothing)");
        }
        Selection::MsgId(msg_id) => {
            msg_id.data_ui(ctx, ui, preview);
        }
        Selection::DataPath(data_path) => {
            data_path.data_ui(ctx, ui, preview);
        }
        Selection::Instance(_, instance_id) => {
            instance_id.data_ui(ctx, ui, preview);
        }
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
        Selection::MsgId(_) => {
            // TODO(andreas): Show space views that show objects from this message.
            ui.weak("(nothing)");
        }

        Selection::DataPath(data_path) => {
            list_existing_data_blueprints(ui, ctx, data_path.obj_path(), blueprint);
        }

        Selection::SpaceView(space_view_id) => {
            if let Some(space_view) = blueprint.viewport.space_view(space_view_id) {
                if ui.button("Remove from Viewport").clicked() {
                    blueprint.viewport.remove(space_view_id);
                    blueprint.viewport.mark_user_interaction();
                    ctx.selection_state_mut().clear_current();
                } else {
                    if ui.button("Clone Space View").clicked() {
                        let mut new_space_view = space_view.clone();
                        new_space_view.id = super::SpaceViewId::random();
                        blueprint.viewport.add_space_view(new_space_view);
                        blueprint.viewport.mark_user_interaction();
                    }

                    if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                        ui.add_space(4.0);
                        space_view.selection_ui(ctx, ui);
                    }
                }
            }
        }

        Selection::Instance(space_view_id, instance_id) => {
            if let Some(space_view) = space_view_id
                .and_then(|space_view_id| blueprint.viewport.space_view_mut(&space_view_id))
            {
                if instance_id.instance_index.is_some() {
                    ui.horizontal(|ui| {
                        ui.label("Part of");
                        ctx.obj_path_button(ui, *space_view_id, &instance_id.obj_path);
                    });
                } else {
                    let data_blueprint = space_view.data_blueprint.data_blueprints_individual();
                    let mut props = data_blueprint.get(&instance_id.obj_path);
                    obj_props_ui(
                        ctx,
                        ui,
                        Some(&instance_id.obj_path),
                        &mut props,
                        &space_view.view_state,
                    );
                    data_blueprint.set(instance_id.obj_path.clone(), props);
                }
            } else {
                list_existing_data_blueprints(ui, ctx, &instance_id.obj_path, blueprint);
            }
        }

        Selection::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                if let Some(group) = space_view
                    .data_blueprint
                    .group_mut(*data_blueprint_group_handle)
                {
                    obj_props_ui(
                        ctx,
                        ui,
                        None,
                        &mut group.properties_individual,
                        &space_view.view_state,
                    );

                    ui.separator();
                } else {
                    ctx.selection_state_mut().clear_current();
                }
            }
        }
    }
}

fn list_existing_data_blueprints(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    obj_path: &ObjPath,
    blueprint: &Blueprint,
) {
    let space_views_with_path = blueprint.viewport.space_views_containing_obj_path(obj_path);

    if space_views_with_path.is_empty() {
        ui.weak("(Not shown in any Space View)");
        // TODO(andreas): Offer options for adding?
    } else {
        ui.label("Is shown in:");

        ui.indent("list of data blueprints indent", |ui| {
            for space_view_id in &space_views_with_path {
                if let Some(space_view) = blueprint.viewport.space_view(space_view_id) {
                    ctx.obj_path_button_to(ui, Some(*space_view_id), obj_path, &space_view.name);
                }
            }
        });
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
            let query_time = ctx
                .rec_cfg
                .time_ctrl
                .time_i64()
                .map_or(TimeInt::MAX, TimeInt::from);
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
