use re_data_store::{query_latest_single, EditableAutoValue, EntityPath, EntityProperties};
use re_log_types::{TimeType, Transform};

use crate::{
    ui::{view_spatial::SpatialNavigationMode, Blueprint},
    Item, UiVerbosity, ViewerContext,
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
        ui: &mut egui::Ui,
        blueprint: &mut Blueprint,
    ) {
        let panel = egui::SidePanel::right("selection_view")
            .min_width(120.0)
            .default_width(250.0)
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
                        if let Some(selection) = ctx
                            .rec_cfg
                            .selection_state
                            .selection_ui(ctx.re_ui, ui, blueprint)
                        {
                            ctx.set_multi_selection(selection.iter().cloned());
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
                            self.contents(ui, ctx, blueprint);
                        });
                    });
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

        if ctx.selection().is_empty() {
            return;
        }

        let num_selections = ctx.selection().len();
        let selection = ctx.selection().to_vec();
        for (i, selection) in selection.iter().enumerate() {
            ui.push_id(i, |ui| {
                what_is_selected_ui(ui, ctx, blueprint, selection);

                if has_data_section(selection) {
                    ctx.re_ui.large_collapsing_header(ui, "Data", true, |ui| {
                        selection.data_ui(ctx, ui, UiVerbosity::All, &query);
                    });
                }

                ctx.re_ui
                    .large_collapsing_header(ui, "Blueprint", true, |ui| {
                        blueprint_ui(ui, ctx, blueprint, selection);
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
        Item::MsgId(_) | Item::ComponentPath(_) | Item::InstancePath(_, _) => true,
        // Skip data ui since we don't know yet what to show for these.
        Item::SpaceView(_) | Item::DataBlueprintGroup(_, _) => false,
    }
}

/// What is selected? Not the contents, just the short id of it.
pub fn what_is_selected_ui(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    blueprint: &mut Blueprint,
    item: &Item,
) {
    match item {
        Item::MsgId(msg_id) => {
            ui.horizontal(|ui| {
                ui.label("Message ID:");
                ctx.msg_id_button(ui, *msg_id);
            });
        }
        Item::ComponentPath(re_log_types::ComponentPath {
            entity_path,
            component_name,
        }) => {
            egui::Grid::new("component_path")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Entity:");
                    ctx.entity_path_button(ui, None, entity_path);
                    ui.end_row();

                    ui.label("Component:");
                    ui.label(component_name.short_name())
                        .on_hover_text(component_name.full_name());
                    ui.end_row();
                });
        }
        Item::SpaceView(space_view_id) => {
            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
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
                ctx.instance_path_button(ui, *space_view_id, instance_path);
                ui.end_row();

                if let Some(space_view_id) = space_view_id {
                    if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                        ui.label("in Space View:");
                        ctx.space_view_button(ui, space_view);
                        ui.end_row();
                    }
                }
            });
        }
        Item::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                if let Some(group) = space_view
                    .data_blueprint
                    .group_mut(*data_blueprint_group_handle)
                {
                    egui::Grid::new("data_blueprint_group")
                        .num_columns(2)
                        .show(ui, |ui| {
                            ui.label("Data Group:");
                            ctx.data_blueprint_group_button_to(
                                ui,
                                group.display_name.clone(),
                                space_view.id,
                                *data_blueprint_group_handle,
                            );
                            ui.end_row();

                            ui.label("in Space View:");
                            ctx.space_view_button_to(
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

impl DataUi for Item {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match self {
            Item::SpaceView(_) | Item::DataBlueprintGroup(_, _) => {
                // Shouldn't be reachable since SelectionPanel::contents doesn't show data ui for these.
                // If you add something in here make sure to adjust SelectionPanel::contents accordingly.
                debug_assert!(!has_data_section(self));
            }
            Item::MsgId(msg_id) => {
                msg_id.data_ui(ctx, ui, verbosity, query);
            }
            Item::ComponentPath(component_path) => {
                component_path.data_ui(ctx, ui, verbosity, query);
            }
            Item::InstancePath(_, instance_path) => {
                instance_path.data_ui(ctx, ui, verbosity, query);
            }
        }
    }
}

/// What is the blueprint stuff for this item?
fn blueprint_ui(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    blueprint: &mut Blueprint,
    item: &Item,
) {
    match item {
        Item::MsgId(_) => {
            // TODO(andreas): Show space views that contains entities that's part of this message.
            ui.weak("(nothing)");
        }

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
                    blueprint
                        .viewport
                        .show_add_remove_entities_window(*space_view_id);
                }

                if ui
                    .button("Clone view")
                    .on_hover_text("Create an exact duplicate of this Space View including all blueprint settings")
                    .clicked()
                {
                    if let Some(space_view) = blueprint.viewport.space_view(space_view_id) {
                        let mut new_space_view = space_view.clone();
                        new_space_view.id = super::SpaceViewId::random();
                        blueprint.viewport.add_space_view(new_space_view);
                        blueprint.viewport.mark_user_interaction();
                    }
                }
            });

            ui.add_space(ui.spacing().item_spacing.y);

            if let Some(space_view) = blueprint.viewport.space_view_mut(space_view_id) {
                space_view.selection_ui(ctx, ui);
            }
        }

        Item::InstancePath(space_view_id, instance_path) => {
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

        Item::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
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
    view_state: &ViewState,
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

            color_mapping_props_ui(ctx, ui, entity_props);

            if view_state.state_spatial.nav_mode == SpatialNavigationMode::ThreeD {
                if let Some(entity_path) = entity_path {
                    let query = ctx.current_query();
                    if let Some(re_log_types::Transform::Pinhole(_)) =
                        query_latest_single::<Transform>(&ctx.log_db.entity_db, entity_path, &query)
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
                            entity_props.pinhole_image_plane_distance =
                                EditableAutoValue::UserEdited(distance);
                        }
                        ui.end_row();
                    }
                }
            }
        });
}

// TODO: filter
fn color_mapping_props_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_props: &mut EntityProperties,
) {
    let current = entity_props.color_mapper;

    fn selectable_label(
        ui: &mut egui::Ui,
        props: &mut EntityProperties,
        current: ColorMapper,
        proposed: ColorMapper,
    ) {
        if ui
            .selectable_label(current == proposed, proposed.to_string())
            .clicked()
        {
            props.color_mapper = proposed;
        }
    }

    ui.label("Color mapping");
    egui::ComboBox::from_id_source("color_mapper_kind")
        .selected_text(current.to_string())
        .show_ui(ui, |ui| {
            ui.style_mut().wrap = Some(false);
            ui.set_min_width(64.0);

            // TODO: that sucks
            selectable_label(ui, entity_props, current, ColorMapper::None);
            selectable_label(
                ui,
                entity_props,
                current,
                ColorMapper::ColorMap(ColorMap::Turbo),
            );
            selectable_label(
                ui,
                entity_props,
                current,
                ColorMapper::ColorMap(ColorMap::Viridis),
            );
            selectable_label(
                ui,
                entity_props,
                current,
                ColorMapper::ColorMap(ColorMap::Plasma),
            );
            selectable_label(
                ui,
                entity_props,
                current,
                ColorMapper::ColorMap(ColorMap::Magma),
            );
            selectable_label(
                ui,
                entity_props,
                current,
                ColorMapper::ColorMap(ColorMap::Inferno),
            );
        });

    ui.end_row();
}

fn pinhole_props_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    entity_props: &mut EntityProperties,
) {
    // pinhole_image_plane_distance
    let query = ctx.current_query();
    if let Some(re_log_types::Transform::Pinhole(pinhole)) =
        query_latest_single::<Transform>(&ctx.log_db.entity_db, entity_path, &query)
    {
        ui.label("Image plane distance");
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
        ui.end_row();
    }
}

fn depth_props_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    entity_props: &mut EntityProperties,
) {
    let query = ctx.current_query();

    // Find closest pinhole transform, if any.
    let mut pinhole_ent_path = None;
    let mut cur_path = Some(entity_path.clone());
    while let Some(path) = cur_path {
        if let Some(re_log_types::Transform::Pinhole(_)) =
            query_latest_single::<Transform>(&ctx.log_db.entity_db, &path, &query)
        {
            pinhole_ent_path = Some(path);
            break;
        }
        cur_path = path.parent();
    }

    // Early out if there's no pinhole transform upwards in the tree.
    let Some(pinhole_ent_path) = pinhole_ent_path else { return; };

    entity_props.backproject_pinhole_ent_path = Some(pinhole_ent_path.clone());

    let tensor = query_latest_single::<Tensor>(&ctx.log_db.entity_db, entity_path, &query);
    if tensor.map(|t| t.meaning) == Some(TensorDataMeaning::Depth) {
        ui.checkbox(&mut entity_props.backproject_depth, "Backproject Depth")
            .on_hover_text(
                "If enabled, the depth texture will be backprojected into a point cloud rather \
                than simply displayed as an image.",
            );
        ui.end_row();

        if entity_props.backproject_depth {
            ui.label("Backproject scale");
            let mut scale = entity_props.backproject_scale.into_inner();
            let speed = (scale * 0.05).at_least(0.01);
            if ui
                .add(
                    egui::DragValue::new(&mut scale)
                        .clamp_range(0.0..=1.0e8)
                        .speed(speed),
                )
                .on_hover_text("Scales the the backprojected point cloud")
                .changed()
            {
                entity_props.backproject_scale = ordered_float::NotNan::new(scale).unwrap();
            }
            ui.end_row();

            ui.label("Backproject radius scale");
            let mut radius_scale = entity_props.backproject_radius_scale.into_inner();
            let speed = (radius_scale * 0.05).at_least(0.01);
            if ui
                .add(
                    egui::DragValue::new(&mut radius_scale)
                        .clamp_range(0.0..=1.0e8)
                        .speed(speed),
                )
                .on_hover_text("Scales the radii of the points in the backprojected point cloud")
                .changed()
            {
                entity_props.backproject_radius_scale =
                    ordered_float::NotNan::new(radius_scale).unwrap();
            }
            ui.end_row();

            ui.label("Pinhole");
            ctx.entity_path_button(ui, None, &pinhole_ent_path)
                .on_hover_text(
                    "The entity path of the pinhole transform being used to do the backprojection.",
                );
            ui.end_row();
        }
    }
}
