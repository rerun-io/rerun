use std::collections::BTreeMap;

use arrayvec::ArrayVec;
use egui::ahash::HashMap;
use egui::{NumExt as _, Vec2, Vec2b};
use egui_plot::{ColorConflictHandling, Legend, Line, Plot, PlotPoint, Points};
use itertools::{Either, Itertools as _};
use nohash_hasher::IntSet;
use re_chunk_store::TimeType;
use re_format::time::next_grid_tick_magnitude_nanos;
use re_log_types::external::arrow::datatypes::DataType;
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt};
use re_sdk_types::archetypes::{Scalars, SeriesLines, SeriesPoints};
use re_sdk_types::blueprint::archetypes::{
    ActiveVisualizers, PlotBackground, PlotLegend, ScalarAxis, TimeAxis,
};
use re_sdk_types::blueprint::components::{
    Corner2D, Enabled, LinkAxis, LockRangeDuringZoom, VisualizerInstructionId,
};
use re_sdk_types::components::{AggregationPolicy, Color, Name, Range1D, SeriesVisible, Visible};
use re_sdk_types::datatypes::TimeRange;
use re_sdk_types::{
    ComponentBatch as _, ComponentIdentifier, Loggable as _, View as _, ViewClassIdentifier,
};
use re_ui::{Help, IconText, MouseButtonText, UiExt as _, icons, list_item};
use re_view::controls::{MOVE_TIME_CURSOR_BUTTON, SELECTION_RECT_ZOOM_BUTTON};
use re_view::view_property_ui;
use re_viewer_context::external::re_entity_db::InstancePath;
use re_viewer_context::{
    BlueprintContext as _, DatatypeMatch, IdentifiedViewSystem as _, IndicatedEntities, Item,
    PerVisualizerType, PerVisualizerTypeInViewClass, QueryRange, RecommendedView,
    RecommendedVisualizers, SystemCommandSender as _, SystemExecutionOutput, TimeControlCommand,
    ViewClass, ViewClassExt as _, ViewClassRegistryError, ViewHighlights, ViewId, ViewQuery,
    ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError,
    ViewSystemIdentifier, ViewerContext, VisualizableEntities, VisualizableReason,
    VisualizerComponentMappings, VisualizerComponentSource,
};
use re_viewport_blueprint::ViewProperty;
use smallvec::SmallVec;
use vec1::Vec1;

use crate::PlotSeriesKind;
use crate::line_visualizer_system::SeriesLinesSystem;
use crate::naming::{SeriesInfo, SeriesNamesContext};
use crate::point_visualizer_system::SeriesPointsSystem;

// ---

/// We only show this many colors directly.
const NUM_SHOWN_VISUALIZER_COLORS: usize = 2;

#[derive(Clone)]
pub struct TimeSeriesViewState {
    /// The range of the scalar values currently on screen.
    pub(crate) scalar_range: Range1D,

    /// The size of the current range of time which covers the whole time series.
    pub(crate) max_time_view_range: AbsoluteTimeRange,

    /// We offset the time values of the plot so that unix timestamps don't run out of precision.
    ///
    /// Other parts of the system, such as query clamping, need to be aware of that offset in order
    /// to work properly.
    pub(crate) time_offset: i64,

    /// Cached disambiguated names for visualizers, used when no label is provided.
    pub(crate) default_series_name_formats: HashMap<VisualizerInstructionId, String>,

    /// The number of time series rendered by each visualizer instruction last frame.
    ///
    /// We track egui-ids here because the number of "series" passed to egui can actually be much higher
    /// since every color change, every discontinuity, etc. creates a new series, sharing the same egui id.
    pub(crate) num_time_series_last_frame_per_instruction:
        HashMap<VisualizerInstructionId, IntSet<egui::Id>>,
}

impl Default for TimeSeriesViewState {
    fn default() -> Self {
        Self {
            scalar_range: [0.0, 0.0].into(),
            max_time_view_range: AbsoluteTimeRange::EMPTY,
            time_offset: 0,
            default_series_name_formats: Default::default(),
            num_time_series_last_frame_per_instruction: Default::default(),
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

type ViewType = re_sdk_types::blueprint::views::TimeSeriesView;

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
        crate::fallbacks::register_fallbacks(system_registry);

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
        viewer_ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<TimeSeriesViewState>()?;

        list_item::list_item_scope(ui, "time_series_selection_ui", |ui| {
            let ctx = self.view_context(viewer_ctx, view_id, state, space_origin);
            view_property_ui::<PlotBackground>(&ctx, ui);
            view_property_ui::<PlotLegend>(&ctx, ui);

            let link_x_axis = ViewProperty::from_archetype::<TimeAxis>(
                ctx.blueprint_db(),
                ctx.blueprint_query(),
                view_id,
            )
            .component_or_fallback::<LinkAxis>(&ctx, TimeAxis::descriptor_link().component)?;

            match link_x_axis {
                LinkAxis::Independent => {
                    view_property_ui::<TimeAxis>(&ctx, ui);
                }
                LinkAxis::LinkToGlobal => {
                    re_view::view_property_ui_with_redirect::<TimeAxis>(
                        &ctx,
                        ui,
                        TimeAxis::descriptor_view_range().component,
                        re_viewer_context::GLOBAL_VIEW_ID,
                    );
                }
            }

            view_property_ui::<ScalarAxis>(&ctx, ui);

            Ok::<(), ViewSystemExecutionError>(())
        })
        .inner?;

        Ok(())
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> ViewSpawnHeuristics {
        re_tracing::profile_function!();

        // Note that point series has the same visualizable conditions, it doesn't matter which one we look at here.
        let Some(visualizable_entities) = ctx
            .visualizable_entities_per_visualizer
            .get(&SeriesLinesSystem::identifier())
        else {
            return ViewSpawnHeuristics::empty();
        };

        let entities_with_exact_match =
            visualizable_entities
                .iter()
                .filter_map(|(entity_path, reason)| {
                    if !include_entity(entity_path) {
                        return None;
                    }
                    reason
                        .full_native_match(Scalars::descriptor_scalars().component)
                        .then_some(entity_path)
                });

        ViewSpawnHeuristics::new_with_order_preserved(
            entities_with_exact_match
                .into_iter()
                .map(|entity_path| RecommendedView::new_single_entity(entity_path.clone())),
        )
    }

    /// Auto picked visualizers for an entity if there was not explicit selection.
    fn recommended_visualizers_for_entity(
        &self,
        entity_path: &EntityPath,
        visualizable_entities_per_visualizer: &PerVisualizerTypeInViewClass<VisualizableEntities>,
        indicated_entities_per_visualizer: &PerVisualizerType<IndicatedEntities>,
    ) -> RecommendedVisualizers {
        let available_visualizers: HashMap<ViewSystemIdentifier, &VisualizableReason> =
            visualizable_entities_per_visualizer
                .iter()
                .filter_map(|(visualizer, ents)| {
                    ents.get(entity_path).map(|reason| (*visualizer, reason))
                })
                .collect();

        let mut recommended = RecommendedVisualizers(
            available_visualizers
                .iter()
                .filter_map(|(visualizer, reason)| {
                    // Filter out entities that weren't indicated.
                    // We later fall back on to line visualizers for those.
                    if indicated_entities_per_visualizer
                        .get(visualizer)?
                        .contains(entity_path)
                    {
                        // Each scalar source becomes a separate VisualizerComponentMappings
                        // so that each nested scalar field gets its own time series.
                        let all_mappings: Vec<VisualizerComponentMappings> =
                            all_scalar_mappings(reason)
                                .map(|(component, source)| BTreeMap::from([(component, source)]))
                                .collect();
                        Vec1::try_from_vec(all_mappings)
                            .ok()
                            .map(|mappings| (*visualizer, mappings))
                    } else {
                        None
                    }
                })
                .collect(),
        );

        // If there were no other visualizers, but the SeriesLineSystem is available, use it.
        if recommended.0.is_empty()
            && let Some(series_line_visualizable_reason) =
                available_visualizers.get(&SeriesLinesSystem::identifier())
        {
            // Each scalar source becomes a separate VisualizerComponentMappings
            // so that each nested scalar field gets its own time series.
            let all_mappings: Vec<VisualizerComponentMappings> =
                all_scalar_mappings(series_line_visualizable_reason)
                    .map(|(component, source)| BTreeMap::from([(component, source)]))
                    .collect();
            if let Ok(mappings) = Vec1::try_from_vec(all_mappings) {
                recommended
                    .0
                    .insert(SeriesLinesSystem::identifier(), mappings);
            }
        }

        recommended
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
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

        state.num_time_series_last_frame_per_instruction.clear();

        let view_query_result = ctx.lookup_query_result(query.view_id);
        let scalar_component = Scalars::descriptor_scalars().component;

        let mut series_names = SeriesNamesContext::default();

        {
            re_tracing::profile_scope!("iterate all_plot_series");

            for series in &all_plot_series {
                let instruction_id = series.visualizer_instruction_id;

                if let Some(data_result) = view_query_result
                    .tree
                    .lookup_result_by_visualizer_instruction(instruction_id)
                    && let Some(instruction) = data_result
                        .visualizer_instructions
                        .iter()
                        .find(|instr| instr.id == instruction_id)
                {
                    let (component, selector) = instruction
                        .component_mappings
                        .get(&scalar_component)
                        .and_then(|mapping| match mapping {
                            re_viewer_context::VisualizerComponentSource::SourceComponent {
                                source_component,
                                selector,
                            } => Some((*source_component, selector.clone())),
                            _ => None,
                        })
                        .unwrap_or((scalar_component, String::new()));

                    series_names.insert(
                        instruction_id,
                        SeriesInfo::new(data_result.entity_path.clone(), component, &selector),
                    );
                }

                state
                    .num_time_series_last_frame_per_instruction
                    .entry(instruction_id)
                    .or_default()
                    .insert(series.id());
            }
        }

        // Compute disambiguated names after all series are collected
        state.default_series_name_formats = series_names.dissambiguated_names();

        // Note that a several plot items can point to the same entity path and in some cases even to the same instance path!
        // (e.g. when plotting both lines & points with the same entity/instance path)
        let plot_item_id_to_instance_path: HashMap<egui::Id, InstancePath> = all_plot_series
            .iter()
            .map(|series| (series.id(), series.instance_path.clone()))
            .collect();

        let current_time = ctx.time_ctrl.time_i64();
        let Some(timeline) = ctx.time_ctrl.timeline() else {
            return Ok(());
        };
        let time_type = timeline.typ();

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

        // Get the min and max time/X value for the visible plot.
        let min_view_time = all_plot_series
            .iter()
            .filter_map(|line| line.points.first().map(|(t, _)| *t))
            .min()
            .unwrap_or(0);
        let max_view_time = all_plot_series
            .iter()
            .filter_map(|line| line.points.last().map(|(t, _)| *t))
            .max()
            .unwrap_or(0);

        let recording = ctx.recording();

        let timeline_range = recording
            .time_range_for(timeline.name())
            .unwrap_or(AbsoluteTimeRange::EVERYTHING);

        state.max_time_view_range = AbsoluteTimeRange::new(
            TimeInt::saturated_temporal_i64(min_view_time),
            TimeInt::saturated_temporal_i64(max_view_time),
        );

        let blueprint_db = ctx.blueprint_db();
        let view_id = query.view_id;

        let view_ctx = self.view_context(ctx, view_id, state, query.space_origin);
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

        let view_current_time = re_sdk_types::datatypes::TimeInt(
            current_time
                .unwrap_or_default()
                .at_least(timeline_range.min.as_i64()),
        );

        let query_result;
        // If we globally link the x-axis it will ignore this view's time range property and use
        // `GLOBAL_VIEW_ID's` time range property instead.
        let (time_range_property, time_range_ctx) = match link_x_axis {
            LinkAxis::Independent => (&time_axis, &view_ctx),
            LinkAxis::LinkToGlobal => {
                query_result = re_viewer_context::DataQueryResult::default();

                (
                    &ViewProperty::from_archetype::<TimeAxis>(
                        ctx.blueprint_db(),
                        ctx.blueprint_query,
                        re_viewer_context::GLOBAL_VIEW_ID,
                    ),
                    &re_viewer_context::ViewContext {
                        viewer_ctx: ctx,
                        view_id: re_viewer_context::GLOBAL_VIEW_ID,
                        view_class_identifier: Self::identifier(),
                        space_origin: query.space_origin,
                        view_state: state,
                        query_result: &query_result,
                    },
                )
            }
        };

        let view_time_range = time_range_property
            .component_or_fallback::<re_sdk_types::blueprint::components::TimeRange>(
                time_range_ctx,
                TimeAxis::descriptor_view_range().component,
            )?;

        let resolve_time_range =
            |view_time_range: &re_sdk_types::blueprint::components::TimeRange| {
                make_range_sane(Range1D::new(
                    match view_time_range.start {
                        re_sdk_types::datatypes::TimeRangeBoundary::Infinite => {
                            timeline_range.min.as_i64()
                        }
                        _ => {
                            view_time_range
                                .start
                                .start_boundary_time(view_current_time)
                                .0
                        }
                    }
                    .saturating_sub(time_offset) as f64,
                    match view_time_range.end {
                        re_sdk_types::datatypes::TimeRangeBoundary::Infinite => {
                            timeline_range.max.as_i64()
                        }
                        _ => view_time_range.end.end_boundary_time(view_current_time).0,
                    }
                    .saturating_sub(time_offset) as f64,
                ))
            };

        let x_range = resolve_time_range(&view_time_range);

        let scalar_axis =
            ViewProperty::from_archetype::<ScalarAxis>(blueprint_db, ctx.blueprint_query, view_id);
        let y_range = scalar_axis.component_or_fallback::<Range1D>(
            &view_ctx,
            ScalarAxis::descriptor_range().component,
        )?;
        let y_range = make_range_sane(y_range);

        let zoom_lock = Vec2b::new(
            **time_axis.component_or_fallback::<LockRangeDuringZoom>(
                &view_ctx,
                TimeAxis::descriptor_zoom_lock().component,
            )?,
            **scalar_axis.component_or_fallback::<LockRangeDuringZoom>(
                &view_ctx,
                ScalarAxis::descriptor_zoom_lock().component,
            )?,
        );

        // TODO(jleibs): If this is allowed to be different, need to track it per line.
        let aggregation_factor = all_plot_series
            .first()
            .map_or(1.0, |line| line.aggregation_factor);

        let aggregator = all_plot_series
            .first()
            .map(|line| line.aggregator)
            .unwrap_or_default();

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
                .auto_bounds(false)
                .allow_zoom(!zoom_lock)
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
                transform,
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

            // Decide if the time cursor should be displayed, and if so where:
            let time_x = current_time
                .map(|current_time| (current_time.saturating_sub(time_offset)) as f64)
                .filter(|&x| {
                    // only display the time cursor when it's actually above the plot area
                    transform.bounds().min()[0] <= x && x <= transform.bounds().max()[0]
                })
                .map(|x| transform.position_from_point(&PlotPoint::new(x, 0.0)).x);

            if let Some(time_x) = time_x {
                draw_time_cursor(ctx, ui, &response, &transform, time_offset, time_x);
            }

            // Can determine whether we're resetting only now since we need to know whether there's a plot item hovered.
            let is_resetting = plot_double_clicked && hovered_data_result.is_none();

            if is_resetting {
                reset_view(ctx, time_range_property, &scalar_axis);

                ui.ctx().request_repaint(); // Make sure we get another frame with the view reset.
            } else {
                let unchanged_bounds = egui_plot::PlotBounds::from_min_max(
                    [x_range.start(), y_range.start()],
                    [x_range.end(), y_range.end()],
                );

                if unchanged_bounds != *transform.bounds() {
                    let new_x_range = transform_axis_range(transform, 0);
                    let new_x_range_rounded =
                        Range1D::new(new_x_range.start().round(), new_x_range.end().round());

                    let new_view_time_range =
                        re_sdk_types::blueprint::components::TimeRange(TimeRange {
                            start: re_sdk_types::datatypes::TimeRangeBoundary::Absolute(
                                re_sdk_types::datatypes::TimeInt(
                                    (new_x_range_rounded.start() as i64)
                                        .saturating_add(time_offset),
                                ),
                            ),
                            end: re_sdk_types::datatypes::TimeRangeBoundary::Absolute(
                                re_sdk_types::datatypes::TimeInt(
                                    (new_x_range_rounded.end() as i64).saturating_add(time_offset),
                                ),
                            ),
                        });

                    if new_x_range != x_range && view_time_range != new_view_time_range {
                        time_range_property.save_blueprint_component(
                            ctx,
                            &TimeAxis::descriptor_view_range(),
                            &new_view_time_range,
                        );
                        ui.ctx().request_repaint(); // Make sure we get another frame with this new range applied.
                    }

                    let new_y_range = transform_axis_range(transform, 1);

                    // Write new y_range if it has changed.
                    if new_y_range != y_range {
                        scalar_axis.save_blueprint_component(
                            ctx,
                            &ScalarAxis::descriptor_range(),
                            &new_y_range,
                        );
                        ui.ctx().request_repaint(); // Make sure we get another frame with this new range applied.
                    }
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

            Ok(())
        })
        .inner
    }

    fn visualizers_ui<'a>(
        &'a self,
        viewer_ctx: &'a re_viewer_context::ViewerContext<'a>,
        view_id: ViewId,
        state: &'a mut dyn ViewState,
        space_origin: &'a EntityPath,
    ) -> Option<Box<dyn Fn(&mut egui::Ui) + 'a>> {
        let state = state.downcast_mut::<TimeSeriesViewState>().ok()?;

        let visualizer_ui = move |ui: &mut egui::Ui| {
            list_item::list_item_scope(ui, "time_series_visualizers_ui", |ui| {
                let ctx = self.view_context(viewer_ctx, view_id, state, space_origin);
                re_tracing::profile_function!();
                let query_result = ctx.query_result;

                let handles = query_result.tree.data_results_by_path.values().sorted();
                for handle in handles {
                    let Some(node) = query_result.tree.data_results.get(*handle) else {
                        continue;
                    };
                    let pill_margin = egui::Margin::symmetric(8, 6);
                    for instruction in &node.data_result.visualizer_instructions {
                        ui.add_space(10.0);

                        let entity_path = &node.data_result.entity_path;

                        let full_path = entity_path
                            .to_string()
                            .strip_prefix('/')
                            .map(|s| s.to_owned())
                            .unwrap_or_else(|| entity_path.to_string());

                        let series_color =
                            get_time_series_color(&ctx, &node.data_result, instruction);

                        let display_name =
                            get_time_series_name(&ctx, &node.data_result, instruction);

                        // Estimate the pill height so Sides can vertically center
                        // both sides (pill on the left, trash button on the right).
                        let pill_height = 2.0 * ui.text_style_height(&egui::TextStyle::Body)
                            + ui.spacing().item_spacing.y
                            + pill_margin.sum().y;

                        egui::Sides::new().height(pill_height).shrink_left().show(
                            ui,
                            |ui| {
                                let mut frame = egui::Frame::default()
                                    .fill(ui.tokens().visualizer_list_pill_bg_color)
                                    .corner_radius(4.0)
                                    .inner_margin(pill_margin)
                                    .begin(ui);
                                {
                                    let ui = &mut frame.content_ui;
                                    ui.set_width(ui.available_width());

                                    // Disable text selection so hovering the text only hovers the pill
                                    ui.style_mut().interaction.selectable_labels = false;

                                    // Visualizer name and entity path
                                    let labels =
                                        ui.vertical(|ui| {
                                            ui.label(egui::RichText::new(&display_name).color(
                                                ui.tokens().visualizer_list_title_text_color,
                                            ));
                                            ui.label(
                                                egui::RichText::new(&full_path).size(10.5).color(
                                                    ui.tokens().visualizer_list_path_text_color,
                                                ),
                                            );
                                        });

                                    // Color box(es) on the right, vertically centered on the labels.
                                    series_color.ui(ui, labels.response.rect.center().y);
                                }
                                let response = frame
                                    .allocate_space(ui)
                                    .interact(egui::Sense::click())
                                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                                if response.hovered() {
                                    frame.frame.fill =
                                        ui.tokens().visualizer_list_pill_bg_color_hovered;
                                }
                                if response.clicked() {
                                    let instance_path = InstancePath::from(entity_path.clone());
                                    ctx.viewer_ctx.command_sender().send_system(
                                        re_viewer_context::SystemCommand::set_selection(
                                            Item::DataResult(ctx.view_id, instance_path),
                                        ),
                                    );
                                }
                                frame.paint(ui);
                            },
                            |ui| {
                                // Trashcan button to remove this visualizer.
                                let remove_response =
                                    ui.small_icon_button(&re_ui::icons::TRASH, "Remove visualizer");
                                if remove_response.clicked() {
                                    let override_base_path = &node.data_result.override_base_path;

                                    let active_visualizers = node
                                        .data_result
                                        .visualizer_instructions
                                        .iter()
                                        .filter(|v| v.id != instruction.id)
                                        .collect::<Vec<_>>();

                                    let archetype = ActiveVisualizers::new(
                                        active_visualizers.iter().map(|v| v.id.0),
                                    );

                                    ctx.save_blueprint_archetype(
                                        override_base_path.clone(),
                                        &archetype,
                                    );

                                    // Ensure the remaining instructions are persisted so that their
                                    // types and mappings are available on the next frame.
                                    for visualizer_instruction in active_visualizers {
                                        visualizer_instruction
                                            .write_instruction_to_blueprint(ctx.viewer_ctx);
                                    }
                                }
                            },
                        );
                    }
                }
            });
        };

        Some(Box::new(visualizer_ui))
    }
}

/// Returns a priority score for a given Arrow datatype.
/// Lower scores are preferred.
fn scalar_datatype_priority(datatype: &re_log_types::external::arrow::datatypes::DataType) -> u32 {
    use re_log_types::external::arrow::datatypes::DataType;
    match datatype {
        DataType::Float64 => 0,
        DataType::Float32 => 1,
        DataType::Float16 => 2,
        // Note: We can visualize the following datatype but don't recommend them.
        DataType::Int64 => 3,
        DataType::Int32 => 5,
        DataType::Int16 => 7,
        DataType::Int8 => 9,
        DataType::Boolean => 11,
        DataType::UInt64 => 4,
        DataType::UInt32 => 6,
        DataType::UInt16 => 8,
        DataType::UInt8 => 10,
        _ => 100, // Any other type gets lowest priority
    }
}

const RECOMMENDED_DATATYPES: &[DataType] =
    &[DataType::Float64, DataType::Float32, DataType::Float16];

fn all_scalar_mappings(
    reason: &VisualizableReason,
) -> impl Iterator<Item = (ComponentIdentifier, VisualizerComponentSource)> {
    let re_viewer_context::VisualizableReason::DatatypeMatchAny {
        target_component: _,
        matches,
    } = reason
    else {
        return Either::Left(std::iter::empty());
    };

    let target = Scalars::descriptor_scalars();

    // Flatten all (component, selector) pairs into a single comparable list
    // to find the globally best match across all components.
    let candidates = matches.iter().flat_map(|(source_component, match_info)| {
        let is_rerun_native_type = match_info.component_type() == &target.component_type;

        // If it's not the exact semantic type that we're looking for,
        // but it is a Rerun-builtin semantic type then we don't consider it at all.
        if !is_rerun_native_type
            && match_info
                .component_type()
                .is_some_and(|t| t.is_rerun_type())
        {
            return Either::Left(Either::Right(std::iter::empty()));
        }

        let primary_match_order = match match_info {
            DatatypeMatch::NativeSemantics { .. } => {
                i32::from(*source_component != target.component)
            }
            DatatypeMatch::PhysicalDatatypeOnly { .. } => {
                if *source_component == target.component {
                    0
                } else {
                    2
                }
            }
        };

        match match_info {
            DatatypeMatch::NativeSemantics { arrow_datatype, .. } => {
                Either::Left(Either::Left(std::iter::once((
                    primary_match_order,
                    is_rerun_native_type,
                    scalar_datatype_priority(arrow_datatype),
                    *source_component,
                    0usize,
                    String::new(),
                ))))
            }
            DatatypeMatch::PhysicalDatatypeOnly {
                arrow_datatype,
                selectors,
                ..
            } => {
                if selectors.is_empty() {
                    if RECOMMENDED_DATATYPES.contains(match_info.arrow_datatype()) {
                        Either::Left(Either::Left(std::iter::once((
                            primary_match_order,
                            is_rerun_native_type,
                            scalar_datatype_priority(arrow_datatype),
                            *source_component,
                            0usize,
                            String::new(),
                        ))))
                    } else {
                        Either::Left(Either::Right(std::iter::empty()))
                    }
                } else {
                    // Nested field access: selector_index preserves field definition order.
                    Either::Right(selectors.iter().enumerate().filter_map(
                        move |(selector_index, (selector, datatype))| {
                            RECOMMENDED_DATATYPES.contains(datatype).then_some((
                                primary_match_order,
                                is_rerun_native_type,
                                scalar_datatype_priority(datatype),
                                *source_component,
                                selector_index,
                                selector.to_string(),
                            ))
                        },
                    ))
                }
            }
        }
    });

    // Priority key (lower = better):
    // 1. primary_match_order (0=exact match, 1=semantic match, 2=physical only)
    // 2. is_rerun_native_type (false < true, prefer custom types)
    // 3. scalar_datatype_priority (Float64=0, Float32=1, etc.)
    // 4. source_component (deterministic component selection)
    // 5. selector_index (field definition order within component)
    Either::Right(
        candidates
            .into_iter()
            .sorted_by_key(
                |(match_order, is_rerun, dt_priority, component, field_order, _)| {
                    (
                        *match_order,
                        *is_rerun,
                        *dt_priority,
                        *component,
                        *field_order,
                    )
                },
            )
            .map(move |(_, _, _, source_component, _, selector)| {
                (
                    target.component,
                    VisualizerComponentSource::SourceComponent {
                        source_component,
                        selector,
                    },
                )
            }),
    )
}

fn draw_time_cursor(
    ctx: &ViewerContext<'_>,
    ui: &egui::Ui,
    response: &egui::Response,
    transform: &egui_plot::PlotTransform,
    time_offset: i64,
    mut time_x: f32,
) -> egui::Response {
    let interact_radius = ui.style().interaction.resize_grab_radius_side;
    let line_rect = egui::Rect::from_x_y_ranges(time_x..=time_x, response.rect.y_range())
        .expand(interact_radius);

    let time_drag_id = ui.id().with("time_drag");
    let time_cursor_response = ui
        .interact(line_rect, time_drag_id, egui::Sense::drag())
        .on_hover_and_drag_cursor(egui::CursorIcon::ResizeHorizontal);

    if time_cursor_response.dragged()
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
    }

    ui.paint_time_cursor(
        ui.painter(),
        Some(&time_cursor_response),
        time_x,
        time_cursor_response.rect.y_range(),
    );
    time_cursor_response
}

fn reset_view(ctx: &ViewerContext<'_>, time_axis: &ViewProperty, scalar_axis: &ViewProperty) {
    scalar_axis.reset_blueprint_component(ctx, ScalarAxis::descriptor_range());
    time_axis.reset_blueprint_component(ctx, TimeAxis::descriptor_view_range());
}

/// axis = 0 is the x axis
///
/// axis = 1 is the y axis
fn transform_axis_range(transform: egui_plot::PlotTransform, axis: usize) -> Range1D {
    Range1D::new(
        transform.bounds().min()[axis],
        transform.bounds().max()[axis],
    )
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
            .map(|series| series.id())
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
    let mut per_visualizer_inst_id_series_new_visibility_state: HashMap<
        VisualizerInstructionId,
        SmallVec<[bool; 1]>,
    > = HashMap::default();
    let mut series_to_update = Vec::new();
    for series in all_plot_series {
        let entity_visibility_flags = per_visualizer_inst_id_series_new_visibility_state
            .entry(series.visualizer_instruction_id)
            .or_default();

        let visible_new = !hidden_items.contains(&series.id());

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
        let Some(visibility_state) = per_visualizer_inst_id_series_new_visibility_state
            .remove(&series.visualizer_instruction_id)
        else {
            continue;
        };
        let Some(result) = query_results.result_for_entity(&series.instance_path.entity_path)
        else {
            continue;
        };

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

        if let Some(visualizer_instruction) = result
            .visualizer_instructions
            .iter()
            .find(|instr| instr.id == series.visualizer_instruction_id)
        {
            let override_path = &visualizer_instruction.override_path;

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
        } else {
            re_log::warn_once!(
                "Could not find visualizer instruction for series at instance path `{}`",
                series.instance_path
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

                    [(p.0.saturating_sub(time_offset)) as _, p.1]
                })
                .collect::<Vec<_>>()
        } else {
            // TODO(emilk/egui_plot#92): Note we still need to produce a series, so it shows up in the legend.
            // As of writing, egui_plot gets confused if this is an empty series, so
            // we still add a single point (but don't have it influence the scalar range!)
            series
                .points
                .first()
                .map(|p| vec![[(p.0.saturating_sub(time_offset)) as _, p.1]])
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
                    .id(series.id()),
            ),
            PlotSeriesKind::Scatter(scatter_attrs) => plot_ui.points(
                Points::new(&series.label, points)
                    .color(color)
                    .radius(series.radius_ui)
                    .shape(scatter_attrs.marker.into())
                    .highlight(highlight)
                    .id(series.id()),
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
pub fn make_range_sane(y_range: Range1D) -> Range1D {
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

fn strip_instance_number(str: &str) -> String {
    if let Some(stripped) = str.strip_suffix(']').and_then(|s| {
        let i = s.rfind('[')?;
        s[i + 1..]
            .bytes()
            .all(|b| b.is_ascii_digit())
            .then_some(&s[..i])
    }) {
        format!("{stripped}[]")
    } else {
        str.to_owned()
    }
}

/// Returns the name for a time series visualizer.
fn get_time_series_name(
    ctx: &re_viewer_context::ViewContext<'_>,
    data_result: &re_viewer_context::DataResult,
    instruction: &re_viewer_context::VisualizerInstruction,
) -> String {
    let component = if instruction.visualizer_type == SeriesLinesSystem::identifier() {
        SeriesLines::descriptor_names().component
    } else if instruction.visualizer_type == SeriesPointsSystem::identifier() {
        SeriesPoints::descriptor_names().component
    } else {
        return instruction.visualizer_type.to_string();
    };

    let query_result = re_view::latest_at_with_blueprint_resolved_data(
        ctx,
        None,
        &ctx.current_query(),
        data_result,
        [component],
        Some(instruction),
    );

    let first_name = query_result.get_mono_with_fallback::<Name>(component);

    // We might have already "injected" the instance number into the name of the series.
    // So we re-normalize the series name again.
    strip_instance_number(&first_name)
}

/// List of colors for a time series visualizer.
#[derive(Default)]
struct TimeSeriesColors {
    instance_count: usize,
    colors: ArrayVec<egui::Color32, NUM_SHOWN_VISUALIZER_COLORS>,
}

impl TimeSeriesColors {
    // TODO(RR-3745): Don't calculate positions here, find a more egui-friendly way to do this.
    /// Draws color boxes (and an optional "+N" badge) right-aligned and vertically centered on
    /// the given `center_y`.
    fn ui(&self, ui: &egui::Ui, center_y: f32) {
        if self.colors.is_empty() {
            return;
        }

        let size = ui.tokens().visualizer_list_color_box_size;
        let color_box_size = egui::vec2(size, size);
        let spacing = 4.0;

        let num_boxes = if self.instance_count > 2 {
            1
        } else {
            self.colors.len()
        };

        // Draw "+N" badge when there are more than 2 instances
        let mut right_edge = ui.max_rect().right();
        if self.instance_count > 2 {
            let badge_text = format!("+{}", self.instance_count - 1);
            let galley = ui.painter().layout_no_wrap(
                badge_text,
                egui::FontId::proportional(10.5),
                ui.tokens().visualizer_list_path_text_color,
            );
            let badge_rect = egui::Rect::from_center_size(
                egui::pos2(right_edge - galley.size().x / 2.0, center_y),
                galley.size(),
            );
            ui.painter()
                .galley(badge_rect.min, galley, egui::Color32::WHITE);
            right_edge -= badge_rect.width() + spacing;
        }

        // Draw color boxes from right to left
        for color in self.colors[..num_boxes].iter().rev() {
            let rect = egui::Rect::from_center_size(
                egui::pos2(right_edge - size / 2.0, center_y),
                color_box_size,
            );
            ui.painter().rect(
                rect,
                3.0,
                *color,
                ui.tokens().visualizer_list_color_box_stroke,
                egui::StrokeKind::Inside,
            );
            right_edge -= size + spacing;
        }
    }
}

/// Returns the colors of time series plots.
fn get_time_series_color(
    ctx: &re_viewer_context::ViewContext<'_>,
    data_result: &re_viewer_context::DataResult,
    instruction: &re_viewer_context::VisualizerInstruction,
) -> TimeSeriesColors {
    let color_component = if instruction.visualizer_type == SeriesLinesSystem::identifier() {
        SeriesLines::descriptor_colors().component
    } else if instruction.visualizer_type == SeriesPointsSystem::identifier() {
        SeriesPoints::descriptor_colors().component
    } else {
        // Unkownn visualizer type, don't show any colors
        return TimeSeriesColors::default();
    };

    // Get the colors for each instance
    let query = ctx.current_query();
    let color_result = re_view::latest_at_with_blueprint_resolved_data(
        ctx,
        None,
        &query,
        data_result,
        [color_component],
        Some(instruction),
    );

    let raw_color_cell = if let Some(color_cells) = color_result.get_raw_cell(color_component) {
        // We have color data either in the store or as overrides
        color_cells
    } else {
        // No color data in the store or overrides, use the fallback
        ctx.viewer_ctx.component_fallback_registry.fallback_for(
            color_component,
            Some(<Color as re_sdk_types::Component>::name()),
            &ctx.query_context(data_result, query, instruction.id),
        )
    };

    let Ok(color_components) = Color::from_arrow(&raw_color_cell) else {
        re_log::error_once!("Failed to cast color array to Color");
        return TimeSeriesColors::default();
    };

    let colors = color_components
        .iter()
        .map(|&value| value.into()) // Color is ABGR, egui uses to RGBA, can't use bytemuck here.
        .take(NUM_SHOWN_VISUALIZER_COLORS)
        .collect();

    TimeSeriesColors {
        instance_count: color_components.len(),
        colors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_view() {
        re_test_context::TestContext::test_help_view(|ctx| TimeSeriesView.help(ctx));
    }

    fn build_visualizable_entities(
        entity_path: &EntityPath,
        reason: VisualizableReason,
    ) -> PerVisualizerTypeInViewClass<VisualizableEntities> {
        PerVisualizerTypeInViewClass {
            view_class_identifier: TimeSeriesView::identifier(),
            per_visualizer: std::iter::once((
                SeriesLinesSystem::identifier(),
                VisualizableEntities(std::iter::once((entity_path.clone(), reason)).collect()),
            ))
            .collect(),
        }
    }

    /// Regression: non-recommended physical datatype (`Int32`) must not cause
    /// `SeriesLinesSystem` to be recommended with an empty mapping.
    #[test]
    fn test_no_recommendation_for_non_recommended_datatype() {
        let entity_path = EntityPath::from("sensor/data");
        let viz = build_visualizable_entities(
            &entity_path,
            VisualizableReason::DatatypeMatchAny {
                target_component: Scalars::descriptor_scalars().component,
                matches: std::iter::once((
                    Scalars::descriptor_scalars().component,
                    DatatypeMatch::PhysicalDatatypeOnly {
                        arrow_datatype: DataType::Int32,
                        component_type: None,
                        selectors: vec![],
                    },
                ))
                .collect(),
            },
        );
        let indicated = PerVisualizerType::default();
        let result =
            TimeSeriesView.recommended_visualizers_for_entity(&entity_path, &viz, &indicated);

        assert!(result.0.is_empty());
    }

    /// `SeriesLinesSystem` should be recommended when the datatype is a recommended one, even if not indicated.
    #[test]
    fn test_recommendation_for_recommended_datatype() {
        let entity_path = EntityPath::from("sensor/data");
        let viz = build_visualizable_entities(
            &entity_path,
            VisualizableReason::DatatypeMatchAny {
                target_component: Scalars::descriptor_scalars().component,
                matches: std::iter::once((
                    Scalars::descriptor_scalars().component,
                    DatatypeMatch::NativeSemantics {
                        arrow_datatype: DataType::Float64,
                        component_type: None,
                    },
                ))
                .collect(),
            },
        );
        let indicated = PerVisualizerType::default();
        let result =
            TimeSeriesView.recommended_visualizers_for_entity(&entity_path, &viz, &indicated);

        assert!(result.0.contains_key(&SeriesLinesSystem::identifier()));
        let mappings = &result.0[&SeriesLinesSystem::identifier()];
        assert_eq!(mappings.len(), 1);
        assert!(mappings[0].contains_key(&Scalars::descriptor_scalars().component));
    }
}

#[test]
fn test_strip_instance_number() {
    // Empty string
    assert_eq!(strip_instance_number(""), "");

    // No brackets at all
    assert_eq!(strip_instance_number("foo"), "foo");
    assert_eq!(strip_instance_number("hello world"), "hello world");

    // Valid instance numbers should be normalized to []
    assert_eq!(strip_instance_number("foo[0]"), "foo[]");
    assert_eq!(strip_instance_number("foo[1]"), "foo[]");
    assert_eq!(strip_instance_number("foo[123]"), "foo[]");

    // Empty brackets (no digits) should remain unchanged
    assert_eq!(strip_instance_number("foo[]"), "foo[]");

    // Non-digit content in brackets should remain unchanged
    assert_eq!(strip_instance_number("foo[abc]"), "foo[abc]");
    assert_eq!(strip_instance_number("foo[1a]"), "foo[1a]");
    assert_eq!(strip_instance_number("foo[a1]"), "foo[a1]");
    assert_eq!(strip_instance_number("foo[ ]"), "foo[ ]");
    assert_eq!(strip_instance_number("foo[1 2]"), "foo[1 2]");

    // Half-open brackets (only `[`) should remain unchanged
    assert_eq!(strip_instance_number("foo["), "foo[");
    assert_eq!(strip_instance_number("["), "[");
    assert_eq!(strip_instance_number("foo[123"), "foo[123");

    // Half-closed brackets (only `]`) should remain unchanged
    assert_eq!(strip_instance_number("foo]"), "foo]");
    assert_eq!(strip_instance_number("]"), "]");
    assert_eq!(strip_instance_number("123]"), "123]");

    // Multiple bracket pairs - only the last valid instance number is stripped
    assert_eq!(strip_instance_number("foo[0][1]"), "foo[0][]");
    assert_eq!(strip_instance_number("foo[abc][123]"), "foo[abc][]");

    // Nested or malformed brackets
    assert_eq!(strip_instance_number("foo[[0]]"), "foo[[0]]");
    assert_eq!(strip_instance_number("foo[0]["), "foo[0][");
    assert_eq!(strip_instance_number("foo][0]"), "foo][]");

    // Edge cases with brackets in the middle
    assert_eq!(strip_instance_number("foo[0]bar"), "foo[0]bar");
    assert_eq!(strip_instance_number("foo[0]bar[1]"), "foo[0]bar[]");
}
