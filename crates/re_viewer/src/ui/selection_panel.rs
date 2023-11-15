use egui::NumExt as _;
use std::collections::HashSet;
use std::ops::RangeInclusive;

use re_data_store::{
    ColorMapper, Colormap, EditableAutoValue, EntityPath, EntityProperties, ExtraQueryHistory,
    VisibleHistoryBoundary,
};
use re_data_ui::{image_meaning_for_entity, item_ui, DataUi};
use re_log_types::TimeType;
use re_space_view_spatial::{SpatialSpaceView2D, SpatialSpaceView3D};
use re_space_view_time_series::TimeSeriesSpaceView;
use re_types::{
    components::{PinholeProjection, Transform3D},
    tensor_data::TensorDataMeaning,
};
use re_types_core::ComponentName;
use re_viewer_context::{
    gpu_bridge::colormap_dropdown_button_ui, Item, SpaceViewClassName, SpaceViewId, UiVerbosity,
    ViewerContext,
};
use re_viewport::{external::re_space_view::DataQuery as _, Viewport, ViewportBlueprint};

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

        panel.show_animated_inside(ui, expanded, |ui: &mut egui::Ui| {
            // Set the clip rectangle to the panel for the benefit of nested, "full span" widgets
            // like large collapsing headers. Here, no need to extend `ui.max_rect()` as the
            // enclosing frame doesn't have inner margins.
            ui.set_clip_rect(ui.max_rect());

            egui::Frame {
                inner_margin: re_ui::ReUi::panel_margin(),
                ..Default::default()
            }
            .show(ui, |ui| {
                let hover = "The Selection View contains information and options about the currently selected object(s)";
                ctx.re_ui
                    .panel_title_bar_with_buttons(ui, "Selection", Some(hover), |ui| {
                        if let Some(selection) = self.selection_state_ui.selection_ui(
                            ctx.re_ui,
                            ui,
                            &viewport.blueprint,
                            &mut ctx.selection_state_mut().history,
                        ) {
                            ctx.selection_state_mut()
                                .set_selection(selection.iter().cloned());
                        }
                    });

                egui::ScrollArea::both()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
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

        let num_selections = ctx.selection().len();
        let selection = ctx.selection().to_vec();
        for (i, item) in selection.iter().enumerate() {
            ui.push_id(i, |ui| {
                what_is_selected_ui(ui, ctx, &mut viewport.blueprint, item);

                if has_data_section(item) {
                    ctx.re_ui.large_collapsing_header(ui, "Data", true, |ui| {
                        item.data_ui(ctx, ui, UiVerbosity::All, &query);
                    });
                }

                ctx.re_ui
                    .large_collapsing_header(ui, "Blueprint", true, |ui| {
                        blueprint_ui(ui, ctx, viewport, item);
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

/// What is selected? Not the contents, just the short id of it.
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
            egui::Grid::new("component_path")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Entity");
                    item_ui::entity_path_button(ctx, ui, None, entity_path);
                    ui.end_row();

                    ui.label("Component");
                    ui.label(component_name.short_name())
                        .on_hover_text(component_name.full_name());
                    ui.end_row();
                });
        }
        Item::SpaceView(space_view_id) => {
            if let Some(space_view) = viewport.space_view_mut(space_view_id) {
                ui.horizontal(|ui| {
                    ui.label("Space View");
                    ui.text_edit_singleline(&mut space_view.display_name);
                });
            }
        }
        Item::InstancePath(space_view_id, instance_path) => {
            egui::Grid::new("space_view_id_entity_path").show(ui, |ui| {
                if instance_path.instance_key.is_splat() {
                    ui.label("Entity");
                } else {
                    ui.label("Entity instance");
                }
                item_ui::instance_path_button(ctx, ui, *space_view_id, instance_path);
                ui.end_row();

                if let Some(space_view_id) = space_view_id {
                    if let Some(space_view) = viewport.space_view_mut(space_view_id) {
                        ui.label("In Space View");
                        space_view_button(ctx, ui, space_view);
                        ui.end_row();
                    }
                }
            });
        }
        Item::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
            if let Some(space_view) = viewport.space_view(space_view_id) {
                if let Some(group) = space_view.contents.group(*data_blueprint_group_handle) {
                    egui::Grid::new("data_blueprint_group")
                        .num_columns(2)
                        .show(ui, |ui| {
                            ui.label("Data Group");
                            item_ui::data_blueprint_group_button_to(
                                ctx,
                                ui,
                                group.display_name.clone(),
                                space_view.id,
                                *data_blueprint_group_handle,
                            );
                            ui.end_row();

                            ui.label("In Space View");
                            space_view_button(ctx, ui, space_view);
                            ui.end_row();
                        });
                }
            }
        }
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
        Item::ComponentPath(component_path) => {
            list_existing_data_blueprints(
                ui,
                ctx,
                component_path.entity_path(),
                &viewport.blueprint,
            );
        }

        Item::SpaceView(space_view_id) => {
            ui.horizontal(|ui| {
                if ui
                    .button("Add/remove Entities")
                    .on_hover_text("Manually add or remove Entities from the Space View")
                    .clicked()
                {
                    viewport
                        .show_add_remove_entities_window(*space_view_id);
                }

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

            ui.add_space(ui.spacing().item_spacing.y);

            if let Some(space_view) = viewport.blueprint.space_view_mut(space_view_id) {
                let space_view_class = *space_view.class_name();
                let space_view_state = viewport.state.space_view_state_mut(
                    ctx.space_view_class_registry,
                    space_view.id,
                    space_view.class_name(),
                );
                let root_data_result = space_view.root_data_result(ctx);
                let mut props = root_data_result.resolved_properties.clone();

                space_view
                    .class(ctx.space_view_class_registry)
                    .selection_ui(
                        ctx,
                        ui,
                        space_view_state,
                        &space_view.space_origin,
                        space_view.id,
                    );

                visible_history_ui(ctx, ui, &space_view_class, None, &mut props.visible_history);

                root_data_result.save_override(props, ctx);
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
                        let space_view_class_name = *space_view.class_name();
                        let data_result = space_view.contents.resolve(
                            space_view,
                            ctx,
                            &instance_path.entity_path,
                        );
                        let mut props = data_result.resolved_properties.clone();
                        entity_props_ui(
                            ctx,
                            ui,
                            &space_view_class_name,
                            Some(&instance_path.entity_path),
                            &mut props,
                        );
                        data_result.save_override(props, ctx);
                    }
                }
            } else {
                list_existing_data_blueprints(
                    ui,
                    ctx,
                    &instance_path.entity_path,
                    &viewport.blueprint,
                );
            }
        }

        Item::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
            if let Some(space_view) = viewport.blueprint.space_view_mut(space_view_id) {
                if let Some(group) = space_view.contents.group(*data_blueprint_group_handle) {
                    let data_result =
                        space_view
                            .contents
                            .resolve(space_view, ctx, &group.group_path);
                    let space_view_class_name = *space_view.class_name();
                    let mut props = data_result.resolved_properties.clone();
                    entity_props_ui(ctx, ui, &space_view_class_name, None, &mut props);
                    data_result.save_override(props, ctx);
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
    blueprint: &ViewportBlueprint<'_>,
) {
    let space_views_with_path = blueprint.space_views_containing_entity_path(entity_path);

    if space_views_with_path.is_empty() {
        ui.weak("(Not shown in any Space View)");
        // TODO(andreas): Offer options for adding?
    } else {
        ui.label("Is shown in");

        ui.indent("list of data blueprints indent", |ui| {
            for space_view_id in &space_views_with_path {
                if let Some(space_view) = blueprint.space_view(space_view_id) {
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
    space_view_class: &SpaceViewClassName,
    entity_path: Option<&EntityPath>,
    entity_props: &mut EntityProperties,
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
        entity_path,
        &mut entity_props.visible_history,
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

/// These space views support the Visible History feature.
static VISIBLE_HISTORY_SUPPORTED_SPACE_VIEWS: once_cell::sync::Lazy<HashSet<SpaceViewClassName>> =
    once_cell::sync::Lazy::new(|| {
        [
            SpatialSpaceView3D::NAME,
            SpatialSpaceView2D::NAME,
            TimeSeriesSpaceView::NAME,
        ]
        .map(Into::into)
        .into()
    });

/// Entities containing one of these components support the Visible History feature.
static VISIBLE_HISTORY_SUPPORTED_COMPONENT_NAMES: once_cell::sync::Lazy<Vec<ComponentName>> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.HalfSizes2D",
            "rerun.components.HalfSizes3D",
            "rerun.components.LineStrip2D",
            "rerun.components.LineStrip3D",
            "rerun.components.Position2D",
            "rerun.components.Position3D",
            "rerun.components.Scalar",
            "rerun.components.TensorData",
            "rerun.components.Vector3D",
        ]
        .map(Into::into)
        .into()
    });

// TODO(#4145): This method is obviously unfortunate. It's a temporary solution until the ViewPart
// system is able to report its ability to handle the visible history feature.
fn should_display_visible_history(
    ctx: &mut ViewerContext<'_>,
    space_view_class: &SpaceViewClassName,
    entity_path: Option<&EntityPath>,
) -> bool {
    if !VISIBLE_HISTORY_SUPPORTED_SPACE_VIEWS.contains(space_view_class) {
        return false;
    }

    if let Some(entity_path) = entity_path {
        let store = ctx.store_db.store();
        let component_names = store.all_components(ctx.rec_cfg.time_ctrl.timeline(), entity_path);
        if let Some(component_names) = component_names {
            if !component_names
                .iter()
                .any(|name| VISIBLE_HISTORY_SUPPORTED_COMPONENT_NAMES.contains(name))
            {
                return false;
            }
        } else {
            return false;
        }
    }

    true
}

pub fn visible_history_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    space_view_class: &SpaceViewClassName,
    entity_path: Option<&EntityPath>,
    visible_history_prop: &mut ExtraQueryHistory,
) {
    if !should_display_visible_history(ctx, space_view_class, entity_path) {
        return;
    }

    let re_ui = ctx.re_ui;

    re_ui
        .checkbox(ui, &mut visible_history_prop.enabled, "Visible history")
        .on_hover_text(
            "Enable Visible History.\n\nBy default, only the last state before the \
            current time is shown. By activating Visible History, all data within a \
            time window is shown instead.",
        );

    let time_range = if let Some(times) = ctx
        .store_db
        .time_histogram(ctx.rec_cfg.time_ctrl.timeline())
    {
        times.min_key().unwrap_or_default()..=times.max_key().unwrap_or_default()
    } else {
        0..=0
    };

    let current_time = ctx
        .rec_cfg
        .time_ctrl
        .time_i64()
        .unwrap_or_default()
        .at_least(*time_range.start()); // accounts for timeless time (TimeInt::BEGINNING)

    let sequence_timeline = matches!(ctx.rec_cfg.time_ctrl.timeline().typ(), TimeType::Sequence);

    let visible_history = if sequence_timeline {
        &mut visible_history_prop.sequences
    } else {
        &mut visible_history_prop.nanos
    };

    let visible_history_time_range = visible_history.from(current_time.into()).as_i64()
        ..=visible_history.to(current_time.into()).as_i64();

    ui.add_enabled_ui(visible_history_prop.enabled, |ui| {
        egui::Grid::new("visible_history_boundaries")
            .num_columns(4)
            .show(ui, |ui| {
                ui.label("From");
                visible_history_boundary_ui(
                    re_ui,
                    ui,
                    &mut visible_history.from,
                    sequence_timeline,
                    current_time,
                    time_range.clone(),
                    true,
                    *visible_history_time_range.end(),
                );

                ui.end_row();

                ui.label("To");
                visible_history_boundary_ui(
                    re_ui,
                    ui,
                    &mut visible_history.to,
                    sequence_timeline,
                    current_time,
                    time_range,
                    false,
                    *visible_history_time_range.start(),
                );

                ui.end_row();
            });
    });
    ui.add(
        egui::Label::new(
            egui::RichText::new(if sequence_timeline {
                "These settings apply to all sequence timelines."
            } else {
                "These settings apply to all temporal timelines."
            })
            .italics()
            .weak(),
        )
        .wrap(true),
    );
}

#[allow(clippy::too_many_arguments)]
fn visible_history_boundary_ui(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    visible_history_boundary: &mut VisibleHistoryBoundary,
    sequence_timeline: bool,
    current_time: i64,
    mut time_range: RangeInclusive<i64>,
    low_bound: bool,
    other_boundary_absolute: i64,
) {
    let span = time_range.end() - time_range.start();

    // Hot "usability" area! This achieves two things:
    // 1) It makes sure the time range in relative mode has enough margin beyond the current
    //    timeline's span to avoid the boundary value to be modified by changing the current time
    //    cursor
    // 2) It makes sure the two boundaries don't cross in time (i.e. low > high). It does so by
    //    prioritizing the low boundary. Moving the low boundary against the high boundary will
    //    displace the high boundary. On the other hand, the high boundary cannot be moved against
    //    the low boundary. This asymmetry is intentional, and avoids both boundaries fighting each
    //    other in some corner cases (when the user interacts with the current time cursor).
    #[allow(clippy::collapsible_else_if)] // for readability
    if matches!(
        visible_history_boundary,
        VisibleHistoryBoundary::RelativeToTimeCursor(_)
    ) {
        if low_bound {
            time_range = -span..=2 * span;
        } else {
            time_range =
                (other_boundary_absolute.saturating_sub(current_time)).at_least(-span)..=2 * span;
        }
    } else {
        if !low_bound {
            time_range = other_boundary_absolute.at_least(-span)..=*time_range.end();
        }
    }

    match visible_history_boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(value)
        | VisibleHistoryBoundary::Absolute(value) => {
            if sequence_timeline {
                let speed = (span as f32 * 0.005).at_least(1.0);

                ui.add(
                    egui::DragValue::new(value)
                        .clamp_range(time_range)
                        .speed(speed),
                );
            } else {
                re_ui.time_drag_value(ui, value, &time_range);
            }
        }
        VisibleHistoryBoundary::Infinite => {
            let mut unused = 0.0;
            ui.add_enabled(
                false,
                egui::DragValue::new(&mut unused).custom_formatter(|_, _| "∞".to_owned()),
            );
        }
    }

    let (abs_time, rel_time) = match visible_history_boundary {
        VisibleHistoryBoundary::RelativeToTimeCursor(value) => (*value + current_time, *value),
        VisibleHistoryBoundary::Absolute(value) => (*value, *value - current_time),
        VisibleHistoryBoundary::Infinite => (current_time, 0),
    };

    egui::ComboBox::from_id_source(if low_bound {
        "time_history_low_bound"
    } else {
        "time_history_high_bound"
    })
    .selected_text(visible_history_boundary.label())
    .show_ui(ui, |ui| {
        ui.set_min_width(64.0);

        ui.selectable_value(
            visible_history_boundary,
            VisibleHistoryBoundary::RelativeToTimeCursor(rel_time),
            VisibleHistoryBoundary::RELATIVE_LABEL,
        )
        .on_hover_text(if low_bound {
            "Show data from a time point relative to the current time."
        } else {
            "Show data until a time point relative to the current time."
        });
        ui.selectable_value(
            visible_history_boundary,
            VisibleHistoryBoundary::Absolute(abs_time),
            VisibleHistoryBoundary::ABSOLUTE_LABEL,
        )
        .on_hover_text(if low_bound {
            "Show data from an absolute time point."
        } else {
            "Show data until an absolute time point."
        });
        ui.selectable_value(
            visible_history_boundary,
            VisibleHistoryBoundary::Infinite,
            VisibleHistoryBoundary::INFINITE_LABEL,
        )
        .on_hover_text(if low_bound {
            "Show data from the beginning of the timeline"
        } else {
            "Show data until the end of the timeline"
        });
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
