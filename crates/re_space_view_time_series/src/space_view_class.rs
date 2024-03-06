use egui::ahash::{HashMap, HashSet};
use egui::NumExt as _;
use egui_plot::{Legend, Line, Plot, PlotPoint, Points};

use re_data_store::TimeType;
use re_format::next_grid_tick_magnitude_ns;
use re_log_types::{EntityPath, EntityPathFilter, TimeZone};
use re_space_view::{controls, query_space_view_sub_archetype_or_default};
use re_types::blueprint::components::Corner2D;
use re_types::components::Range1D;
use re_viewer_context::external::re_entity_db::{
    EditableAutoValue, EntityProperties, TimeSeriesAggregator,
};
use re_viewer_context::{
    IdentifiedViewSystem, IndicatedEntities, PerVisualizer, RecommendedSpaceView,
    SmallVisualizerSet, SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId,
    SpaceViewSpawnHeuristics, SpaceViewState, SpaceViewSystemExecutionError, SystemExecutionOutput,
    ViewQuery, ViewSystemIdentifier, ViewerContext, VisualizableEntities,
};

use crate::legacy_visualizer_system::LegacyTimeSeriesSystem;
use crate::line_visualizer_system::SeriesLineSystem;
use crate::point_visualizer_system::SeriesPointSystem;
use crate::PlotSeriesKind;

// ---

#[derive(Clone)]
pub struct TimeSeriesSpaceViewState {
    /// Is the user dragging the cursor this frame?
    is_dragging_time_cursor: bool,

    /// Was the user dragging the cursor last frame?
    was_dragging_time_cursor: bool,

    /// State of egui_plot's auto bounds before the user started dragging the time cursor.
    saved_auto_bounds: egui::Vec2b,

    /// State of egui_plot's bounds.
    saved_y_axis_range: [f64; 2],

    /// To track when the range has been edited.
    last_y_range: Option<Range1D>,

    /// To track when the range lock has been enabled/disabled.
    last_y_lock_range_during_zoom: bool,
}

impl Default for TimeSeriesSpaceViewState {
    fn default() -> Self {
        Self {
            is_dragging_time_cursor: false,
            was_dragging_time_cursor: false,
            saved_auto_bounds: Default::default(),
            saved_y_axis_range: [0.0, 1.0],
            last_y_range: None,
            last_y_lock_range_during_zoom: false,
        }
    }
}

impl SpaceViewState for TimeSeriesSpaceViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct TimeSeriesSpaceView;

const DEFAULT_LEGEND_CORNER: egui_plot::Corner = egui_plot::Corner::RightBottom;

impl SpaceViewClass for TimeSeriesSpaceView {
    type State = TimeSeriesSpaceViewState;

    const IDENTIFIER: &'static str = "Time Series";
    const DISPLAY_NAME: &'static str = "Time Series";

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_TIMESERIES
    }

    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText {
        let mut layout = re_ui::LayoutJobBuilder::new(re_ui);

        layout.add("Pan by dragging, or scroll (+ ");
        layout.add(controls::HORIZONTAL_SCROLL_MODIFIER);
        layout.add(" for horizontal).\n");

        layout.add("Zoom with pinch gesture or scroll + ");
        layout.add(controls::ZOOM_SCROLL_MODIFIER);
        layout.add(".\n");

        layout.add("Scroll + ");
        layout.add(controls::ASPECT_SCROLL_MODIFIER);
        layout.add(" to zoom only the temporal axis while holding the y-range fixed.\n");

        layout.add("Drag ");
        layout.add(controls::SELECTION_RECT_ZOOM_BUTTON);
        layout.add(" to zoom in/out using a selection.\n");

        layout.add("Click ");
        layout.add(controls::MOVE_TIME_CURSOR_BUTTON);
        layout.add(" to move the time cursor.\n\n");

        layout.add_button_text(controls::RESET_VIEW_BUTTON_TEXT);
        layout.add(" to reset the view.");

        layout.layout_job.into()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_visualizer::<LegacyTimeSeriesSystem>()?;
        system_registry.register_visualizer::<SeriesLineSystem>()?;
        system_registry.register_visualizer::<SeriesPointSystem>()?;
        Ok(())
    }

    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Low
    }

    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _space_origin: &EntityPath,
        space_view_id: SpaceViewId,
        root_entity_properties: &mut EntityProperties,
    ) {
        ctx.re_ui
        .selection_grid(ui, "time_series_selection_ui_aggregation")
        .show(ui, |ui| {
            ctx.re_ui
                .grid_left_hand_label(ui, "Zoom Aggregation")
                .on_hover_text("Configures the zoom-dependent scalar aggregation.\n
This is done only if steps on the X axis go below 1.0, i.e. a single pixel covers more than one tick worth of data.\n
It can greatly improve performance (and readability) in such situations as it prevents overdraw.");

            let mut agg_mode = *root_entity_properties.time_series_aggregator.get();

            egui::ComboBox::from_id_source("aggregation_mode")
                .selected_text(agg_mode.to_string())
                .show_ui(ui, |ui| {
                    ui.style_mut().wrap = Some(false);
                    ui.set_min_width(64.0);

                    for variant in TimeSeriesAggregator::variants() {
                        ui.selectable_value(&mut agg_mode, variant, variant.to_string())
                            .on_hover_text(variant.description());
                    }
                });

            root_entity_properties.time_series_aggregator =
                EditableAutoValue::UserEdited(agg_mode);
        });

        legend_ui(ctx, space_view_id, ui);
        axis_ui(ctx, space_view_id, ui, state);
    }

    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> SpaceViewSpawnHeuristics {
        re_tracing::profile_function!();

        // For all following lookups, checking indicators is enough, since we know that this is enough to infer visualizability here.
        let mut indicated_entities = IndicatedEntities::default();

        for indicated in [
            LegacyTimeSeriesSystem::identifier(),
            SeriesLineSystem::identifier(),
            SeriesPointSystem::identifier(),
        ]
        .iter()
        .filter_map(|&system_id| ctx.indicated_entities_per_visualizer.get(&system_id))
        {
            indicated_entities.0.extend(indicated.0.iter().cloned());
        }

        // Because SeriesLine is our fallback visualizer, also include any entities for which
        // SeriesLine is applicable, even if not indicated.
        if let Some(applicable) = ctx
            .applicable_entities_per_visualizer
            .get(&SeriesLineSystem::identifier())
        {
            indicated_entities.0.extend(applicable.iter().cloned());
        }

        if indicated_entities.0.is_empty() {
            return SpaceViewSpawnHeuristics::default();
        }

        // Spawn time series data at the root if there's time series data either
        // directly at the root or one of its children.
        // TODO(#4926): This seems to be unnecessarily complicated.
        let subtree_of_root_entity = &ctx.entity_db.tree().children;
        if indicated_entities.contains(&EntityPath::root())
            || subtree_of_root_entity
                .iter()
                .any(|(_, subtree)| indicated_entities.contains(&subtree.path))
        {
            return SpaceViewSpawnHeuristics {
                recommended_space_views: vec![RecommendedSpaceView {
                    root: EntityPath::root(),
                    query_filter: EntityPathFilter::subtree_entity_filter(&EntityPath::root()),
                }],
            };
        }

        // If there's other entities that have the right indicator & didn't match the above,
        // spawn a time series view for each child of the root that has any entities with the right indicator.
        let mut child_of_root_entities = HashSet::default();
        for entity in indicated_entities.iter() {
            if let Some(child_of_root) = entity.iter().next() {
                child_of_root_entities.insert(child_of_root);
            }
        }
        let recommended_space_views = child_of_root_entities
            .into_iter()
            .map(|path_part| {
                let entity = EntityPath::new(vec![path_part.clone()]);
                RecommendedSpaceView {
                    query_filter: EntityPathFilter::subtree_entity_filter(&entity),
                    root: entity,
                }
            })
            .collect();

        SpaceViewSpawnHeuristics {
            recommended_space_views,
        }
    }

    /// Choose the default visualizers to enable for this entity.
    fn choose_default_visualizers(
        &self,
        entity_path: &EntityPath,
        visualizable_entities_per_visualizer: &PerVisualizer<VisualizableEntities>,
        indicated_entities_per_visualizer: &PerVisualizer<IndicatedEntities>,
    ) -> SmallVisualizerSet {
        let available_visualizers: HashSet<&ViewSystemIdentifier> =
            visualizable_entities_per_visualizer
                .iter()
                .filter_map(|(visualizer, ents)| {
                    if ents.contains(entity_path) {
                        Some(visualizer)
                    } else {
                        None
                    }
                })
                .collect();

        let mut visualizers: SmallVisualizerSet = available_visualizers
            .iter()
            .filter_map(|visualizer| {
                if indicated_entities_per_visualizer
                    .get(*visualizer)
                    .map_or(false, |matching_list| matching_list.contains(entity_path))
                {
                    Some(**visualizer)
                } else {
                    None
                }
            })
            .collect();

        // If there were no other visualizers, but the SeriesLineSystem is available, use it.
        if visualizers.is_empty() && available_visualizers.contains(&SeriesLineSystem::identifier())
        {
            visualizers.insert(0, SeriesLineSystem::identifier());
        }

        visualizers
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _root_entity_properties: &EntityProperties,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let blueprint_db = ctx.store_context.blueprint;
        let blueprint_query = ctx.blueprint_query;

        let (
            re_types::blueprint::archetypes::PlotLegend {
                visible: legend_visible,
                corner: legend_corner,
            },
            _,
        ) = query_space_view_sub_archetype_or_default(query.space_view_id, blueprint_db, blueprint_query);

        let (
            re_types::blueprint::archetypes::ScalarAxis {
                range: y_range,
                lock_range_during_zoom: y_lock_range_during_zoom,
            },
            _,
        ) = query_space_view_sub_archetype_or_default(query.space_view_id, blueprint_db, blueprint_query);

        let (current_time, time_type, timeline) = {
            // Avoid holding the lock for long
            let time_ctrl = ctx.rec_cfg.time_ctrl.read();
            let current_time = time_ctrl.time_i64();
            let time_type = time_ctrl.time_type();
            let timeline = *time_ctrl.timeline();
            (current_time, time_type, timeline)
        };

        let timeline_name = timeline.name().to_string();

        let legacy_time_series = system_output.view_systems.get::<LegacyTimeSeriesSystem>()?;
        let line_series = system_output.view_systems.get::<SeriesLineSystem>()?;
        let point_series = system_output.view_systems.get::<SeriesPointSystem>()?;

        let all_plot_series: Vec<_> = legacy_time_series
            .all_series
            .iter()
            .chain(line_series.all_series.iter())
            .chain(point_series.all_series.iter())
            .collect();

        // Get the minimum time/X value for the entire plot…
        let min_time = all_plot_series
            .iter()
            .map(|line| line.min_time)
            .min()
            .unwrap_or(0);

        // TODO(jleibs): If this is allowed to be different, need to track it per line.
        let aggregation_factor = all_plot_series
            .first()
            .map_or(1.0, |line| line.aggregation_factor);

        let aggregator = all_plot_series
            .first()
            .map(|line| line.aggregator)
            .unwrap_or_default();

        // …then use that as an offset to avoid nasty precision issues with
        // large times (nanos since epoch does not fit into a f64).
        let time_offset = if timeline.typ() == TimeType::Time {
            // In order to make the tick-marks on the time axis fall on whole days, hours, minutes etc,
            // we need to round to a whole day:
            round_ns_to_start_of_day(min_time)
        } else {
            min_time
        };

        // use timeline_name as part of id, so that egui stores different pan/zoom for different timelines
        let plot_id_src = ("plot", &timeline_name);

        let y_lock_range_during_zoom = y_lock_range_during_zoom.map_or(false, |v| v.0);
        let lock_y_during_zoom = y_lock_range_during_zoom
            || ui.input(|i| i.modifiers.contains(controls::ASPECT_SCROLL_MODIFIER));

        let auto_y = y_range.is_none();

        // We don't want to allow vertical when y is locked or else the view "bounces" when we scroll and
        // then reset to the locked range.
        if lock_y_during_zoom {
            ui.input_mut(|i| i.smooth_scroll_delta.y = 0.0);
        }

        // TODO(jleibs): Would be nice to disable vertical drag instead of just resetting.

        // TODO(#5075): Boxed-zoom should be fixed to accommodate the locked range.
        let time_zone_for_timestamps = ctx.app_options.time_zone;
        let mut plot = Plot::new(plot_id_src)
            .id(crate::plot_id(query.space_view_id))
            .auto_bounds([true, auto_y].into())
            .allow_zoom([true, !lock_y_during_zoom])
            .allow_drag([true, !lock_y_during_zoom])
            .x_axis_formatter(move |time, _, _| {
                format_time(
                    time_type,
                    time.value as i64 + time_offset,
                    time_zone_for_timestamps,
                )
            })
            .label_formatter(move |name, value| {
                let name = if name.is_empty() { "y" } else { name };
                let label = time_type.format(
                    (value.x as i64 + time_offset).into(),
                    time_zone_for_timestamps,
                );

                let is_integer = value.y.round() == value.y;
                let decimals = if is_integer { 0 } else { 5 };

                if aggregator == TimeSeriesAggregator::Off || aggregation_factor <= 1.0 {
                    format!("{timeline_name}: {label}\n{name}: {:.decimals$}", value.y)
                } else {
                    format!(
                        "{timeline_name}: {label}\n{name}: {:.decimals$}\n\
                        {aggregator} aggregation over approx. {aggregation_factor:.1} time points",
                        value.y,
                    )
                }
            });

        if legend_visible.unwrap_or(true.into()).0 {
            plot = plot.legend(
                Legend::default().position(
                    legend_corner
                        .unwrap_or_default()
                        .try_into()
                        .unwrap_or(DEFAULT_LEGEND_CORNER),
                ),
            );
        }

        if timeline.typ() == TimeType::Time {
            let canvas_size = ui.available_size();
            plot = plot.x_grid_spacer(move |spacer| ns_grid_spacer(canvas_size, &spacer));
        }

        let mut plot_item_id_to_entity_path = HashMap::default();

        let mut is_resetting = false;

        let egui_plot::PlotResponse {
            inner: _,
            response,
            transform,
            hovered_plot_item,
        } = plot.show(ui, |plot_ui| {
            if plot_ui.response().secondary_clicked() {
                let mut time_ctrl_write = ctx.rec_cfg.time_ctrl.write();
                let timeline = *time_ctrl_write.timeline();
                time_ctrl_write.set_timeline_and_time(
                    timeline,
                    plot_ui.pointer_coordinate().unwrap().x as i64 + time_offset,
                );
                time_ctrl_write.pause();
            }

            let range_was_edited = state.last_y_range != y_range;
            state.last_y_range = y_range;

            let locked_y_range_was_enabled =
                y_lock_range_during_zoom && !state.last_y_lock_range_during_zoom;
            state.last_y_lock_range_during_zoom = y_lock_range_during_zoom;

            is_resetting = plot_ui.response().double_clicked();
            let current_auto = plot_ui.auto_bounds();

            if let Some(y_range) = y_range {
                // If we have a y_range, there are a few cases where we want to adjust the bounds.
                // - The range was just edited
                // - The locking behavior was just set to true
                // - The zoom behavior is in LockToRange
                // - The user double-clicked
                if range_was_edited
                    || locked_y_range_was_enabled
                    || lock_y_during_zoom
                    || is_resetting
                {
                    let current_bounds = plot_ui.plot_bounds();
                    let mut min = current_bounds.min();
                    let mut max = current_bounds.max();

                    if range_was_edited || is_resetting || locked_y_range_was_enabled {
                        min[1] = y_range.0[0];
                        max[1] = y_range.0[1];
                    }

                    let new_bounds = egui_plot::PlotBounds::from_min_max(min, max);
                    plot_ui.set_plot_bounds(new_bounds);
                    // If we are resetting, we still want the X value to be auto for
                    // this frame.
                    plot_ui.set_auto_bounds([current_auto[0] || is_resetting, false].into());
                }
            } else if lock_y_during_zoom || range_was_edited {
                plot_ui.set_auto_bounds([current_auto[0] || is_resetting, is_resetting].into());
            }

            for series in all_plot_series {
                let points = series
                    .points
                    .iter()
                    .map(|p| [(p.0 - time_offset) as _, p.1])
                    .collect::<Vec<_>>();

                let color = series.color;
                let id = egui::Id::new(series.entity_path.hash());
                plot_item_id_to_entity_path.insert(id, series.entity_path.clone());

                match series.kind {
                    PlotSeriesKind::Continuous => plot_ui.line(
                        Line::new(points)
                            .name(series.label.as_str())
                            .color(color)
                            .width(series.width)
                            .id(id),
                    ),
                    PlotSeriesKind::Scatter(scatter_attrs) => plot_ui.points(
                        Points::new(points)
                            .name(series.label.as_str())
                            .color(color)
                            .radius(series.width)
                            .shape(scatter_attrs.marker.into())
                            .id(id),
                    ),
                    // Break up the chart. At some point we might want something fancier.
                    PlotSeriesKind::Clear => {}
                }
            }

            if state.is_dragging_time_cursor {
                if !state.was_dragging_time_cursor {
                    state.saved_auto_bounds = plot_ui.auto_bounds();
                }
                // Freeze any change to the plot boundaries to avoid weird interaction with the time
                // cursor.
                plot_ui.set_plot_bounds(plot_ui.plot_bounds());
            } else if state.was_dragging_time_cursor {
                plot_ui.set_auto_bounds(state.saved_auto_bounds);
            }

            state.was_dragging_time_cursor = state.is_dragging_time_cursor;
            let bounds = plot_ui.plot_bounds().range_y();
            state.saved_y_axis_range = [*bounds.start(), *bounds.end()];
        });

        // Decide if the time cursor should be displayed, and if so where:
        let time_x = current_time
            .map(|current_time| (current_time - time_offset) as f64)
            .filter(|&x| {
                // only display the time cursor when it's actually above the plot area
                transform.bounds().min()[0] <= x && x <= transform.bounds().max()[0]
            })
            .map(|x| transform.position_from_point(&PlotPoint::new(x, 0.0)).x);

        // If we are not resetting on this frame, interact with the plot items (lines, scatters, etc.)
        if !is_resetting {
            if let Some(hovered) = hovered_plot_item
                .and_then(|hovered_plot_item| plot_item_id_to_entity_path.get(&hovered_plot_item))
                .map(|entity_path| {
                    re_viewer_context::Item::DataResult(
                        query.space_view_id,
                        entity_path.clone().into(),
                    )
                })
                .or_else(|| {
                    if response.hovered() {
                        Some(re_viewer_context::Item::SpaceView(query.space_view_id))
                    } else {
                        None
                    }
                })
            {
                ctx.select_hovered_on_click(&response, hovered);
            }
        }

        if let Some(mut time_x) = time_x {
            let interact_radius = ui.style().interaction.resize_grab_radius_side;
            let line_rect = egui::Rect::from_x_y_ranges(time_x..=time_x, response.rect.y_range())
                .expand(interact_radius);

            let time_drag_id = ui.id().with("time_drag");
            let response = ui
                .interact(line_rect, time_drag_id, egui::Sense::drag())
                .on_hover_and_drag_cursor(egui::CursorIcon::ResizeHorizontal);

            state.is_dragging_time_cursor = false;
            if response.dragged() {
                if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let new_offset_time = transform.value_from_position(pointer_pos).x;
                    let new_time = time_offset + new_offset_time.round() as i64;

                    // Avoid frame-delay:
                    time_x = pointer_pos.x;

                    let mut time_ctrl = ctx.rec_cfg.time_ctrl.write();
                    time_ctrl.set_time(new_time);
                    time_ctrl.pause();

                    state.is_dragging_time_cursor = true;
                }
            }

            ctx.re_ui.paint_time_cursor(
                ui,
                ui.painter(),
                &response,
                time_x,
                response.rect.y_range(),
            );
        }

        Ok(())
    }
}

fn legend_ui(ctx: &ViewerContext<'_>, space_view_id: SpaceViewId, ui: &mut egui::Ui) {
    // TODO(jleibs): use editors

    let blueprint_db = ctx.store_context.blueprint;
    let blueprint_query = ctx.blueprint_query;
    let (re_types::blueprint::archetypes::PlotLegend { visible, corner }, blueprint_path) =
        query_space_view_sub_archetype_or_default(space_view_id, blueprint_db, blueprint_query);

    ctx.re_ui
        .selection_grid(ui, "time_series_selection_ui_legend")
        .show(ui, |ui| {
            ctx.re_ui.grid_left_hand_label(ui, "Legend");

            ui.vertical(|ui| {
                let visible = visible.unwrap_or(true.into());
                let mut edit_visibility = visible;
                ctx.re_ui.checkbox(ui, &mut edit_visibility.0, "Visible");
                if visible != edit_visibility {
                    ctx.save_blueprint_component(&blueprint_path, &edit_visibility);
                }

                let corner = corner.unwrap_or(DEFAULT_LEGEND_CORNER.into());
                let mut edit_corner = corner;
                egui::ComboBox::from_id_source("legend_corner")
                    .selected_text(format!("{corner}"))
                    .show_ui(ui, |ui| {
                        ui.style_mut().wrap = Some(false);
                        ui.set_min_width(64.0);

                        ui.selectable_value(
                            &mut edit_corner,
                            egui_plot::Corner::LeftTop.into(),
                            format!("{}", Corner2D::from(egui_plot::Corner::LeftTop)),
                        );
                        ui.selectable_value(
                            &mut edit_corner,
                            egui_plot::Corner::RightTop.into(),
                            format!("{}", Corner2D::from(egui_plot::Corner::RightTop)),
                        );
                        ui.selectable_value(
                            &mut edit_corner,
                            egui_plot::Corner::LeftBottom.into(),
                            format!("{}", Corner2D::from(egui_plot::Corner::LeftBottom)),
                        );
                        ui.selectable_value(
                            &mut edit_corner,
                            egui_plot::Corner::RightBottom.into(),
                            format!("{}", Corner2D::from(egui_plot::Corner::RightBottom)),
                        );
                    });
                if corner != edit_corner {
                    ctx.save_blueprint_component(&blueprint_path, &edit_corner);
                }
            });

            ui.end_row();
        });
}

fn axis_ui(
    ctx: &ViewerContext<'_>,
    space_view_id: SpaceViewId,
    ui: &mut egui::Ui,
    state: &TimeSeriesSpaceViewState,
) {
    // TODO(jleibs): use editors

    let (
        re_types::blueprint::archetypes::ScalarAxis {
            range: y_range,
            lock_range_during_zoom: y_lock_range_during_zoom,
        },
        blueprint_path,
    ) = query_space_view_sub_archetype_or_default(
        space_view_id,
        ctx.store_context.blueprint,
        ctx.blueprint_query,
    );

    ctx.re_ui.collapsing_header(ui, "Y Axis", true, |ui| {
        ctx.re_ui
            .selection_grid(ui, "time_series_selection_ui_y_axis_range")
            .show(ui, |ui| {
                ctx.re_ui.grid_left_hand_label(ui, "Range");

                ui.vertical(|ui| {
                    let mut auto_range = y_range.is_none();

                    ui.horizontal(|ui| {
                        ctx.re_ui
                            .radio_value(ui, &mut auto_range, true, "Auto")
                            .on_hover_text("Automatically adjust the Y axis to fit the data.");
                        ctx.re_ui
                            .radio_value(ui, &mut auto_range, false, "Manual")
                            .on_hover_text("Manually specify a min and max Y value. This will define the range when resetting or locking the view range.");
                    });

                    if !auto_range {
                        let mut range_edit = y_range
                            .unwrap_or_else(|| y_range.unwrap_or(Range1D(state.saved_y_axis_range)));

                        ui.horizontal(|ui| {
                            // Max < Min is not supported.
                            // Also, egui_plot doesn't handle min==max (it ends up picking a default range instead then)
                            let prev_min = crate::util::next_up_f64(range_edit.0[0]);
                            let prev_max = range_edit.0[1];
                            // Scale the speed to the size of the range
                            let speed = ((prev_max - prev_min) * 0.01).at_least(0.001);
                            ui.label("Min");
                            ui.add(
                                egui::DragValue::new(&mut range_edit.0[0])
                                    .speed(speed)
                                    .clamp_range(std::f64::MIN..=prev_max),
                            );
                            ui.label("Max");
                            ui.add(
                                egui::DragValue::new(&mut range_edit.0[1])
                                    .speed(speed)
                                    .clamp_range(prev_min..=std::f64::MAX),
                            );
                        });

                        if y_range != Some(range_edit) {
                            ctx.save_blueprint_component(&blueprint_path, &range_edit);
                        }
                    } else if y_range.is_some() {
                        ctx.save_empty_blueprint_component::<Range1D>(&blueprint_path);
                    }
                });

                ui.end_row();
            });

        ctx.re_ui
            .selection_grid(ui, "time_series_selection_ui_y_axis_zoom")
            .show(ui, |ui| {
                ctx.re_ui.grid_left_hand_label(ui, "Zoom Behavior");

                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let y_lock_zoom = y_lock_range_during_zoom.unwrap_or(false.into());
                        let mut edit_locked = y_lock_zoom;
                        ctx.re_ui
                            .checkbox(ui, &mut edit_locked.0, "Lock Range")
                            .on_hover_text(
                            "If set, when zooming, the Y axis range will remain locked to the specified range.",
                        );
                        if y_lock_zoom != edit_locked {
                            ctx.save_blueprint_component(&blueprint_path, &edit_locked);
                        }
                    })
                });

                ui.end_row();
            });
    });
}

fn format_time(time_type: TimeType, time_int: i64, time_zone_for_timestamps: TimeZone) -> String {
    if time_type == TimeType::Time {
        let time = re_log_types::Time::from_ns_since_epoch(time_int);
        time.format_time_compact(time_zone_for_timestamps)
    } else {
        time_type.format(
            re_log_types::TimeInt::from(time_int),
            time_zone_for_timestamps,
        )
    }
}

fn ns_grid_spacer(
    canvas_size: egui::Vec2,
    input: &egui_plot::GridInput,
) -> Vec<egui_plot::GridMark> {
    let minimum_medium_line_spacing = 150.0; // ≈min size of a label
    let max_medium_lines = canvas_size.x as f64 / minimum_medium_line_spacing;

    let (min_ns, max_ns) = input.bounds;
    let width_ns = max_ns - min_ns;

    let mut small_spacing_ns = 1;
    while width_ns / (next_grid_tick_magnitude_ns(small_spacing_ns) as f64) > max_medium_lines {
        small_spacing_ns = next_grid_tick_magnitude_ns(small_spacing_ns);
    }
    let medium_spacing_ns = next_grid_tick_magnitude_ns(small_spacing_ns);
    let big_spacing_ns = next_grid_tick_magnitude_ns(medium_spacing_ns);

    let mut current_ns = (min_ns.floor() as i64) / small_spacing_ns * small_spacing_ns;
    let mut marks = vec![];

    while current_ns <= max_ns.ceil() as i64 {
        let is_big_line = current_ns % big_spacing_ns == 0;
        let is_medium_line = current_ns % medium_spacing_ns == 0;

        let step_size = if is_big_line {
            big_spacing_ns
        } else if is_medium_line {
            medium_spacing_ns
        } else {
            small_spacing_ns
        };

        marks.push(egui_plot::GridMark {
            value: current_ns as f64,
            step_size: step_size as f64,
        });

        current_ns += small_spacing_ns;
    }

    marks
}

fn round_ns_to_start_of_day(ns: i64) -> i64 {
    let ns_per_day = 24 * 60 * 60 * 1_000_000_000;
    (ns + ns_per_day / 2) / ns_per_day * ns_per_day
}
