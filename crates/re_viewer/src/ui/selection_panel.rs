use egui::NumExt as _;

use re_components::{Pinhole, Tensor, TensorDataMeaning};
use re_data_store::{ColorMapper, Colormap, EditableAutoValue, EntityPath, EntityProperties};
use re_data_ui::{item_ui, DataUi};
use re_log_types::TimeType;
use re_viewer_context::{Item, SpaceViewId, UiVerbosity, ViewerContext};
use re_viewport::{Viewport, ViewportState};

use crate::ui::Blueprint;

use super::selection_history_ui::SelectionHistoryUi;

// ---

/// The "Selection View" side-bar.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct SelectionPanel {
    selection_state_ui: SelectionHistoryUi,
}

impl SelectionPanel {
    pub fn show_panel(
        &mut self,
        viewport_state: &mut ViewportState,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        blueprint: &mut Blueprint,
    ) {
        let screen_width = ui.ctx().screen_rect().width();

        let panel = egui::SidePanel::right("selection_view")
            .min_width(120.0)
            .default_width((0.45 * screen_width).min(250.0).round())
            .max_width((0.65 * screen_width).round())
            .resizable(true)
            .frame(egui::Frame {
                fill: ui.style().visuals.panel_fill,
                ..Default::default()
            });

        panel.show_animated_inside(
            ui,
            blueprint.selection_panel_expanded,
            |ui: &mut egui::Ui| {
                egui::TopBottomPanel::top("selection_panel_title_bar")
                    .exact_height(re_ui::ReUi::title_bar_height())
                    .frame(egui::Frame {
                        inner_margin: egui::Margin::symmetric(re_ui::ReUi::view_padding(), 0.0),
                        ..Default::default()
                    })
                    .show_inside(ui, |ui| {
                        if let Some(selection) = self.selection_state_ui.selection_ui(
                            ctx.re_ui,
                            ui,
                            blueprint,
                            &mut ctx.selection_state_mut().history,
                        ) {
                            ctx.selection_state_mut()
                                .set_selection(selection.iter().cloned());
                        }
                    });

                egui::ScrollArea::both()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        egui::Frame {
                            inner_margin: egui::Margin::same(re_ui::ReUi::view_padding()),
                            ..Default::default()
                        }
                        .show(ui, |ui| {
                            self.contents(viewport_state, ctx, ui, blueprint);
                        });
                    });
            },
        );
    }

    #[allow(clippy::unused_self)]
    fn contents(
        &mut self,
        viewport_state: &mut ViewportState,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        blueprint: &mut Blueprint,
    ) {
        re_tracing::profile_function!();

        let query = ctx.current_query();

        if ctx.selection().is_empty() {
            return;
        }

        let num_selections = ctx.selection().len();
        let selection = ctx.selection().to_vec();
        for (i, item) in selection.iter().enumerate() {
            ui.push_id(i, |ui| {
                what_is_selected_ui(ui, ctx, &mut blueprint.viewport, item);

                if has_data_section(item) {
                    ctx.re_ui.large_collapsing_header(ui, "Data", true, |ui| {
                        item.data_ui(ctx, ui, UiVerbosity::All, &query);
                    });
                }

                ctx.re_ui
                    .large_collapsing_header(ui, "Blueprint", true, |ui| {
                        blueprint_ui(viewport_state, ui, ctx, blueprint, item);
                    });

                if i + 1 < num_selections {
                    // Add space some space between selections
                    ui.add(egui::Separator::default().spacing(24.0).grow(20.0));
                }
            });
        }
    }
}

fn has_data_section(item: &Item) -> bool {
    match item {
        Item::ComponentPath(_) | Item::InstancePath(_, _) => true,
        // Skip data ui since we don't know yet what to show for these.
        Item::SpaceView(_) | Item::DataBlueprintGroup(_, _) => false,
    }
}

/// What is selected? Not the contents, just the short id of it.
fn what_is_selected_ui(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    viewport: &mut Viewport,
    item: &Item,
) {
    match item {
        Item::ComponentPath(re_log_types::ComponentPath {
            entity_path,
            component_name,
        }) => {
            egui::Grid::new("component_path")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Entity:");
                    item_ui::entity_path_button(ctx, ui, None, entity_path);
                    ui.end_row();

                    ui.label("Component:");
                    ui.label(component_name.short_name())
                        .on_hover_text(component_name.full_name());
                    ui.end_row();
                });
        }
        Item::SpaceView(space_view_id) => {
            if let Some(space_view) = viewport.space_view_mut(space_view_id) {
                ui.horizontal(|ui| {
                    ui.label("Space view:");
                    ui.text_edit_singleline(&mut space_view.display_name);
                });
            }
        }
        Item::InstancePath(space_view_id, instance_path) => {
            egui::Grid::new("space_view_id_entity_path").show(ui, |ui| {
                if instance_path.instance_key.is_splat() {
                    ui.label("Entity:");
                } else {
                    ui.label("Entity instance:");
                }
                item_ui::instance_path_button(ctx, ui, *space_view_id, instance_path);
                ui.end_row();

                if let Some(space_view_id) = space_view_id {
                    if let Some(space_view) = viewport.space_view_mut(space_view_id) {
                        ui.label("in Space View:");
                        re_viewport::item_ui::space_view_button(ctx, ui, space_view);
                        ui.end_row();
                    }
                }
            });
        }
        Item::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
            if let Some(space_view) = viewport.space_view_mut(space_view_id) {
                if let Some(group) = space_view
                    .data_blueprint
                    .group_mut(*data_blueprint_group_handle)
                {
                    egui::Grid::new("data_blueprint_group")
                        .num_columns(2)
                        .show(ui, |ui| {
                            ui.label("Data Group:");
                            item_ui::data_blueprint_group_button_to(
                                ctx,
                                ui,
                                group.display_name.clone(),
                                space_view.id,
                                *data_blueprint_group_handle,
                            );
                            ui.end_row();

                            ui.label("in Space View:");
                            re_viewport::item_ui::space_view_button_to(
                                ctx,
                                ui,
                                space_view.display_name.clone(),
                                space_view.id,
                                space_view.category,
                            );
                            ui.end_row();
                        });
                }
            }
        }
    }
}

/// What is the blueprint stuff for this item?
fn blueprint_ui(
    viewport_state: &mut ViewportState,
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    blueprint: &mut Blueprint,
    item: &Item,
) {
    match item {
        Item::ComponentPath(component_path) => {
            list_existing_data_blueprints(ui, ctx, component_path.entity_path(), blueprint);
        }

        Item::SpaceView(space_view_id) => {
            ui.horizontal(|ui| {
                if ui
                    .button("Add/remove entities")
                    .on_hover_text("Manually add or remove entities from the Space View.")
                    .clicked()
                {
                    viewport_state
                        .show_add_remove_entities_window(*space_view_id);
                }

                if ui
                    .button("Clone view")
                    .on_hover_text("Create an exact duplicate of this Space View including all blueprint settings")
                    .clicked()
                {
                    if let Some(space_view) = blueprint.viewport.space_view(space_view_id) {
                        let mut new_space_view = space_view.clone();
                        new_space_view.id = SpaceViewId::random();
                        blueprint.viewport.add_space_view(new_space_view);
                        blueprint.viewport.mark_user_interaction();
                    }
                }
            });

            ui.add_space(ui.spacing().item_spacing.y);

            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                let space_view_state = viewport_state.space_view_state(
                    ctx.space_view_class_registry,
                    space_view.id,
                    space_view.class,
                );
                space_view.selection_ui(space_view_state, ctx, ui);
            }
        }

        Item::InstancePath(space_view_id, instance_path) => {
            if let Some(space_view_id) = space_view_id {
                if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                    if instance_path.instance_key.is_specific() {
                        ui.horizontal(|ui| {
                            ui.label("Part of");
                            item_ui::entity_path_button(
                                ctx,
                                ui,
                                Some(*space_view_id),
                                &instance_path.entity_path,
                            );
                        });
                        // TODO(emilk): show the values of this specific instance (e.g. point in the point cloud)!
                    } else {
                        // splat - the whole entity
                        let data_blueprint = space_view.data_blueprint.data_blueprints_individual();
                        let mut props = data_blueprint.get(&instance_path.entity_path);
                        entity_props_ui(ctx, ui, Some(&instance_path.entity_path), &mut props);
                        data_blueprint.set(instance_path.entity_path.clone(), props);
                    }
                }
            } else {
                list_existing_data_blueprints(ui, ctx, &instance_path.entity_path, blueprint);
            }
        }

        Item::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                if let Some(group) = space_view
                    .data_blueprint
                    .group_mut(*data_blueprint_group_handle)
                {
                    entity_props_ui(ctx, ui, None, &mut group.properties_individual);
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
                    item_ui::entity_path_button_to(
                        ctx,
                        ui,
                        Some(*space_view_id),
                        entity_path,
                        &space_view.display_name,
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
) {
    ui.checkbox(&mut entity_props.visible, "Visible");
    ui.checkbox(&mut entity_props.interactive, "Interactive")
        .on_hover_text("If disabled, the entity will not react to any mouse interaction");

    egui::Grid::new("entity_properties")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Visible history");
            let visible_history = &mut entity_props.visible_history;
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
                    .on_hover_text("Include this much history of the Entity in the Space View.");
                    visible_history.nanos = (time_sec * 1e9).round() as _;
                }
                TimeType::Sequence => {
                    let speed = (visible_history.sequences as f32 * 0.05).at_least(1.0);
                    ui.add(
                        egui::DragValue::new(&mut visible_history.sequences)
                            .clamp_range(0.0..=f32::INFINITY)
                            .speed(speed),
                    )
                    .on_hover_text("Include this much history of the Entity in the Space View.");
                }
            }
            ui.end_row();

            // TODO(wumpf): It would be nice to only show pinhole & depth properties in the context of a 3D view.
            // if *view_state.state_spatial.nav_mode.get() == SpatialNavigationMode::ThreeD {
            if let Some(entity_path) = entity_path {
                pinhole_props_ui(ctx, ui, entity_path, entity_props);
                depth_props_ui(ctx, ui, entity_path, entity_props);
            }
        });
}

fn colormap_props_ui(ui: &mut egui::Ui, entity_props: &mut EntityProperties) {
    let current = *entity_props.color_mapper.get();

    ui.label("Color map");
    egui::ComboBox::from_id_source("color_mapper")
        .selected_text(current.to_string())
        .show_ui(ui, |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(64.0);

            // TODO(cmc): that is not ideal but I don't want to import yet another proc-macro...
            let mut add_label = |proposed| {
                if ui
                    .selectable_label(current == proposed, proposed.to_string())
                    .clicked()
                {
                    entity_props.color_mapper = EditableAutoValue::Auto(proposed);
                }
            };

            add_label(ColorMapper::Colormap(Colormap::Grayscale));
            add_label(ColorMapper::Colormap(Colormap::Turbo));
            add_label(ColorMapper::Colormap(Colormap::Viridis));
            add_label(ColorMapper::Colormap(Colormap::Plasma));
            add_label(ColorMapper::Colormap(Colormap::Magma));
            add_label(ColorMapper::Colormap(Colormap::Inferno));
        });

    ui.end_row();
}

fn pinhole_props_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    entity_props: &mut EntityProperties,
) {
    let query = ctx.current_query();
    let store = &ctx.store_db.entity_db.data_store;
    if store
        .query_latest_component::<Pinhole>(entity_path, &query)
        .is_some()
    {
        ui.label("Image plane distance");
        let mut distance = *entity_props.pinhole_image_plane_distance.get();
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
            entity_props.pinhole_image_plane_distance = EditableAutoValue::UserEdited(distance);
        }
        ui.end_row();
    }
}

fn depth_props_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    entity_props: &mut EntityProperties,
) -> Option<()> {
    re_tracing::profile_function!();

    let query = ctx.current_query();
    let store = &ctx.store_db.entity_db.data_store;
    let tensor = store.query_latest_component::<Tensor>(entity_path, &query)?;
    if tensor.meaning != TensorDataMeaning::Depth {
        return Some(());
    }
    let pinhole_ent_path = ctx
        .store_db
        .entity_db
        .data_store
        .query_latest_component_at_closest_ancestor::<Pinhole>(entity_path, &query)?
        .0;

    let mut backproject_depth = *entity_props.backproject_depth.get();

    if ui
        .checkbox(&mut backproject_depth, "Backproject Depth")
        .on_hover_text(
            "If enabled, the depth texture will be backprojected into a point cloud rather \
                than simply displayed as an image.",
        )
        .changed()
    {
        entity_props.backproject_depth = EditableAutoValue::UserEdited(backproject_depth);
    }
    ui.end_row();

    if backproject_depth {
        ui.label("Pinhole");
        item_ui::entity_path_button(ctx, ui, None, &pinhole_ent_path).on_hover_text(
            "The entity path of the pinhole transform being used to do the backprojection.",
        );
        ui.end_row();

        depth_from_world_scale_ui(ui, &mut entity_props.depth_from_world_scale);

        backproject_radius_scale_ui(ui, &mut entity_props.backproject_radius_scale);

        // TODO(cmc): This should apply to the depth map entity as a whole, but for that we
        // need to get the current hardcoded colormapping out of the image cache first.
        colormap_props_ui(ui, entity_props);
    }

    Some(())
}

fn depth_from_world_scale_ui(ui: &mut egui::Ui, property: &mut EditableAutoValue<f32>) {
    ui.label("Backproject meter");
    let mut value = *property.get();
    let speed = (value * 0.05).at_least(0.01);
    let response = ui
    .add(
        egui::DragValue::new(&mut value)
            .clamp_range(0.0..=1.0e8)
            .speed(speed),
    )
    .on_hover_text("How many steps in the depth image correspond to one world-space unit. For instance, 1000 means millimeters.\n\
                    Double-click to reset.");
    if response.double_clicked() {
        // reset to auto - the exact value will be restored somewhere else
        *property = EditableAutoValue::Auto(value);
        response.surrender_focus();
    } else if response.changed() {
        *property = EditableAutoValue::UserEdited(value);
    }
    ui.end_row();
}

fn backproject_radius_scale_ui(ui: &mut egui::Ui, property: &mut EditableAutoValue<f32>) {
    ui.label("Backproject radius scale");
    let mut value = *property.get();
    let speed = (value * 0.01).at_least(0.001);
    let response = ui
        .add(
            egui::DragValue::new(&mut value)
                .clamp_range(0.0..=1.0e8)
                .speed(speed),
        )
        .on_hover_text(
            "Scales the radii of the points in the backprojected point cloud.\n\
            This is a factor of the projected pixel diameter. \
            This means a scale of 0.5 will leave adjacent pixels at the same depth value just touching.\n\
            Double-click to reset.",
        );
    if response.double_clicked() {
        *property = EditableAutoValue::Auto(2.0);
        response.surrender_focus();
    } else if response.changed() {
        *property = EditableAutoValue::UserEdited(value);
    }
    ui.end_row();
}
