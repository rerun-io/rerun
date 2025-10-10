use egui::{
    NumExt as _, Vec2,
    ahash::{HashMap, HashSet},
};
use egui_plot::{ColorConflictHandling, Legend, Line, Plot, PlotPoint, Points};
use nohash_hasher::IntSet;
use re_viewer_context::{
    BlueprintContext as _, IdentifiedViewSystem as _, IndicatedEntities, MaybeVisualizableEntities,
    PerVisualizer, QueryRange, RecommendedView, SmallVisualizerSet, SystemExecutionOutput,
    TimeControlCommand, ViewClass, ViewClassExt as _, ViewClassRegistryError, ViewHighlights,
    ViewId, ViewQuery, ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError,
    ViewSystemIdentifier, ViewerContext, VisualizableEntities,
    external::re_entity_db::InstancePath,
};
use re_viewport_blueprint::ViewProperty;
use smallvec::SmallVec;

use crate::{
    PlotSeriesKind, line_visualizer_system::SeriesLinesSystem,
    point_visualizer_system::SeriesPointsSystem,
};
use re_chunk_store::TimeType;
use re_format::time::next_grid_tick_magnitude_nanos;
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt};
use re_types::{
    ComponentBatch as _, View as _, ViewClassIdentifier,
    archetypes::{SeriesLines, SeriesPoints},
    blueprint::{
        archetypes::{PlotBackground, PlotLegend, ScalarAxis, TimeAxis},
        components::{Corner2D, Enabled, LinkAxis, LockRangeDuringZoom},
    },
    components::{AggregationPolicy, Color, Range1D, SeriesVisible, Visible},
    datatypes::TimeRange,
};
use re_ui::{Help, IconText, MouseButtonText, UiExt as _, icons, list_item};
use re_view::{
    controls::{MOVE_TIME_CURSOR_BUTTON, SELECTION_RECT_ZOOM_BUTTON},
    view_property_ui,
};

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

    /// The size of the current range of time which covers the whole time series.
    max_time_view_range: AbsoluteTimeRange,

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

    /// Whether to reset the plot bounds next frame.
    reset_bounds_next_frame: bool,
}

impl Default for TimeSeriesViewState {
    fn default() -> Self {
        Self {
            is_dragging_time_cursor: false,
            was_dragging_time_cursor: false,
            saved_auto_bounds: egui::Vec2b {
                // Default x bounds to automatically show all time values.
                x: true,
                // Never use y auto bounds: we dictated bounds via blueprint under all circumstances.
                y: false,
            },
            scalar_range: [0.0, 0.0].into(),
            max_time_view_range: AbsoluteTimeRange::EMPTY,
            time_offset: 0,
            default_names_for_entities: Default::default(),
            reset_bounds_next_frame: false,
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

    fn help(&self, os: egui::os::OperatingSystem) -> Help {
        let egui::InputOptions {
            zoom_modifier,
            horizontal_scroll_modifier,
            vertical_scroll_modifier,
            ..
        } = egui::InputOptions::default(); // This is OK, since we don't allow the user to change these modifiers.

        Help::new("Time series view")
            .docs_link("https://rerun.io/docs/reference/types/views/time_series_view")
            .control("Pan", (icons::LEFT_MOUSE_CLICK, "+", "drag"))
            .control(
                "Horizontal pan",
                IconText::from_modifiers_and(os, horizontal_scroll_modifier, icons::SCROLL),
            )
            .control(
                "Zoom",
                IconText::from_modifiers_and(os, zoom_modifier, icons::SCROLL),
            )
            .control(
                "Zoom X-axis",
                IconText::from_modifiers_and(
                    os,
                    zoom_modifier | horizontal_scroll_modifier,
                    icons::SCROLL,
                ),
            )
            .control(
                "Zoom Y-axis",
                IconText::from_modifiers_and(
                    os,
                    zoom_modifier | vertical_scroll_modifier,
                    icons::SCROLL,
                ),
            )
            .control(
                "Zoom to selection",
                (MouseButtonText(SELECTION_RECT_ZOOM_BUTTON), "+", "drag"),
            )
            .control("Move time cursor", MouseButtonText(MOVE_TIME_CURSOR_BUTTON))
            .control("Reset view", ("double", icons::LEFT_MOUSE_CLICK))
            .control_separator()
            .control("Hide/show series", (icons::LEFT_MOUSE_CLICK, "legend"))
            .control(
                "Hide/show other series",
                (
                    IconText::from_modifiers_and(os, egui::Modifiers::ALT, icons::LEFT_MOUSE_CLICK),
                    "legend",
                ),
            )
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        for component in [
            SeriesLines::descriptor_names().component,
            SeriesPoints::descriptor_names().component,
        ] {
            system_registry.register_fallback_provider::<re_types::components::Name>(
                component,
                |ctx| {
                    let state = ctx.view_state().downcast_ref::<TimeSeriesViewState>();

                    state
                        .ok()
                        .and_then(|state| {
                            state
                                .default_names_for_entities
                                .get(ctx.target_entity_path)
                                .map(|name| name.clone().into())
                        })
                        .or_else(|| {
                            ctx.target_entity_path
                                .last()
                                .map(|part| part.ui_string().into())
                        })
                        .unwrap_or_default()
                },
            );
        }
        system_registry.register_fallback_provider(
            ScalarAxis::descriptor_range().component,
            |ctx| {
                ctx.view_state()
                    .as_any()
                    .downcast_ref::<TimeSeriesViewState>()
                    .map(|s| make_range_sane(s.scalar_range))
                    .unwrap_or_default()
            },
        );
        system_registry.register_fallback_provider(
            TimeAxis::descriptor_view_range().component,
            |ctx| {
                let (timeline_min, timeline_max) = ctx
                    .viewer_ctx()
                    .recording()
                    .times_per_timeline()
                    .get(ctx.viewer_ctx().time_ctrl.timeline().name())
                    .and_then(|stats| {
                        Some((
                            *stats.per_time.keys().next()?,
                            *stats.per_time.keys().next_back()?,
                        ))
                    })
                    .unzip();
                ctx.view_state()
                    .as_any()
                    .downcast_ref::<TimeSeriesViewState>()
                    .map(|s| {
                        re_types::blueprint::components::TimeRange(TimeRange {
                            start: if Some(s.max_time_view_range.min) == timeline_min {
                                re_types::datatypes::TimeRangeBoundary::Infinite
                            } else {
                                re_types::datatypes::TimeRangeBoundary::Absolute(
                                    s.max_time_view_range.min.into(),
                                )
                            },
                            end: if Some(s.max_time_view_range.max) == timeline_max {
                                re_types::datatypes::TimeRangeBoundary::Infinite
                            } else {
                                re_types::datatypes::TimeRangeBoundary::Absolute(
                                    s.max_time_view_range.max.into(),
                                )
                            },
                        })
                    })
                    .unwrap_or(re_types::blueprint::components::TimeRange(TimeRange {
                        start: re_types::datatypes::TimeRangeBoundary::Infinite,
                        end: re_types::datatypes::TimeRangeBoundary::Infinite,
                    }))
            },
        );

        system_registry.register_visualizer::<SeriesLinesSystem>()?;
        system_registry.register_visualizer::<SeriesPointsSystem>()?;
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
            let ctx = self.view_context(ctx, view_id, state);
            view_property_ui::<PlotBackground>(&ctx, ui);
            view_property_ui::<PlotLegend>(&ctx, ui);
            view_property_ui::<TimeAxis>(&ctx, ui);
            view_property_ui::<ScalarAxis>(&ctx, ui);
        });

        Ok(())
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> ViewSpawnHeuristics {
        re_tracing::profile_function!();

        // For all following lookups, checking indicators is enough, since we know that this is enough to infer visualizability here.
        let mut indicated_entities = IndicatedEntities::default();

        for indicated in [
            SeriesLinesSystem::identifier(),
            SeriesPointsSystem::identifier(),
        ]
        .iter()
        .filter_map(|&system_id| ctx.indicated_entities_per_visualizer.get(&system_id))
        {
            indicated_entities.0.extend(indicated.0.iter().cloned());
        }

        // Because SeriesLines is our fallback visualizer, also include any entities for which
        // SeriesLines is visualizable, even if not indicated.
        if let Some(maybe_visualizable) = ctx
            .maybe_visualizable_entities_per_visualizer
            .get(&SeriesLinesSystem::identifier())
        {
            indicated_entities
                .0
                .extend(maybe_visualizable.iter().cloned());
        }

        // Ensure we don't modify this list anymore before we check the `include_entity`.
        let indicated_entities = indicated_entities;

        if !indicated_entities.iter().any(include_entity) {
            return ViewSpawnHeuristics::empty();
        }

        // Spawn time series data at the root if theres 'either:
        // * time series data directly at the root
        // * all time series data are direct children of the root
        //
        // This heuristic was last edited in 2015-04-11 by @emilk,
        // because it was triggering too often (https://github.com/rerun-io/rerun/pull/9587).
        // Maybe we should remove it completely?
        // I have a feeling it was added to handle the case many scalars of the form `/x`, `/y`, `/z` etc,
        // but we now support logging multiple scalars in one entity.
        //
        // This is the last hold out of "child of root" spawning, which we removed otherwise
        // (see https://github.com/rerun-io/rerun/issues/4926)
        let root_entities: IntSet<EntityPath> = ctx
            .recording()
            .tree()
            .children
            .values()
            .map(|subtree| subtree.path.clone())
            .collect();
        if indicated_entities.contains(&EntityPath::root())
            || indicated_entities.is_subset(&root_entities)
        {
            return ViewSpawnHeuristics::root();
        }

        // If there's other entities that have the right indicator & didn't match the above,
        // spawn a time series view for each child of the root that has any entities with the right indicator.
        let mut child_of_root_entities = HashSet::default();
        #[expect(clippy::iter_over_hash_type)]
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
        if visualizers.is_empty()
            && available_visualizers.contains(&SeriesLinesSystem::identifier())
        {
            visualizers.insert(0, SeriesLinesSystem::identifier());
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

        let line_series = system_output.view_systems.get::<SeriesLinesSystem>()?;
        let point_series = system_output.view_systems.get::<SeriesPointsSystem>()?;

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

        let current_time = ctx.time_ctrl.time_i64();
        let time_type = ctx.time_ctrl.time_type();
        let timeline = *ctx.time_ctrl.timeline();

        let timeline_name = timeline.name().to_string();

        // Get the minimum time/X value for the entire plot…
        let min_time = all_plot_series
            .iter()
            .map(|line| line.min_time)
            .min()
            .unwrap_or(0);

        // …then use that as an offset to avoid nasty precision issues with
        // large times (nanos since epoch does not fit into a f64).
        let time_offset = match timeline.typ() {
            TimeType::Sequence => min_time,
            TimeType::TimestampNs | TimeType::DurationNs => {
                // In order to make the tick-marks on the time axis fall on whole days, hours, minutes etc,
                // we need to round to a whole day:
                round_nanos_to_start_of_day(min_time)
            }
        };
        state.time_offset = time_offset;

        // Get the maximum time/X value for the entire plot
        let max_time = all_plot_series
            .iter()
            .map(|line| line.points.last().map(|(t, _)| *t).unwrap_or(line.min_time))
            .max()
            .unwrap_or(0);

        let timeline_start = ctx
            .recording()
            .time_histogram(ctx.time_ctrl.timeline().name())
            .and_then(|times| times.min_key())
            .unwrap_or_default();

        let timeline_end = ctx
            .recording()
            .time_histogram(ctx.time_ctrl.timeline().name())
            .and_then(|times| times.max_key())
            .unwrap_or_default();

        state.max_time_view_range = AbsoluteTimeRange::new(
            TimeInt::saturated_temporal_i64(min_time),
            TimeInt::saturated_temporal_i64(max_time),
        );

        let blueprint_db = ctx.blueprint_db();
        let view_id = query.view_id;

        let view_ctx = self.view_context(ctx, view_id, state);
        let background = ViewProperty::from_archetype::<PlotBackground>(
            blueprint_db,
            ctx.blueprint_query,
            view_id,
        );
        let background_color = background.component_or_fallback::<Color>(
            &view_ctx,
            PlotBackground::descriptor_color().component,
        )?;
        let show_grid = background.component_or_fallback::<Enabled>(
            &view_ctx,
            PlotBackground::descriptor_show_grid().component,
        )?;

        let plot_legend =
            ViewProperty::from_archetype::<PlotLegend>(blueprint_db, ctx.blueprint_query, view_id);
        let legend_visible = plot_legend.component_or_fallback::<Visible>(
            &view_ctx,
            PlotLegend::descriptor_visible().component,
        )?;
        let legend_corner = plot_legend.component_or_fallback::<Corner2D>(
            &view_ctx,
            PlotLegend::descriptor_corner().component,
        )?;

        let time_axis =
            ViewProperty::from_archetype::<TimeAxis>(blueprint_db, ctx.blueprint_query, view_id);
        let link_x_axis = time_axis
            .component_or_fallback::<LinkAxis>(&view_ctx, TimeAxis::descriptor_link().component)?;

        let view_current_time =
            re_types::datatypes::TimeInt(current_time.unwrap_or_default().at_least(timeline_start));

        let view_time_range = time_axis
            .component_or_fallback::<re_types::blueprint::components::TimeRange>(
                &view_ctx,
                TimeAxis::descriptor_view_range().component,
            )?;
        let x_range = make_range_sane(Range1D::new(
            (match view_time_range.start {
                re_types::datatypes::TimeRangeBoundary::Infinite => timeline_start,
                _ => {
                    view_time_range
                        .start
                        .start_boundary_time(view_current_time)
                        .0
                }
            } - time_offset) as f64,
            (match view_time_range.end {
                re_types::datatypes::TimeRangeBoundary::Infinite => timeline_end,
                _ => view_time_range.end.end_boundary_time(view_current_time).0,
            } - time_offset) as f64,
        ));

        let scalar_axis =
            ViewProperty::from_archetype::<ScalarAxis>(blueprint_db, ctx.blueprint_query, view_id);
        let y_range = scalar_axis.component_or_fallback::<Range1D>(
            &view_ctx,
            ScalarAxis::descriptor_range().component,
        )?;
        let y_range = make_range_sane(y_range);

        let y_zoom_lock = scalar_axis.component_or_fallback::<LockRangeDuringZoom>(
            &view_ctx,
            ScalarAxis::descriptor_zoom_lock().component,
        )?;
        let y_zoom_lock = y_zoom_lock.0.0;

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
                round_nanos_to_start_of_day(min_time)
            }
        };
        state.time_offset = time_offset;

        let lock_y_during_zoom = y_zoom_lock;

        // TODO(#5075): Boxed-zoom should be fixed to accommodate the locked range.
        let timestamp_format = ctx.app_options().timestamp_format;

        let plot_id = crate::plot_id(query.view_id);

        set_plot_visibility_from_store(ui.ctx(), &all_plot_series, plot_id);

        let min_axis_thickness = ui.tokens().small_icon_size.y;

        ui.scope(|ui| {
            // use timeline_name as part of id, so that egui stores different pan/zoom for different timelines
            let plot_id_src = ("plot", &timeline_name);

            ui.style_mut().visuals.extreme_bg_color = background_color.into();

            let mut plot = Plot::new(plot_id_src)
                .id(plot_id)
                .show_grid(**show_grid)
                .auto_bounds(state.saved_auto_bounds) // Note that this only sets the initial default.
                .allow_zoom(false)
                .allow_scroll(false)
                .allow_drag(false)
                .custom_x_axes(vec![
                    egui_plot::AxisHints::new_x()
                        .min_thickness(min_axis_thickness)
                        .formatter(move |time, _| {
                            re_log_types::TimeCell::new(
                                time_type,
                                (time.value as i64).saturating_add(time_offset),
                            )
                            .format_compact(timestamp_format)
                        }),
                ])
                .custom_y_axes(vec![
                    egui_plot::AxisHints::new_y()
                        .min_thickness(min_axis_thickness)
                        .formatter(move |mark, _| format_y_axis(mark)),
                ])
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

            if state.reset_bounds_next_frame {
                plot = plot.reset();
            }

            match link_x_axis {
                LinkAxis::Independent => {}
                LinkAxis::LinkToGlobal => {
                    plot = plot.link_axis(timeline.name().as_str(), [true, false]);
                }
            }

            // Sharing the same cursor is always nice:
            plot = plot.link_cursor(timeline.name().as_str(), [true; 2]);

            if *legend_visible.0 {
                plot = plot.legend(
                    Legend::default()
                        .position(legend_corner.into())
                        .color_conflict_handling(ColorConflictHandling::PickFirst),
                );
            }

            match timeline.typ() {
                TimeType::Sequence => {}
                TimeType::DurationNs | TimeType::TimestampNs => {
                    let canvas_size = ui.available_size();
                    plot =
                        plot.x_grid_spacer(move |spacer| nanos_grid_spacer(canvas_size, &spacer));
                }
            }

            let mut plot_double_clicked = false;
            let egui_plot::PlotResponse {
                inner: _,
                response,
                mut transform,
                hovered_plot_item,
            } = plot.show(ui, |plot_ui| {
                if plot_ui.response().secondary_clicked()
                    && let Some(pointer) = plot_ui.pointer_coordinate()
                {
                    let time = re_log_types::TimeReal::from(pointer.x as i64 + time_offset);
                    ctx.send_time_commands([
                        TimeControlCommand::SetTime(time),
                        TimeControlCommand::Pause,
                    ]);
                }

                plot_double_clicked = plot_ui.response().double_clicked();

                // Let the user pick x and y ranges from the blueprint:
                plot_ui.set_plot_bounds_y(y_range);
                plot_ui.set_plot_bounds_x(x_range);

                // Needed by for the visualizers' fallback provider.
                state.default_names_for_entities = EntityPath::short_names_with_disambiguation(
                    all_plot_series
                        .iter()
                        .map(|series| series.instance_path.entity_path.clone())
                        // `short_names_with_disambiguation` expects no duplicate entities
                        .collect::<nohash_hasher::IntSet<_>>(),
                );

                // if state.is_dragging_time_cursor {
                //     if !state.was_dragging_time_cursor {
                //         state.saved_auto_bounds = plot_ui.auto_bounds();
                //     }
                //     // Freeze any change to the plot boundaries to avoid weird interaction with the time cursor.
                //     plot_ui.set_auto_bounds([false, false]);
                // } else if state.was_dragging_time_cursor {
                //     plot_ui.set_auto_bounds(state.saved_auto_bounds);
                // } else {
                //     plot_ui.set_auto_bounds([
                //         // X bounds are handled by egui plot - either to auto or manually controlled.
                //         state.reset_bounds_next_frame
                //             || (plot_ui.auto_bounds().x && link_x_axis == LinkAxis::Independent),
                //         // Y bounds are always handled by the blueprint.
                //         false,
                //     ]);
                // }

                state.reset_bounds_next_frame = false;
                state.was_dragging_time_cursor = state.is_dragging_time_cursor;

                add_series_to_plot(
                    plot_ui,
                    &query.highlights,
                    &all_plot_series,
                    time_offset,
                    &mut state.scalar_range,
                );
            });

            // Interact with the plot items (lines, scatters, etc.)
            let hovered_data_result = hovered_plot_item
                .and_then(|hovered_plot_item| plot_item_id_to_instance_path.get(&hovered_plot_item))
                .map(|instance_path| {
                    re_viewer_context::Item::DataResult(query.view_id, instance_path.clone())
                });
            if let Some(hovered) = hovered_data_result.clone().or_else(|| {
                if response.hovered() {
                    Some(re_viewer_context::Item::View(query.view_id))
                } else {
                    None
                }
            }) {
                ctx.handle_select_hover_drag_interactions(&response, hovered, false);
            }

            // Can determine whether we're resetting only now since we need to know whether there's a plot item hovered.
            let is_resetting = plot_double_clicked && hovered_data_result.is_none();

            if is_resetting {
                scalar_axis.reset_static_blueprint_component(ctx, ScalarAxis::descriptor_range());
                time_axis.reset_static_blueprint_component(ctx, TimeAxis::descriptor_view_range());

                state.reset_bounds_next_frame = true;
                ui.ctx().request_repaint(); // Make sure we get another frame with the reset actually applied.
            } else {
                // We manually handle inputs for the plot to better interact with blueprints.
                let drag_delta = if response.dragged_by(egui::PointerButton::Primary) {
                    response.drag_delta()
                } else {
                    egui::Vec2::ZERO
                };
                let (scroll_delta, zoom_delta) =
                    ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta_2d()));

                let move_delta = drag_delta + scroll_delta;

                if let Some(hover_pos) = response.hover_pos()
                    && (move_delta != egui::Vec2::ZERO || zoom_delta != egui::Vec2::ONE)
                {
                    transform.translate_bounds((-move_delta.x as f64, -move_delta.y as f64));

                    transform.zoom(zoom_delta, hover_pos);

                    let new_x_range = transform.bounds().range_x();
                    let new_x_range = Range1D::new(*new_x_range.start(), *new_x_range.end());

                    let new_view_time_range =
                        re_types::blueprint::components::TimeRange(TimeRange {
                            start: re_types::datatypes::TimeRangeBoundary::Absolute(
                                re_types::datatypes::TimeInt(
                                    new_x_range.start() as i64 + time_offset,
                                ),
                            ),
                            end: re_types::datatypes::TimeRangeBoundary::Absolute(
                                re_types::datatypes::TimeInt(
                                    new_x_range.end() as i64 + time_offset,
                                ),
                            ),
                        });

                    if new_x_range != x_range && view_time_range != new_view_time_range {
                        time_axis.save_static_blueprint_component(
                            ctx,
                            &TimeAxis::descriptor_view_range(),
                            &new_view_time_range,
                        );
                        ui.ctx().request_repaint(); // Make sure we get another frame with this new range applied.
                    }

                    let new_y_range = transform.bounds().range_y();
                    let new_y_range = Range1D::new(*new_y_range.start(), *new_y_range.end());

                    // Write new y_range if it has changed.
                    if new_y_range != y_range {
                        scalar_axis.save_static_blueprint_component(
                            ctx,
                            &ScalarAxis::descriptor_range(),
                            &new_y_range,
                        );
                        ui.ctx().request_repaint(); // Make sure we get another frame with this new range applied.
                    }
                }
                // let move_delta = drag_delta + scroll_delta;

                // let new_range = |range: Range1D, move_delta, zoom_delta| {
                //     let plot_move_delta = dpos_dvalue * move_delta;
                //     let new_size = range.abs_len() * zoom_delta;
                //     let new_center = (range.start() + range.end()) * 0.5 + plot_move_delta;
                //     Range1D::new(new_center - new_size * 0.5, new_center + new_size * 0.5)
                // };

                // let new_x_range = new_range(
                //     x_range,
                //     move_delta.x as f64,
                //     zoom_delta.x as f64,
                // );
                // let new_y_range = new_range(
                //     y_range,
                //     move_delta.y as f64,
                //     zoom_delta.y as f64,
                // );
            }

            // Decide if the time cursor should be displayed, and if so where:
            let time_x = current_time
                .map(|current_time| (current_time.saturating_sub(time_offset)) as f64)
                .filter(|&x| {
                    // only display the time cursor when it's actually above the plot area
                    transform.bounds().min()[0] <= x && x <= transform.bounds().max()[0]
                })
                .map(|x| transform.position_from_point(&PlotPoint::new(x, 0.0)).x);

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
                let line_rect =
                    egui::Rect::from_x_y_ranges(time_x..=time_x, response.rect.y_range())
                        .expand(interact_radius);

                let time_drag_id = ui.id().with("time_drag");
                let response = ui
                    .interact(line_rect, time_drag_id, egui::Sense::drag())
                    .on_hover_and_drag_cursor(egui::CursorIcon::ResizeHorizontal);

                state.is_dragging_time_cursor = false;
                if response.dragged()
                    && let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos())
                {
                    let aim_radius = ui.input(|i| i.aim_radius());
                    let new_offset_time = egui::emath::smart_aim::best_in_range_f64(
                        transform
                            .value_from_position(pointer_pos - aim_radius * Vec2::X)
                            .x,
                        transform
                            .value_from_position(pointer_pos + aim_radius * Vec2::X)
                            .x,
                    );
                    let new_time = time_offset + new_offset_time.round() as i64;

                    // Avoid frame-delay:
                    time_x = pointer_pos.x;

                    ctx.send_time_commands([
                        TimeControlCommand::SetTime(new_time.into()),
                        TimeControlCommand::Pause,
                    ]);

                    state.is_dragging_time_cursor = true;
                }

                ui.paint_time_cursor(ui.painter(), &response, time_x, response.rect.y_range());
            }

            Ok(())
        })
        .inner
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
            PlotSeriesKind::Continuous => Some(SeriesLines::descriptor_visible_series()),
            PlotSeriesKind::Scatter(_) => Some(SeriesPoints::descriptor_visible_series()),
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

        if let Some(serialized_component_batch) =
            descriptor.and_then(|descriptor| component_array.serialized(descriptor))
        {
            ctx.save_serialized_blueprint_component(
                override_path.clone(),
                serialized_component_batch,
            );
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
        let points = if series.visible {
            series
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
                .collect::<Vec<_>>()
        } else {
            // TODO(emilk/egui_plot#92): Note we still need to produce a series, so it shows up in the legend.
            // As of writing, egui_plot gets confused if this is an empty series, so
            // we still add a single point (but don't have it influence the scalar range!)
            series
                .points
                .first()
                .map(|p| vec![[(p.0 - time_offset) as _, p.1]])
                .unwrap_or_default()
        };

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

fn format_y_axis(mark: egui_plot::GridMark) -> String {
    // Example: If the step to the next tick is `0.01`, we should use 2 decimals of precision:
    let num_decimals = -mark.step_size.log10().round() as usize;

    re_format::FloatFormatOptions::DEFAULT_f64
        .with_decimals(num_decimals)
        .format(mark.value)
}

fn nanos_grid_spacer(
    canvas_size: egui::Vec2,
    input: &egui_plot::GridInput,
) -> Vec<egui_plot::GridMark> {
    let minimum_medium_line_spacing = 150.0; // ≈min size of a label
    let max_medium_lines = canvas_size.x as f64 / minimum_medium_line_spacing;

    let (min_nanos, max_nanos) = input.bounds;
    let width_nanos = max_nanos - min_nanos;

    let mut small_spacing_nanos = 1;
    while width_nanos / (next_grid_tick_magnitude_nanos(small_spacing_nanos) as f64)
        > max_medium_lines
    {
        let next_nanos = next_grid_tick_magnitude_nanos(small_spacing_nanos);
        if small_spacing_nanos < next_nanos {
            small_spacing_nanos = next_nanos;
        } else {
            break; // we've reached the max
        }
    }
    let medium_spacing_nanos = next_grid_tick_magnitude_nanos(small_spacing_nanos);
    let big_spacing_nanos = next_grid_tick_magnitude_nanos(medium_spacing_nanos);

    let mut current_nanos = (min_nanos.floor() as i64) / small_spacing_nanos * small_spacing_nanos;
    let mut marks = vec![];

    while current_nanos <= max_nanos.ceil() as i64 {
        let is_big_line = current_nanos % big_spacing_nanos == 0;
        let is_medium_line = current_nanos % medium_spacing_nanos == 0;

        let step_size = if is_big_line {
            big_spacing_nanos
        } else if is_medium_line {
            medium_spacing_nanos
        } else {
            small_spacing_nanos
        };

        marks.push(egui_plot::GridMark {
            value: current_nanos as f64,
            step_size: step_size as f64,
        });

        if let Some(new_nanos) = current_nanos.checked_add(small_spacing_nanos) {
            current_nanos = new_nanos;
        } else {
            break;
        }
    }

    marks
}

fn round_nanos_to_start_of_day(ns: i64) -> i64 {
    let nanos_per_day = 24 * 60 * 60 * 1_000_000_000;
    (ns.saturating_add(nanos_per_day / 2)) / nanos_per_day * nanos_per_day
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
        let center = f64::midpoint(start, end);
        Range1D::new(center - 1.0, center + 1.0)
    } else {
        Range1D::new(start, end)
    }
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| TimeSeriesView.help(ctx));
}
