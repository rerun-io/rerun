use egui::ahash::{HashMap, HashSet};

use egui_plot::{Legend, Line, Plot, PlotPoint, Points};

use re_data_store::TimeType;
use re_format::next_grid_tick_magnitude_ns;
use re_log_types::{EntityPath, TimeInt, TimeZone};
use re_space_view::{controls, view_property_ui};
use re_types::blueprint::archetypes::{PlotLegend, ScalarAxis};
use re_types::blueprint::components::{Corner2D, LockRangeDuringZoom, Visible};
use re_types::{components::Range1D, datatypes::TimeRange, SpaceViewClassIdentifier, View};
use re_ui::{list_item, UiExt as _};
use re_viewer_context::external::re_entity_db::{
    EditableAutoValue, EntityProperties, TimeSeriesAggregator,
};
use re_viewer_context::{
    IdentifiedViewSystem, IndicatedEntities, PerVisualizer, QueryRange, RecommendedSpaceView,
    SmallVisualizerSet, SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId,
    SpaceViewSpawnHeuristics, SpaceViewState, SpaceViewStateExt as _,
    SpaceViewSystemExecutionError, SystemExecutionOutput, TypedComponentFallbackProvider,
    ViewContext, ViewQuery, ViewSystemIdentifier, ViewerContext, VisualizableEntities,
};
use re_viewport_blueprint::ViewProperty;

use crate::line_visualizer_system::SeriesLineSystem;
use crate::point_visualizer_system::SeriesPointSystem;
use crate::util::next_up_f64;
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

    /// The range of the scalar values currently on screen.
    scalar_range: Range1D,
}

impl Default for TimeSeriesSpaceViewState {
    fn default() -> Self {
        Self {
            is_dragging_time_cursor: false,
            was_dragging_time_cursor: false,
            saved_auto_bounds: Default::default(),
            scalar_range: [0.0, 0.0].into(),
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

type ViewType = re_types::blueprint::views::TimeSeriesView;

impl SpaceViewClass for TimeSeriesSpaceView {
    fn identifier() -> SpaceViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Time series"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_TIMESERIES
    }

    fn help_text(&self, egui_ctx: &egui::Context) -> egui::WidgetText {
        let mut layout = re_ui::LayoutJobBuilder::new(egui_ctx);

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
        system_registry.register_visualizer::<SeriesLineSystem>()?;
        system_registry.register_visualizer::<SeriesPointSystem>()?;
        Ok(())
    }

    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<TimeSeriesSpaceViewState>::default()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn SpaceViewState) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Low
    }

    fn default_query_range(&self) -> QueryRange {
        QueryRange::TimeRange(TimeRange::EVERYTHING)
    }

    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        space_view_id: SpaceViewId,
        root_entity_properties: &mut EntityProperties,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let state = state.downcast_mut::<TimeSeriesSpaceViewState>()?;

        list_item::list_item_scope(ui, "time_series_selection_ui", |ui| {
            ui.list_item()
                .interactive(false)
                .show_hierarchical(
                    ui,
                    list_item::PropertyContent::new("Zoom aggregation").value_fn(|ui, _| {
                        let mut agg_mode = *root_entity_properties.time_series_aggregator.get();

                        egui::ComboBox::from_id_source("aggregation_mode")
                            .selected_text(agg_mode.to_string())
                            .show_ui(ui, |ui| {
                                for variant in TimeSeriesAggregator::variants() {
                                    ui.selectable_value(
                                        &mut agg_mode,
                                        variant,
                                        variant.to_string(),
                                    )
                                    .on_hover_text(variant.description());
                                }
                            });

                        root_entity_properties.time_series_aggregator =
                            EditableAutoValue::UserEdited(agg_mode);
                    }),
                )
                .on_hover_text(
                    "Configures the zoom-dependent scalar aggregation.\n\
                     This is done only if steps on the X axis go below 1.0, i.e. a single pixel \
                     covers more than one tick worth of data. It can greatly improve performance \
                     (and readability) in such situations as it prevents overdraw.",
                );

            let visualizer_collection = ctx
                .space_view_class_registry
                .new_visualizer_collection(Self::identifier());

            let view_context = ViewContext {
                viewer_ctx: ctx,
                view_id: space_view_id,
                view_state: state,
                visualizer_collection: &visualizer_collection,
            };

            view_property_ui::<PlotLegend>(&view_context, ui, space_view_id, self);
            view_property_ui::<ScalarAxis>(&view_context, ui, space_view_id, self);
        });

        Ok(())
    }

    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> SpaceViewSpawnHeuristics {
        re_tracing::profile_function!();

        // For all following lookups, checking indicators is enough, since we know that this is enough to infer visualizability here.
        let mut indicated_entities = IndicatedEntities::default();

        for indicated in [
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
        //
        // This is the last hold out of "child of root" spawning, which we removed otherwise
        // (see https://github.com/rerun-io/rerun/issues/4926)
        let subtree_of_root_entity = &ctx.recording().tree().children;
        if indicated_entities.contains(&EntityPath::root())
            || subtree_of_root_entity
                .iter()
                .any(|(_, subtree)| indicated_entities.contains(&subtree.path))
        {
            return SpaceViewSpawnHeuristics::root();
        }

        // If there's other entities that have the right indicator & didn't match the above,
        // spawn a time series view for each child of the root that has any entities with the right indicator.
        let mut child_of_root_entities = HashSet::default();
        for entity in indicated_entities.iter() {
            if let Some(child_of_root) = entity.iter().next() {
                child_of_root_entities.insert(child_of_root);
            }
        }

        SpaceViewSpawnHeuristics::new(child_of_root_entities.into_iter().map(|path_part| {
            let entity = EntityPath::new(vec![path_part.clone()]);
            RecommendedSpaceView::new_subtree(entity)
        }))
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
        state: &mut dyn SpaceViewState,
        _root_entity_properties: &EntityProperties,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let blueprint_db = ctx.blueprint_db();
        let view_id = query.space_view_id;

        let visualizer_collection = ctx
            .space_view_class_registry
            .new_visualizer_collection(Self::identifier());

        let view_context = ViewContext {
            viewer_ctx: ctx,
            view_id: query.space_view_id,
            view_state: state,
            visualizer_collection: &visualizer_collection,
        };

        let plot_legend =
            ViewProperty::from_archetype::<PlotLegend>(blueprint_db, ctx.blueprint_query, view_id);
        let legend_visible = plot_legend.component_or_fallback::<Visible>(&view_context, self)?;
        let legend_corner = plot_legend.component_or_fallback::<Corner2D>(&view_context, self)?;

        let scalar_axis =
            ViewProperty::from_archetype::<ScalarAxis>(blueprint_db, ctx.blueprint_query, view_id);
        let y_range = scalar_axis.component_or_fallback::<Range1D>(&view_context, self)?;
        let y_zoom_lock =
            scalar_axis.component_or_fallback::<LockRangeDuringZoom>(&view_context, self)?;
        let y_zoom_lock = y_zoom_lock.0 .0;

        let state = state.downcast_mut::<TimeSeriesSpaceViewState>()?;

        let (current_time, time_type, timeline) = {
            // Avoid holding the lock for long
            let time_ctrl = ctx.rec_cfg.time_ctrl.read();
            let current_time = time_ctrl.time_i64();
            let time_type = time_ctrl.time_type();
            let timeline = *time_ctrl.timeline();
            (current_time, time_type, timeline)
        };

        let timeline_name = timeline.name().to_string();

        let line_series = system_output.view_systems.get::<SeriesLineSystem>()?;
        let point_series = system_output.view_systems.get::<SeriesPointSystem>()?;

        let all_plot_series: Vec<_> = std::iter::empty()
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

        let lock_y_during_zoom =
            y_zoom_lock || ui.input(|i| i.modifiers.contains(controls::ASPECT_SCROLL_MODIFIER));

        // We don't want to allow vertical when y is locked or else the view "bounces" when we scroll and
        // then reset to the locked range.
        if lock_y_during_zoom {
            ui.input_mut(|i| i.smooth_scroll_delta.y = 0.0);
        }

        // TODO(#5075): Boxed-zoom should be fixed to accommodate the locked range.
        let time_zone_for_timestamps = ctx.app_options.time_zone;
        let mut plot = Plot::new(plot_id_src)
            .id(crate::plot_id(query.space_view_id))
            .auto_bounds([true, false].into()) // Never use y auto bounds: we dictated bounds via blueprint under all circumstances.
            .allow_zoom([true, !lock_y_during_zoom])
            .x_axis_formatter(move |time, _, _| {
                format_time(
                    time_type,
                    (time.value as i64).saturating_add(time_offset),
                    time_zone_for_timestamps,
                )
            })
            .y_axis_formatter(move |mark, _, _| format_y_axis(mark))
            .y_axis_width(3) // in digits
            .label_formatter(move |name, value| {
                let name = if name.is_empty() { "y" } else { name };
                let label = time_type.format(
                    TimeInt::new_temporal((value.x as i64).saturating_add(time_offset)),
                    time_zone_for_timestamps,
                );

                let y_value = re_format::format_f64(value.y);

                if aggregator == TimeSeriesAggregator::Off || aggregation_factor <= 1.0 {
                    format!("{timeline_name}: {label}\n{name}: {y_value}")
                } else {
                    format!(
                        "{timeline_name}: {label}\n{name}: {y_value}\n\
                        {aggregator} aggregation over approx. {aggregation_factor:.1} time points",
                    )
                }
            });

        if *legend_visible {
            plot = plot.legend(Legend::default().position(legend_corner.into()));
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

            is_resetting = plot_ui.response().double_clicked();

            let current_bounds = plot_ui.plot_bounds();
            plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                [current_bounds.min()[0], y_range.start()],
                [current_bounds.max()[0], y_range.end()],
            ));

            let current_auto = plot_ui.auto_bounds();
            plot_ui.set_auto_bounds(
                [
                    current_auto[0] || is_resetting,
                    is_resetting && !y_zoom_lock,
                ]
                .into(),
            );

            *state.scalar_range.start_mut() = f64::INFINITY;
            *state.scalar_range.end_mut() = f64::NEG_INFINITY;

            for series in all_plot_series {
                let points = series
                    .points
                    .iter()
                    .map(|p| {
                        if p.1 < state.scalar_range.start() {
                            *state.scalar_range.start_mut() = p.1;
                        }
                        if p.1 > state.scalar_range.end() {
                            *state.scalar_range.end_mut() = p.1;
                        }

                        [(p.0 - time_offset) as _, p.1]
                    })
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
        });

        // Write new y_range if it has changed.
        let new_y_range = Range1D::new(transform.bounds().min()[1], transform.bounds().max()[1]);
        if is_resetting {
            scalar_axis.reset_blueprint_component::<Range1D>(ctx);
        } else if new_y_range != y_range {
            scalar_axis.save_blueprint_component(ctx, &new_y_range);
        }

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

            ui.paint_time_cursor(ui.painter(), &response, time_x, response.rect.y_range());
        }

        Ok(())
    }
}

fn format_time(time_type: TimeType, time_int: i64, time_zone_for_timestamps: TimeZone) -> String {
    if time_type == TimeType::Time {
        let time = re_log_types::Time::from_ns_since_epoch(time_int);
        time.format_time_compact(time_zone_for_timestamps)
    } else {
        time_type.format(TimeInt::new_temporal(time_int), time_zone_for_timestamps)
    }
}

fn format_y_axis(mark: egui_plot::GridMark) -> String {
    // Example: If the step to the next tick is `0.01`, we should use 2 decimals of precision:
    let num_decimals = -mark.step_size.log10().round() as usize;

    re_format::FloatFormatOptions::DEFAULT_f64
        .with_decimals(num_decimals)
        .format(mark.value)
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
        let next_ns = next_grid_tick_magnitude_ns(small_spacing_ns);
        if small_spacing_ns < next_ns {
            small_spacing_ns = next_ns;
        } else {
            break; // we've reached the max
        }
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

        if let Some(new_ns) = current_ns.checked_add(small_spacing_ns) {
            current_ns = new_ns;
        } else {
            break;
        };
    }

    marks
}

fn round_ns_to_start_of_day(ns: i64) -> i64 {
    let ns_per_day = 24 * 60 * 60 * 1_000_000_000;
    (ns + ns_per_day / 2) / ns_per_day * ns_per_day
}

impl TypedComponentFallbackProvider<Corner2D> for TimeSeriesSpaceView {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> Corner2D {
        // Explicitly pick RightCorner2D::RightBottom, we don't want to make this dependent on the (arbitrary)
        // default of Corner2D
        Corner2D::RightBottom
    }
}

impl TypedComponentFallbackProvider<Range1D> for TimeSeriesSpaceView {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> Range1D {
        ctx.view_ctx
            .view_state
            .as_any()
            .downcast_ref::<TimeSeriesSpaceViewState>()
            .map(|s| {
                let mut range = s.scalar_range;

                // egui_plot can't handle a zero or negative range.
                // Enforce a minimum range.
                if !range.start().is_normal() {
                    *range.start_mut() = -1.0;
                }
                if !range.end().is_normal() {
                    *range.end_mut() = 1.0;
                }
                if range.start() >= range.end() {
                    *range.start_mut() = next_up_f64(range.end());
                }

                range
            })
            .unwrap_or_default()
    }
}

re_viewer_context::impl_component_fallback_provider!(TimeSeriesSpaceView => [Corner2D, Range1D]);
