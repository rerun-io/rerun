use re_data_store::{query_transform, EntityPath, EntityProperties};
use re_log_types::TimeType;

use crate::{
    ui::{view_spatial::SpatialNavigationMode, Blueprint},
    Selection, UiVerbosity, ViewerContext,
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

                ui.separator();

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

        let query = ctx.current_query();

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
                                selection.data_ui(ctx, ui, UiVerbosity::All, &query);
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
pub fn what_is_selected_ui(
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
        Selection::ComponentPath(component_path) => {
            ui.horizontal(|ui| {
                ui.label("Entity component:");
                ctx.component_path_button(ui, component_path);
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
        Selection::InstancePath(space_view_id, instance_path) => {
            egui::Grid::new("space_view_id_entity_path").show(ui, |ui| {
                if instance_path.instance_key.is_splat() {
                    ui.label("Entity:");
                } else {
                    ui.label("Entity instance:");
                }
                ctx.instance_path_button(ui, *space_view_id, instance_path);
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

impl DataUi for Selection {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match self {
            Selection::SpaceView(_) | Selection::DataBlueprintGroup(_, _) => {
                ui.weak("(nothing)");
            }
            Selection::MsgId(msg_id) => {
                msg_id.data_ui(ctx, ui, verbosity, query);
            }
            Selection::ComponentPath(component_path) => {
                component_path.data_ui(ctx, ui, verbosity, query);
            }
            Selection::InstancePath(_, instance_path) => {
                instance_path.data_ui(ctx, ui, verbosity, query);
            }
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
            // TODO(andreas): Show space views that contains entities that's part of this message.
            ui.weak("(nothing)");
        }

        Selection::ComponentPath(component_path) => {
            list_existing_data_blueprints(ui, ctx, component_path.entity_path(), blueprint);
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

        Selection::InstancePath(space_view_id, instance_path) => {
            if let Some(space_view) = space_view_id
                .and_then(|space_view_id| blueprint.viewport.space_view_mut(&space_view_id))
            {
                if instance_path.instance_key.is_specific() {
                    ui.horizontal(|ui| {
                        ui.label("Part of");
                        ctx.entity_path_button(ui, *space_view_id, &instance_path.entity_path);
                    });
                    // TODO(emilk): show the values of this specific instance (e.g. point in the point cloud)!
                } else {
                    // splat - the whole entity
                    let data_blueprint = space_view.data_blueprint.data_blueprints_individual();
                    let mut props = data_blueprint.get(&instance_path.entity_path);
                    entity_props_ui(
                        ctx,
                        ui,
                        Some(&instance_path.entity_path),
                        &mut props,
                        &space_view.view_state,
                    );
                    data_blueprint.set(instance_path.entity_path.clone(), props);
                }
            } else {
                list_existing_data_blueprints(ui, ctx, &instance_path.entity_path, blueprint);
            }
        }

        Selection::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                if let Some(group) = space_view
                    .data_blueprint
                    .group_mut(*data_blueprint_group_handle)
                {
                    entity_props_ui(
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
    entity_path: &EntityPath,
    blueprint: &Blueprint,
) {
    let space_views_with_path = blueprint
        .viewport
        .space_views_containing_entity_path(entity_path);

    if space_views_with_path.is_empty() {
        ui.weak("(Not shown in any Space View)");
        // TODO(andreas): Offer options for adding?
    } else {
        ui.label("Is shown in:");

        ui.indent("list of data blueprints indent", |ui| {
            for space_view_id in &space_views_with_path {
                if let Some(space_view) = blueprint.viewport.space_view(space_view_id) {
                    ctx.entity_path_button_to(
                        ui,
                        Some(*space_view_id),
                        entity_path,
                        &space_view.name,
                    );
                }
            }
        });
    }
}

fn entity_props_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: Option<&EntityPath>,
    entity_props: &mut EntityProperties,
    view_state: &ViewState,
) {
    use egui::NumExt;

    let EntityProperties {
        visible,
        visible_history,
        interactive,
        ..
    } = entity_props;

    ui.checkbox(visible, "Visible");
    ui.checkbox(interactive, "Interactive")
        .on_hover_text("If disabled, the entity will not react to any mouse interaction");

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
                .on_hover_text("Include this much history of the entity in the Space View");
                visible_history.nanos = (time_sec * 1e9).round() as _;
            }
            TimeType::Sequence => {
                let speed = (visible_history.sequences as f32 * 0.05).at_least(1.0);
                ui.add(
                    egui::DragValue::new(&mut visible_history.sequences)
                        .clamp_range(0.0..=f32::INFINITY)
                        .speed(speed),
                )
                .on_hover_text("Include this much history of the entity in the Space View");
            }
        }
    });

    if view_state.state_spatial.nav_mode == SpatialNavigationMode::ThreeD {
        if let Some(entity_path) = entity_path {
            let query = ctx.current_query();
            if let Some(re_log_types::Transform::Pinhole(pinhole)) =
                query_transform(&ctx.log_db.entity_db, entity_path, &query)
            {
                ui.horizontal(|ui| {
                    ui.label("Image plane distance:");
                    let mut distance = entity_props.pinhole_image_plane_distance(&pinhole);
                    let speed = (distance * 0.05).at_least(0.01);
                    if ui
                        .add(
                            egui::DragValue::new(&mut distance)
                                .clamp_range(0.0..=1.0e8)
                                .speed(speed),
                        )
                        .on_hover_text("Controls how far away the image plane is.")
                        .changed()
                    {
                        entity_props.set_pinhole_image_plane_distance(distance);
                    }
                });
            }
        }
    }
}
