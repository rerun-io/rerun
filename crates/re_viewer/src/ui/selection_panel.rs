use egui::NumExt as _;

use re_data_store::{
    ColorMapper, Colormap, EditableAutoValue, EntityPath, EntityProperties, VisibleHistory,
};
use re_data_ui::{image_meaning_for_entity, item_ui, DataUi};
use re_log_types::{DataRow, RowId, TimePoint};
use re_space_view_time_series::TimeSeriesSpaceView;
use re_types::{
    components::{PinholeProjection, Transform3D},
    tensor_data::TensorDataMeaning,
};
use re_ui::list_item::ListItem;
use re_ui::ReUi;
use re_viewer_context::{
    gpu_bridge::colormap_dropdown_button_ui, Item, SpaceViewClass, SpaceViewClassName, SpaceViewId,
    SystemCommand, SystemCommandSender as _, UiVerbosity, ViewerContext,
};
use re_viewport::{external::re_space_view::QueryExpressions, Viewport, ViewportBlueprint};

use crate::ui::visible_history::visible_history_ui;

use super::selection_history_ui::SelectionHistoryUi;

// ---

/// The "Selection View" sidebar.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct SelectionPanel {
    selection_state_ui: SelectionHistoryUi,
}

impl SelectionPanel {
    pub fn show_panel(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        viewport: &mut Viewport<'_, '_>,
        expanded: bool,
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

        // Always reset the VH highlight, and let the UI re-set it if needed.
        ctx.rec_cfg.time_ctrl.write().highlighted_range = None;

        panel.show_animated_inside(ui, expanded, |ui: &mut egui::Ui| {
            // Set the clip rectangle to the panel for the benefit of nested, "full span" widgets
            // like large collapsing headers. Here, no need to extend `ui.max_rect()` as the
            // enclosing frame doesn't have inner margins.
            ui.set_clip_rect(ui.max_rect());

            ctx.re_ui.panel_content(ui, |_, ui| {
                let hover = "The Selection View contains information and options about the \
                    currently selected object(s)";
                ctx.re_ui
                    .panel_title_bar_with_buttons(ui, "Selection", Some(hover), |ui| {
                        let mut history = ctx.selection_state().history.lock();
                        if let Some(selection) = self.selection_state_ui.selection_ui(
                            ctx.re_ui,
                            ui,
                            &viewport.blueprint,
                            &mut history,
                        ) {
                            ctx.selection_state()
                                .set_selection(selection.iter().cloned());
                        }
                    });
            });

            // move the vertical spacing between the title and the content to _inside_ the scroll
            // area
            ui.add_space(-ui.spacing().item_spacing.y);

            egui::ScrollArea::both()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.add_space(ui.spacing().item_spacing.y);
                    ctx.re_ui.panel_content(ui, |_, ui| {
                        self.contents(ctx, ui, viewport);
                    });
                });
        });
    }

    #[allow(clippy::unused_self)]
    fn contents(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        viewport: &mut Viewport<'_, '_>,
    ) {
        re_tracing::profile_function!();

        let query = ctx.current_query();

        if ctx.selection().is_empty() {
            return;
        }

        // no gap before the first item title
        ui.add_space(-ui.spacing().item_spacing.y);

        let selection = ctx.selection().to_vec();
        for (i, item) in selection.iter().enumerate() {
            ui.push_id(i, |ui| {
                what_is_selected_ui(ui, ctx, &mut viewport.blueprint, item);

                if let Item::SpaceView(space_view_id) = item {
                    space_view_top_level_properties(
                        ui,
                        ctx,
                        &mut viewport.blueprint,
                        space_view_id,
                    );
                }

                if has_data_section(item) {
                    ctx.re_ui.large_collapsing_header(ui, "Data", true, |ui| {
                        item.data_ui(ctx, ui, UiVerbosity::All, &query);
                    });
                }

                if has_blueprint_section(item) {
                    ctx.re_ui
                        .large_collapsing_header(ui, "Blueprint", true, |ui| {
                            blueprint_ui(ui, ctx, viewport, item);
                        });
                }

                if i < selection.len() - 1 {
                    // Add space some space between selections
                    ui.add_space(8.);
                }
            });
        }
    }
}

fn has_data_section(item: &Item) -> bool {
    match item {
        Item::ComponentPath(_) | Item::InstancePath(_, _) => true,
        // Skip data ui since we don't know yet what to show for these.
        Item::SpaceView(_) | Item::DataBlueprintGroup(_, _, _) => false,
    }
}

fn space_view_button(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view: &re_viewport::SpaceViewBlueprint,
) -> egui::Response {
    let item = Item::SpaceView(space_view.id);
    let is_selected = ctx.selection().contains(&item);

    let response = ctx
        .re_ui
        .selectable_label_with_icon(
            ui,
            space_view.class(ctx.space_view_class_registry).icon(),
            space_view.display_name.clone(),
            is_selected,
        )
        .on_hover_text("Space View");
    item_ui::cursor_interact_with_selectable(ctx, response, item)
}

/// What is selected and where is it located?
///
/// This includes a title bar and contextual information about there this item is located.
fn what_is_selected_ui(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    viewport: &mut ViewportBlueprint<'_>,
    item: &Item,
) {
    match item {
        Item::ComponentPath(re_log_types::ComponentPath {
            entity_path,
            component_name,
        }) => {
            item_title_ui(
                ctx.re_ui,
                ui,
                component_name.short_name(),
                None,
                &format!(
                    "Component {} of entity '{}'",
                    component_name.full_name(),
                    entity_path
                ),
            );

            ui.horizontal(|ui| {
                ui.label("component of");
                item_ui::entity_path_button(ctx, ui, None, entity_path);
            });

            list_existing_data_blueprints(ui, ctx, entity_path, viewport);
        }
        Item::SpaceView(space_view_id) => {
            if let Some(space_view) = viewport.space_view_mut(space_view_id) {
                let space_view_class = space_view.class(ctx.space_view_class_registry);
                item_title_ui(
                    ctx.re_ui,
                    ui,
                    &space_view.display_name,
                    Some(space_view_class.icon()),
                    &format!(
                        "Space View {:?} of type {}",
                        space_view.display_name,
                        space_view_class.name(),
                    ),
                );
            }
        }
        Item::InstancePath(space_view_id, instance_path) => {
            let typ = if instance_path.instance_key.is_splat() {
                "Entity"
            } else {
                "Entity instance"
            };

            if let Some(space_view_id) = space_view_id {
                if let Some(space_view) = viewport.space_view_mut(space_view_id) {
                    item_title_ui(
                        ctx.re_ui,
                        ui,
                        instance_path.to_string().as_str(),
                        None,
                        &format!(
                            "{typ} '{instance_path}' as shown in Space View {:?}",
                            space_view.display_name
                        ),
                    );

                    ui.horizontal(|ui| {
                        ui.label("in");
                        space_view_button(ctx, ui, space_view);
                    });
                }
            } else {
                item_title_ui(
                    ctx.re_ui,
                    ui,
                    instance_path.to_string().as_str(),
                    None,
                    &format!("{typ} '{instance_path}'"),
                );

                list_existing_data_blueprints(ui, ctx, &instance_path.entity_path, viewport);
            }
        }
        Item::DataBlueprintGroup(space_view_id, _query_id, entity_path) => {
            if let Some(space_view) = viewport.space_view(space_view_id) {
                item_title_ui(
                    ctx.re_ui,
                    ui,
                    &entity_path.to_string(),
                    Some(&re_ui::icons::CONTAINER),
                    &format!(
                        "Group {:?} as shown in Space View {:?}",
                        entity_path, space_view.display_name
                    ),
                );

                ui.horizontal(|ui| {
                    ui.label("in");
                    space_view_button(ctx, ui, space_view);
                });
            }
        }
    }
}

/// A title bar for an item.
fn item_title_ui(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    name: &str,
    icon: Option<&re_ui::Icon>,
    hover: &str,
) -> egui::Response {
    let mut list_item = ListItem::new(re_ui, name)
        .with_height(ReUi::title_bar_height())
        .selected(true);

    if let Some(icon) = icon {
        list_item = list_item.with_icon(icon);
    }

    list_item.show(ui).on_hover_text(hover)
}

/// Display a list of all the space views an entity appears in.
fn list_existing_data_blueprints(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    entity_path: &EntityPath,
    blueprint: &ViewportBlueprint<'_>,
) {
    let space_views_with_path = blueprint.space_views_containing_entity_path(ctx, entity_path);

    if space_views_with_path.is_empty() {
        ui.weak("(Not shown in any Space View)");
    } else {
        for space_view_id in &space_views_with_path {
            if let Some(space_view) = blueprint.space_view(space_view_id) {
                ui.horizontal(|ui| {
                    item_ui::entity_path_button_to(
                        ctx,
                        ui,
                        Some(*space_view_id),
                        entity_path,
                        "Shown",
                    );
                    ui.label("in");
                    space_view_button(ctx, ui, space_view);
                });
            }
        }
    }
}

/// Display the top-level properties of a space view.
///
/// This includes the name, space origin entity, and space view type. These properties are singled
/// out as needing to be edited in most case when creating a new Space View, which is why they are
/// shown at the very top.
fn space_view_top_level_properties(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    viewport: &mut ViewportBlueprint<'_>,
    space_view_id: &SpaceViewId,
) {
    if let Some(space_view) = viewport.space_view_mut(space_view_id) {
        egui::Grid::new("space_view_top_level_properties")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Name").on_hover_text(
                    "The name of the Space View used for display purposes. This can be any text \
                    string.",
                );
                ui.text_edit_singleline(&mut space_view.display_name);
                ui.end_row();

                ui.label("Space origin").on_hover_text(
                    "The origin Entity for this Space View. For spatial Space Views, the Space \
                    View's origin is the same as this Entity's origin and all transforms are \
                    relative to it.",
                );
                item_ui::entity_path_button(
                    ctx,
                    ui,
                    Some(*space_view_id),
                    &space_view.space_origin,
                );
                ui.end_row();

                ui.label("Type")
                    .on_hover_text("The type of this Space View");
                ui.label(&*space_view.class(ctx.space_view_class_registry).name());
                ui.end_row();
            });
    }
}

fn has_blueprint_section(item: &Item) -> bool {
    match item {
        Item::ComponentPath(_) => false,
        Item::InstancePath(space_view_id, _) => space_view_id.is_some(),
        _ => true,
    }
}

/// What is the blueprint stuff for this item?
fn blueprint_ui(
    ui: &mut egui::Ui,
    ctx: &mut ViewerContext<'_>,
    viewport: &mut Viewport<'_, '_>,
    item: &Item,
) {
    match item {
        Item::SpaceView(space_view_id) => {
            ui.horizontal(|ui| {
                // TODO(#4377): Don't bother showing add/remove entities dialog since it's broken
                /*
                if ui
                    .button("Add/remove Entities")
                    .on_hover_text("Manually add or remove Entities from the Space View")
                    .clicked()
                {
                    viewport
                        .show_add_remove_entities_window(*space_view_id);
                }
                */

                if ui
                    .button("Clone Space View")
                    .on_hover_text("Create an exact duplicate of this Space View including all Blueprint settings")
                    .clicked()
                {
                    if let Some(space_view) = viewport.blueprint.space_view(space_view_id) {
                        let mut new_space_view = space_view.clone();
                        new_space_view.id = SpaceViewId::random();
                        viewport.blueprint.add_space_view(new_space_view);
                        viewport.blueprint.mark_user_interaction();
                    }
                }
            });

            if let Some(space_view) = viewport.blueprint.space_view_mut(space_view_id) {
                if let Some(query) = space_view.queries.first() {
                    let inclusions = query.expressions.inclusions.join("\n");
                    let mut edited_inclusions = inclusions.clone();
                    let exclusions = query.expressions.exclusions.join("\n");
                    let mut edited_exclusions = exclusions.clone();

                    ui.label("Inclusion expressions");
                    ui.text_edit_multiline(&mut edited_inclusions);
                    ui.label("Exclusion expressions");
                    ui.text_edit_multiline(&mut edited_exclusions);

                    if edited_inclusions != inclusions || edited_exclusions != exclusions {
                        let timepoint = TimePoint::timeless();

                        let expressions_component = QueryExpressions {
                            inclusions: edited_inclusions.split('\n').map(|s| s.into()).collect(),
                            exclusions: edited_exclusions.split('\n').map(|s| s.into()).collect(),
                        };

                        let row = DataRow::from_cells1_sized(
                            RowId::random(),
                            query.id.as_entity_path(),
                            timepoint.clone(),
                            1,
                            [expressions_component],
                        )
                        .unwrap();

                        ctx.command_sender
                            .send_system(SystemCommand::UpdateBlueprint(
                                ctx.store_context.blueprint.store_id().clone(),
                                vec![row],
                            ));

                        space_view.entities_determined_by_user = true;
                    }
                }
            }

            ui.add_space(ui.spacing().item_spacing.y);

            if let Some(space_view) = viewport.blueprint.space_view_mut(space_view_id) {
                let space_view_class = *space_view.class_name();
                let space_view_state = viewport.state.space_view_state_mut(
                    ctx.space_view_class_registry,
                    space_view.id,
                    space_view.class_name(),
                );

                // Space View don't inherit properties.
                let mut resolved_entity_props = EntityProperties::default();

                // TODO(#4194): it should be the responsibility of the space view to provide defaults for entity props
                if space_view_class == TimeSeriesSpaceView::NAME {
                    resolved_entity_props.visible_history.sequences = VisibleHistory::ALL;
                    resolved_entity_props.visible_history.nanos = VisibleHistory::ALL;
                }

                let root_data_result = space_view.root_data_result(ctx.store_context);
                let mut props = root_data_result
                    .individual_properties
                    .clone()
                    .unwrap_or(resolved_entity_props.clone());

                let cursor = ui.cursor();

                space_view
                    .class(ctx.space_view_class_registry)
                    .selection_ui(
                        ctx,
                        ui,
                        space_view_state,
                        &space_view.space_origin,
                        space_view.id,
                        &mut props,
                    );

                if cursor != ui.cursor() {
                    // add some space if something was rendered by selection_ui
                    //TODO(ab): use design token
                    ui.add_space(16.0);
                }

                visible_history_ui(
                    ctx,
                    ui,
                    &space_view_class,
                    true,
                    None,
                    &mut props.visible_history,
                    &resolved_entity_props.visible_history,
                );

                root_data_result.save_override(Some(props), ctx);
            }
        }

        Item::InstancePath(space_view_id, instance_path) => {
            if let Some(space_view_id) = space_view_id {
                if let Some(space_view) = viewport.blueprint.space_view_mut(space_view_id) {
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
                        let space_view_class = *space_view.class_name();
                        let entity_path = &instance_path.entity_path;
                        let as_group = false;

                        let query_result = ctx.lookup_query_result(space_view.query_id());
                        if let Some(data_result) = query_result
                            .tree
                            .lookup_result_by_path_and_group(entity_path, as_group)
                            .cloned()
                        {
                            let mut props = data_result
                                .individual_properties
                                .clone()
                                .unwrap_or_default();
                            entity_props_ui(
                                ctx,
                                ui,
                                &space_view_class,
                                Some(entity_path),
                                &mut props,
                                &data_result.resolved_properties,
                            );
                            data_result.save_override(Some(props), ctx);
                        }
                    }
                }
            }
        }

        Item::DataBlueprintGroup(space_view_id, query_id, group_path) => {
            if let Some(space_view) = viewport.blueprint.space_view_mut(space_view_id) {
                let as_group = true;

                let query_result = ctx.lookup_query_result(*query_id);
                if let Some(data_result) = query_result
                    .tree
                    .lookup_result_by_path_and_group(group_path, as_group)
                    .cloned()
                {
                    let space_view_class = *space_view.class_name();
                    let mut props = data_result
                        .individual_properties
                        .clone()
                        .unwrap_or_default();

                    entity_props_ui(
                        ctx,
                        ui,
                        &space_view_class,
                        None,
                        &mut props,
                        &data_result.resolved_properties,
                    );
                    data_result.save_override(Some(props), ctx);
                }
            } else {
                ctx.selection_state().clear_current();
            }
        }

        Item::ComponentPath(_) => {}
    }
}

fn entity_props_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_class: &SpaceViewClassName,
    entity_path: Option<&EntityPath>,
    entity_props: &mut EntityProperties,
    resolved_entity_props: &EntityProperties,
) {
    let re_ui = ctx.re_ui;
    re_ui.checkbox(ui, &mut entity_props.visible, "Visible");
    re_ui
        .checkbox(ui, &mut entity_props.interactive, "Interactive")
        .on_hover_text("If disabled, the entity will not react to any mouse interaction");

    visible_history_ui(
        ctx,
        ui,
        space_view_class,
        false,
        entity_path,
        &mut entity_props.visible_history,
        &resolved_entity_props.visible_history,
    );

    egui::Grid::new("entity_properties")
        .num_columns(2)
        .show(ui, |ui| {
            // TODO(wumpf): It would be nice to only show pinhole & depth properties in the context of a 3D view.
            // if *view_state.state_spatial.nav_mode.get() == SpatialNavigationMode::ThreeD {
            if let Some(entity_path) = entity_path {
                pinhole_props_ui(ctx, ui, entity_path, entity_props);
                depth_props_ui(ctx, ui, entity_path, entity_props);
                transform3d_visualization_ui(ctx, ui, entity_path, entity_props);
            }
        });
}

fn colormap_props_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_props: &mut EntityProperties,
) {
    let mut re_renderer_colormap = match *entity_props.color_mapper.get() {
        ColorMapper::Colormap(Colormap::Grayscale) => re_renderer::Colormap::Grayscale,
        ColorMapper::Colormap(Colormap::Turbo) => re_renderer::Colormap::Turbo,
        ColorMapper::Colormap(Colormap::Viridis) => re_renderer::Colormap::Viridis,
        ColorMapper::Colormap(Colormap::Plasma) => re_renderer::Colormap::Plasma,
        ColorMapper::Colormap(Colormap::Magma) => re_renderer::Colormap::Magma,
        ColorMapper::Colormap(Colormap::Inferno) => re_renderer::Colormap::Inferno,
    };

    ui.label("Color map");
    colormap_dropdown_button_ui(ctx.render_ctx, ui, &mut re_renderer_colormap);

    let new_colormap = match re_renderer_colormap {
        re_renderer::Colormap::Grayscale => Colormap::Grayscale,
        re_renderer::Colormap::Turbo => Colormap::Turbo,
        re_renderer::Colormap::Viridis => Colormap::Viridis,
        re_renderer::Colormap::Plasma => Colormap::Plasma,
        re_renderer::Colormap::Magma => Colormap::Magma,
        re_renderer::Colormap::Inferno => Colormap::Inferno,
    };
    entity_props.color_mapper = EditableAutoValue::UserEdited(ColorMapper::Colormap(new_colormap));

    ui.end_row();
}

fn pinhole_props_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    entity_props: &mut EntityProperties,
) {
    let query = ctx.current_query();
    let store = ctx.store_db.store();
    if store
        .query_latest_component::<PinholeProjection>(entity_path, &query)
        .is_some()
    {
        ui.label("Image plane distance");
        let mut distance = *entity_props.pinhole_image_plane_distance;
        let speed = (distance * 0.05).at_least(0.01);
        if ui
            .add(
                egui::DragValue::new(&mut distance)
                    .clamp_range(0.0..=1.0e8)
                    .speed(speed),
            )
            .on_hover_text("Controls how far away the image plane is")
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
    let store = ctx.store_db.store();

    let meaning = image_meaning_for_entity(entity_path, ctx);

    if meaning != TensorDataMeaning::Depth {
        return Some(());
    }
    let image_projection_ent_path = store
        .query_latest_component_at_closest_ancestor::<PinholeProjection>(entity_path, &query)?
        .0;

    let mut backproject_depth = *entity_props.backproject_depth;

    if ctx
        .re_ui
        .checkbox(ui, &mut backproject_depth, "Backproject Depth")
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
        item_ui::entity_path_button(ctx, ui, None, &image_projection_ent_path).on_hover_text(
            "The entity path of the pinhole transform being used to do the backprojection.",
        );
        ui.end_row();

        depth_from_world_scale_ui(ui, &mut entity_props.depth_from_world_scale);

        backproject_radius_scale_ui(ui, &mut entity_props.backproject_radius_scale);

        // TODO(cmc): This should apply to the depth map entity as a whole, but for that we
        // need to get the current hardcoded colormapping out of the image cache first.
        colormap_props_ui(ctx, ui, entity_props);
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

fn transform3d_visualization_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_path: &EntityPath,
    entity_props: &mut EntityProperties,
) {
    re_tracing::profile_function!();

    let query = ctx.current_query();
    if ctx
        .store_db
        .store()
        .query_latest_component::<Transform3D>(entity_path, &query)
        .is_none()
    {
        return;
    }

    let show_arrows = &mut entity_props.transform_3d_visible;
    let arrow_length = &mut entity_props.transform_3d_size;

    {
        let mut checked = *show_arrows.get();
        let response = ctx.re_ui.checkbox(ui, &mut checked, "Show transform").on_hover_text(
            "Enables/disables the display of three arrows to visualize the (accumulated) transform at this entity. Red/green/blue show the x/y/z axis respectively.");
        if response.changed() {
            *show_arrows = EditableAutoValue::UserEdited(checked);
        }
        if response.double_clicked() {
            *show_arrows = EditableAutoValue::Auto(checked);
        }
    }

    if *show_arrows.get() {
        ui.end_row();
        ui.label("Transform-arrow length");
        let mut value = *arrow_length.get();
        let speed = (value * 0.05).at_least(0.001);
        let response = ui
            .add(
                egui::DragValue::new(&mut value)
                    .clamp_range(0.0..=1.0e8)
                    .speed(speed),
            )
            .on_hover_text(
                "How long the arrows should be in the entity's own coordinate system. Double-click to reset to auto.",
            );
        if response.double_clicked() {
            // reset to auto - the exact value will be restored somewhere else
            *arrow_length = EditableAutoValue::Auto(value);
            response.surrender_focus();
        } else if response.changed() {
            *arrow_length = EditableAutoValue::UserEdited(value);
        }
    }

    ui.end_row();
}
