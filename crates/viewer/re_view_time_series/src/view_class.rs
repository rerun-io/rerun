use egui::ahash::{HashMap, HashSet};
use egui_plot::{Legend, Line, Plot, PlotPoint, Points};

use crate::line_visualizer_system::SeriesLineSystem;
use crate::point_visualizer_system::SeriesPointSystem;
use crate::PlotSeriesKind;
use re_chunk_store::TimeType;
use re_format::next_grid_tick_magnitude_ns;
use re_log_types::{EntityPath, ResolvedEntityPathFilter, TimeInt, TimestampFormat};
use re_types::{
    archetypes::{SeriesLine, SeriesPoint},
    blueprint::{
        archetypes::{PlotLegend, ScalarAxis},
        components::{Corner2D, LockRangeDuringZoom},
    },
    components::{AggregationPolicy, Range1D, SeriesVisible, Visible},
    datatypes::TimeRange,
    ComponentBatch as _, View as _, ViewClassIdentifier,
};
use re_ui::{icon_text, icons, list_item, shortcut_with_icon, Help, MouseButtonText, UiExt as _};
use re_view::controls::{
    ASPECT_SCROLL_MODIFIER, MOVE_TIME_CURSOR_BUTTON, SELECTION_RECT_ZOOM_BUTTON,
    ZOOM_SCROLL_MODIFIER,
};
use re_view::{controls, view_property_ui};
use re_viewer_context::external::re_entity_db::InstancePath;
use re_viewer_context::{
    IdentifiedViewSystem as _, IndicatedEntities, MaybeVisualizableEntities, PerVisualizer,
    QueryRange, RecommendedView, SmallVisualizerSet, SystemExecutionOutput,
    TypedComponentFallbackProvider, ViewClass, ViewClassRegistryError, ViewHighlights, ViewId,
    ViewQuery, ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError,
    ViewSystemIdentifier, ViewerContext, VisualizableEntities,
};
use re_viewport_blueprint::ViewProperty;
use smallvec::SmallVec;
// ---

#[derive(Clone)]
pub struct TimeSeriesViewState {
    /// Is the user dragging the cursor this frame?
    is_dragging_time_cursor: bool,

    /// Was the user dragging the cursor last frame?
    was_dragging_time_cursor: bool,

    /// State of `egui_plot`'s auto bounds before the user started dragging the time cursor.
    saved_auto_bounds: egui::Vec2b,

    /// The range of the scalar values currently on screen.
    scalar_range: Range1D,

    /// We offset the time values of the plot so that unix timestamps don't run out of precision.
    ///
    /// Other parts of the system, such as query clamping, need to be aware of that offset in order
    /// to work properly.
    pub(crate) time_offset: i64,

    /// Default names for entities, used when no label is provided.
    ///
    /// This is here because it must be computed with full knowledge of all entities in the plot
    /// (e.g. to avoid `hello/x` and `world/x` both being named `x`), and this knowledge must be
    /// forwarded to the default providers.
    pub(crate) default_names_for_entities: HashMap<EntityPath, String>,
}

impl Default for TimeSeriesViewState {
    fn default() -> Self {
        Self {
            is_dragging_time_cursor: false,
            was_dragging_time_cursor: false,
            saved_auto_bounds: Default::default(),
            scalar_range: [0.0, 0.0].into(),
            time_offset: 0,
            default_names_for_entities: Default::default(),
        }
    }
}

impl ViewState for TimeSeriesViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct TimeSeriesView;

type ViewType = re_types::blueprint::views::TimeSeriesView;

impl ViewClass for TimeSeriesView {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Time series"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_TIMESERIES
    }

    fn help(&self, egui_ctx: &egui::Context) -> Help {
        Help::new("Time series view")
            .docs_link("https://rerun.io/docs/reference/types/views/time_series_view")
            .control("Pan", icon_text!(icons::LEFT_MOUSE_CLICK, "+", "drag"))
            .control(
                "Zoom",
                shortcut_with_icon(egui_ctx, ZOOM_SCROLL_MODIFIER, icons::SCROLL),
            )
            .control(
                "Zoom only x-axis",
                shortcut_with_icon(egui_ctx, ASPECT_SCROLL_MODIFIER, icons::SCROLL),
            )
            .control(
                "Zoom to selection",
                icon_text!(MouseButtonText(SELECTION_RECT_ZOOM_BUTTON), "+", "drag"),
            )
            .control(
                "Move time cursor",
                icon_text!(MouseButtonText(MOVE_TIME_CURSOR_BUTTON)),
            )
            .control("Reset view", icon_text!("double", icons::LEFT_MOUSE_CLICK))
            .control_separator()
            .control(
                "Hide/show series",
                icon_text!(icons::LEFT_MOUSE_CLICK, "legend"),
            )
            .control(
                "Hide/show other series",
                icon_text!(
                    shortcut_with_icon(egui_ctx, egui::Modifiers::ALT, icons::LEFT_MOUSE_CLICK),
                    "legend"
                ),
            )
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_visualizer::<SeriesLineSystem>()?;
        system_registry.register_visualizer::<SeriesPointSystem>()?;
        Ok(())
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<TimeSeriesViewState>::default()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn ViewState) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::ViewClassLayoutPriority {
        re_viewer_context::ViewClassLayoutPriority::Low
    }

    fn supports_visible_time_range(&self) -> bool {
        true
    }

    fn default_query_range(&self, _view_state: &dyn ViewState) -> QueryRange {
        QueryRange::TimeRange(TimeRange::EVERYTHING)
    }

    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        _space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<TimeSeriesViewState>()?;

        list_item::list_item_scope(ui, "time_series_selection_ui", |ui| {
            view_property_ui::<PlotLegend>(ctx, ui, view_id, self, state);
            view_property_ui::<ScalarAxis>(ctx, ui, view_id, self, state);
        });

        Ok(())
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        suggested_filter: &ResolvedEntityPathFilter,
    ) -> ViewSpawnHeuristics {
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
        // SeriesLine is visualizable, even if not indicated.
        if let Some(maybe_visualizable) = ctx
            .maybe_visualizable_entities_per_visualizer
            .get(&SeriesLineSystem::identifier())
        {
            indicated_entities
                .0
                .extend(maybe_visualizable.iter().cloned());
        }

        // Ensure we don't modify this list anymore before we check the `suggested_filter`.
        let indicated_entities = indicated_entities;
        if indicated_entities
            .iter()
            .all(|e| suggested_filter.matches(e))
        {
            return ViewSpawnHeuristics::default();
        }

        if indicated_entities.0.is_empty() {
            return ViewSpawnHeuristics::default();
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
            return ViewSpawnHeuristics::root();
        }

        // If there's other entities that have the right indicator & didn't match the above,
        // spawn a time series view for each child of the root that has any entities with the right indicator.
        let mut child_of_root_entities = HashSet::default();
        for entity in indicated_entities.iter() {
            if let Some(child_of_root) = entity.iter().next() {
                child_of_root_entities.insert(child_of_root);
            }
        }

        ViewSpawnHeuristics::new(child_of_root_entities.into_iter().map(|path_part| {
            let entity = EntityPath::new(vec![path_part.clone()]);
            RecommendedView::new_subtree(entity)
        }))
    }

    /// Choose the default visualizers to enable for this entity.
    fn choose_default_visualizers(
        &self,
        entity_path: &EntityPath,
        _maybe_visualizable_entities_per_visualizer: &PerVisualizer<MaybeVisualizableEntities>,
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
                    .is_some_and(|matching_list| matching_list.contains(entity_path))
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
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let state = state.downcast_mut::<TimeSeriesViewState>()?;

        let blueprint_db = ctx.blueprint_db();
        let view_id = query.view_id;

        let plot_legend =
            ViewProperty::from_archetype::<PlotLegend>(blueprint_db, ctx.blueprint_query, view_id);
        let legend_visible = plot_legend.component_or_fallback::<Visible>(ctx, self, state)?;
        let legend_corner = plot_legend.component_or_fallback::<Corner2D>(ctx, self, state)?;

        let scalar_axis =
            ViewProperty::from_archetype::<ScalarAxis>(blueprint_db, ctx.blueprint_query, view_id);
        let y_range = scalar_axis.component_or_fallback::<Range1D>(ctx, self, state)?;
        let y_range = make_range_sane(y_range);

        let y_zoom_lock =
            scalar_axis.component_or_fallback::<LockRangeDuringZoom>(ctx, self, state)?;
        let y_zoom_lock = y_zoom_lock.0 .0;

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

        // Note that a several plot items can point to the same entity path and in some cases even to the same instance path!
        // (e.g. when plotting both lines & points with the same entity/instance path)
        let plot_item_id_to_instance_path: HashMap<egui::Id, InstancePath> = all_plot_series
            .iter()
            .map(|series| (series.id, series.instance_path.clone()))
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
        let time_offset = match timeline.typ() {
            TimeType::Sequence => min_time,
            TimeType::TimestampNs | TimeType::DurationNs => {
                // In order to make the tick-marks on the time axis fall on whole days, hours, minutes etc,
                // we need to round to a whole day:
                round_ns_to_start_of_day(min_time)
            }
        };
        state.time_offset = time_offset;

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
        let timestamp_format = ctx.app_options().timestamp_format;

        let plot_id = crate::plot_id(query.view_id);

        set_plot_visibility_from_store(ui.ctx(), &all_plot_series, plot_id);

        let mut plot = Plot::new(plot_id_src)
            .id(plot_id)
            .auto_bounds([true, false]) // Never use y auto bounds: we dictated bounds via blueprint under all circumstances.
            .allow_zoom([true, !lock_y_during_zoom])
            .x_axis_formatter(move |time, _| {
                format_time(
                    time_type,
                    (time.value as i64).saturating_add(time_offset),
                    timestamp_format,
                )
            })
            .y_axis_formatter(move |mark, _| format_y_axis(mark))
            .label_formatter(move |name, value| {
                let name = if name.is_empty() { "y" } else { name };
                let label = time_type.format(
                    TimeInt::new_temporal((value.x as i64).saturating_add(time_offset)),
                    timestamp_format,
                );

                let y_value = re_format::format_f64(value.y);

                if aggregator == AggregationPolicy::Off || aggregation_factor <= 1.0 {
                    format!("{timeline_name}: {label}\n{name}: {y_value}")
                } else {
                    format!(
                        "{timeline_name}: {label}\n{name}: {y_value}\n\
                        {aggregator} aggregation over approx. {aggregation_factor:.1} time points",
                    )
                }
            });

        if *legend_visible.0 {
            plot = plot.legend(Legend::default().position(legend_corner.into()));
        }

        match timeline.typ() {
            TimeType::Sequence => {}
            TimeType::DurationNs | TimeType::TimestampNs => {
                let canvas_size = ui.available_size();
                plot = plot.x_grid_spacer(move |spacer| ns_grid_spacer(canvas_size, &spacer));
            }
        }

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
            plot_ui.set_auto_bounds([
                current_auto[0] || is_resetting,
                is_resetting && !y_zoom_lock,
            ]);

            // Needed by for the visualizers' fallback provider.
            state.default_names_for_entities = EntityPath::short_names_with_disambiguation(
                all_plot_series
                    .iter()
                    .map(|series| series.instance_path.entity_path.clone())
                    // `short_names_with_disambiguation` expects no duplicate entities
                    .collect::<nohash_hasher::IntSet<_>>(),
            );

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

            add_series_to_plot(
                plot_ui,
                &query.highlights,
                &all_plot_series,
                time_offset,
                &mut state.scalar_range,
            );
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
            .map(|current_time| (current_time.saturating_sub(time_offset)) as f64)
            .filter(|&x| {
                // only display the time cursor when it's actually above the plot area
                transform.bounds().min()[0] <= x && x <= transform.bounds().max()[0]
            })
            .map(|x| transform.position_from_point(&PlotPoint::new(x, 0.0)).x);

        // If we are not resetting on this frame, interact with the plot items (lines, scatters, etc.)
        if !is_resetting {
            if let Some(hovered) = hovered_plot_item
                .and_then(|hovered_plot_item| plot_item_id_to_instance_path.get(&hovered_plot_item))
                .map(|entity_path| {
                    re_viewer_context::Item::DataResult(query.view_id, entity_path.clone())
                })
                .or_else(|| {
                    if response.hovered() {
                        Some(re_viewer_context::Item::View(query.view_id))
                    } else {
                        None
                    }
                })
            {
                ctx.handle_select_hover_drag_interactions(&response, hovered, false);
            }
        }

        // Sync visibility of hidden items with the blueprint (user can hide items via the legend).
        update_series_visibility_overrides_from_plot(
            ctx,
            query,
            &all_plot_series,
            ui.ctx(),
            plot_id,
        );

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

fn set_plot_visibility_from_store(
    egui_ctx: &egui::Context,
    plot_series_from_store: &[&crate::PlotSeries],
    plot_id: egui::Id,
) {
    // egui_plot has its own memory about which plots are visible.
    // We want to store that state in blueprint, so overwrite it (we sync with any changes that ui interaction may do later on, see `update_series_visibility`)
    if let Some(mut plot_memory) = egui_plot::PlotMemory::load(egui_ctx, plot_id) {
        plot_memory.hidden_items = plot_series_from_store
            .iter()
            .filter(|&series| !series.visible)
            .map(|series| series.id)
            .collect();
        plot_memory.store(egui_ctx, plot_id);
    }
}

fn update_series_visibility_overrides_from_plot(
    ctx: &ViewerContext<'_>,
    query: &ViewQuery<'_>,
    all_plot_series: &[&crate::PlotSeries],
    egui_ctx: &egui::Context,
    plot_id: egui::Id,
) {
    let Some(query_results) = ctx.query_results.get(&query.view_id) else {
        return;
    };
    let Some(plot_memory) = egui_plot::PlotMemory::load(egui_ctx, plot_id) else {
        return;
    };
    let hidden_items = plot_memory.hidden_items;

    // Determine which series have changed visibility state.
    let mut per_entity_series_new_visibility_state: HashMap<EntityPath, SmallVec<[bool; 1]>> =
        HashMap::default();
    let mut series_to_update = Vec::new();
    for series in all_plot_series {
        let entity_visibility_flags = per_entity_series_new_visibility_state
            .entry(series.instance_path.entity_path.clone())
            .or_default();

        let visible_new = !hidden_items.contains(&series.id);

        let instance = series.instance_path.instance;
        let index = instance.specific_index().map_or(0, |i| i.get() as usize);
        if entity_visibility_flags.len() <= index {
            entity_visibility_flags.resize(index + 1, false);
        }
        entity_visibility_flags[index] = visible_new;

        if series.visible != visible_new {
            series_to_update.push(series);
        }
    }

    for series in series_to_update {
        let Some(visibility_state) =
            per_entity_series_new_visibility_state.remove(&series.instance_path.entity_path)
        else {
            continue;
        };
        let Some(result) = query_results.result_for_entity(&series.instance_path.entity_path)
        else {
            continue;
        };

        let override_path = result.override_path();
        let descriptor = match series.kind {
            PlotSeriesKind::Continuous => Some(SeriesLine::descriptor_visible_series()),
            PlotSeriesKind::Scatter(_) => Some(SeriesPoint::descriptor_visible_series()),
            PlotSeriesKind::Clear => {
                if cfg!(debug_assertions) {
                    unreachable!(
                        "Clear series can't be hidden since it doesn't show in the first place"
                    );
                }
                None
            }
        };

        let component_array = visibility_state
            .into_iter()
            .map(SeriesVisible::from)
            .collect::<Vec<_>>();

        if let Some(serialized_component_batch) = descriptor.and_then(|descriptor| {
            component_array
                .serialized()
                .map(|serialized| serialized.with_descriptor_override(descriptor))
        }) {
            ctx.save_serialized_blueprint_component(override_path, serialized_component_batch);
        }
    }
}

fn add_series_to_plot(
    plot_ui: &mut egui_plot::PlotUi<'_>,
    highlights: &ViewHighlights,
    all_plot_series: &[&crate::PlotSeries],
    time_offset: i64,
    scalar_range: &mut Range1D,
) {
    re_tracing::profile_function!();

    *scalar_range.start_mut() = f64::INFINITY;
    *scalar_range.end_mut() = f64::NEG_INFINITY;

    for series in all_plot_series {
        let points = series
            .points
            .iter()
            .map(|p| {
                if p.1 < scalar_range.start() {
                    *scalar_range.start_mut() = p.1;
                }
                if p.1 > scalar_range.end() {
                    *scalar_range.end_mut() = p.1;
                }

                [(p.0 - time_offset) as _, p.1]
            })
            .collect::<Vec<_>>();

        let color = series.color;

        let interaction_highlight = highlights
            .entity_highlight(series.instance_path.entity_path.hash())
            .index_highlight(series.instance_path.instance);
        let highlight = interaction_highlight.any();

        match series.kind {
            PlotSeriesKind::Continuous => plot_ui.line(
                Line::new(&series.label, points)
                    .color(color)
                    .width(2.0 * series.radius_ui)
                    .highlight(highlight)
                    .id(series.id),
            ),
            PlotSeriesKind::Scatter(scatter_attrs) => plot_ui.points(
                Points::new(&series.label, points)
                    .color(color)
                    .radius(series.radius_ui)
                    .shape(scatter_attrs.marker.into())
                    .highlight(highlight)
                    .id(series.id),
            ),
            // Break up the chart. At some point we might want something fancier.
            PlotSeriesKind::Clear => {}
        }
    }
}

fn format_time(time_type: TimeType, time_int: i64, timestamp_format: TimestampFormat) -> String {
    match time_type {
        TimeType::DurationNs | TimeType::TimestampNs => {
            let time = re_log_types::Time::from_ns_since_epoch(time_int);
            time.format_time_compact(timestamp_format)
        }
        TimeType::Sequence => time_type.format(TimeInt::new_temporal(time_int), timestamp_format),
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

impl TypedComponentFallbackProvider<Corner2D> for TimeSeriesView {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> Corner2D {
        // Explicitly pick RightCorner2D::RightBottom, we don't want to make this dependent on the (arbitrary)
        // default of Corner2D
        Corner2D::RightBottom
    }
}

impl TypedComponentFallbackProvider<Range1D> for TimeSeriesView {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> Range1D {
        ctx.view_state
            .as_any()
            .downcast_ref::<TimeSeriesViewState>()
            .map(|s| make_range_sane(s.scalar_range))
            .unwrap_or_default()
    }
}

/// Make sure the range is finite and positive, or `egui_plot` might be buggy.
fn make_range_sane(y_range: Range1D) -> Range1D {
    let (mut start, mut end) = (y_range.start(), y_range.end());

    if !start.is_finite() {
        start = -1.0;
    }
    if !end.is_finite() {
        end = 1.0;
    }

    if end < start {
        (start, end) = (end, start);
    }

    if end <= start {
        let center = (start + end) / 2.0;
        Range1D::new(center - 1.0, center + 1.0)
    } else {
        Range1D::new(start, end)
    }
}

re_viewer_context::impl_component_fallback_provider!(TimeSeriesView => [Corner2D, Range1D]);

#[test]
fn test_help_view() {
    re_viewer_context::test_context::TestContext::test_help_view(|ctx| TimeSeriesView.help(ctx));
}
