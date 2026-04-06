use egui::ahash::HashMap;
use egui::{NumExt as _, Vec2, Vec2b};
use egui_plot::{Line, Plot, PlotPoint, Points};
use itertools::{Either, Itertools as _};
use nohash_hasher::{IntMap, IntSet};
use re_chunk_store::TimeType;
use re_format::time::next_grid_tick_magnitude_nanos;
use re_log_types::external::arrow::datatypes::DataType;
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt};
use re_sdk_types::archetypes::{Scalars, SeriesLines, SeriesPoints};
use re_sdk_types::blueprint::archetypes::{PlotBackground, PlotLegend, ScalarAxis, TimeAxis};
use re_sdk_types::blueprint::components::{
    Corner2D, Enabled, LinkAxis, LockRangeDuringZoom, VisualizerInstructionId,
};
use re_sdk_types::components::{AggregationPolicy, Color, Range1D, Visible};
use re_sdk_types::datatypes::TimeRange;
use re_sdk_types::{ComponentBatch as _, ComponentIdentifier, View as _, ViewClassIdentifier};
use re_ui::{Help, IconText, MouseButtonText, UiExt as _, icons, list_item};
use re_view::controls::{MOVE_TIME_CURSOR_BUTTON, SELECTION_RECT_ZOOM_BUTTON};
use re_view::view_property_ui;
use re_viewer_context::{
    BlueprintContext as _, DataResultInteractionAddress, DatatypeMatch, IdentifiedViewSystem as _,
    IndicatedEntities, PerVisualizerType, QueryRange, RecommendedMappings, RecommendedView,
    RecommendedVisualizers, SingleRequiredComponentMatch, SystemExecutionOutput,
    TimeControlCommand, ViewClass, ViewClassExt as _, ViewClassRegistryError, ViewHighlights,
    ViewId, ViewQuery, ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError,
    ViewSystemIdentifier, ViewerContext, VisualizableReason, VisualizerComponentSource,
};
use re_viewport_blueprint::ViewProperty;
use smallvec::SmallVec;
use vec1::Vec1;

use crate::line_visualizer_system::{SeriesLinesOutput, SeriesLinesSystem};
use crate::naming::{SeriesInfo, SeriesNamesContext};
use crate::point_visualizer_system::{SeriesPointsOutput, SeriesPointsSystem};
use crate::util::data_result_time_range;
use crate::{MAX_NUM_NON_INDICATED_RECOMMENDED_VISUALIZERS_PER_ENTITY, PlotSeriesKind};

// ---

#[derive(Clone)]
pub struct TimeSeriesViewState {
    /// The range of the scalar values currently on screen.
    ///
    /// None if no values are on screen right now.
    pub(crate) scalar_range: Option<Range1D>,

    /// The combined query range of all entities in this view.
    ///
    /// Only entities that are currently _visible_ are considered,
    /// but for these the entire data range in the store is calculated
    /// (not just what we're currently zoomed in on).
    pub(crate) full_data_time_range: AbsoluteTimeRange,

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
            scalar_range: None,
            full_data_time_range: AbsoluteTimeRange::EMPTY,
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
        visualizers_with_reason: &[(ViewSystemIdentifier, &VisualizableReason)],
        indicated_entities_per_visualizer: &PerVisualizerType<&IndicatedEntities>,
    ) -> RecommendedVisualizers {
        let available_visualizers: HashMap<ViewSystemIdentifier, &VisualizableReason> =
            visualizers_with_reason
                .iter()
                .map(|(visualizer, reason)| (*visualizer, *reason))
                .collect();

        let mut recommended = RecommendedVisualizers::new(
            available_visualizers
                .iter()
                .filter_map(|(visualizer, reason)| {
                    // Filter out entities that weren't indicated.
                    // We later fall back on to line visualizers for those.
                    if indicated_entities_per_visualizer
                        .get(visualizer)?
                        .contains(entity_path)
                    {
                        let all_mappings: Vec<RecommendedMappings> = all_scalar_mappings(reason)
                            .map(|(component, source)| RecommendedMappings::new(component, source))
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
        if recommended.all_recommendations().is_empty()
            && let Some(series_line_visualizable_reason) =
                available_visualizers.get(&SeriesLinesSystem::identifier())
        {
            let mut mappings_with_auto_spawn: Vec<RecommendedMappings> =
                all_scalar_mappings(series_line_visualizable_reason)
                    .map(|(component, source)| RecommendedMappings::new(component, source))
                    .collect();

            // Not all automatic recommendations should be spawned by default, since that can lead to
            // a huge number of visualizers being spawned for a single entity.
            let recommendation_only = if mappings_with_auto_spawn.len()
                > MAX_NUM_NON_INDICATED_RECOMMENDED_VISUALIZERS_PER_ENTITY
            {
                mappings_with_auto_spawn
                    .split_off(MAX_NUM_NON_INDICATED_RECOMMENDED_VISUALIZERS_PER_ENTITY)
            } else {
                Vec::new()
            };

            // First add mappings with auto spawn since they should show up higher in the list.
            if let Ok(mappings) = Vec1::try_from_vec(mappings_with_auto_spawn) {
                recommended.insert(SeriesLinesSystem::identifier(), mappings, true);
            }
            if let Ok(mappings) = Vec1::try_from_vec(recommendation_only) {
                recommended.insert(SeriesLinesSystem::identifier(), mappings, false);
            }
        }

        recommended
    }

    fn visualizers_section<'a>(
        &'a self,
        ctx: &'a re_viewer_context::ViewContext<'a>,
    ) -> Option<re_viewer_context::VisualizersSectionOutput<'a>> {
        let series_line_id = SeriesLinesSystem::identifier();
        let visualizable_entities = ctx
            .viewer_ctx
            .iter_visualizable_entities_for_view_class(Self::identifier())
            .find(|(viz, _)| *viz == series_line_id)
            .map(|(_, ents)| ents);

        let data_results = ctx.query_result.tree.iter_data_results();
        let add_options = data_results
            .filter_map(|data_result| {
                if data_result.tree_prefix_only {
                    return None;
                }

                // For the "add visualizer" menu, offer a SeriesLine for every possible scalar mapping.
                // Unlike `recommended_visualizers_for_entity` / `all_scalar_mappings`, we don't filter
                // by indication or recommended datatypes — we show everything that could be visualized.
                let VisualizableReason::SingleRequiredComponentMatch(
                    SingleRequiredComponentMatch {
                        target_component: _,
                        matches,
                    },
                ) = visualizable_entities?.get(&data_result.entity_path)?
                else {
                    return None;
                };

                let recommended = if let Ok(mappings) =
                    vec1::Vec1::try_from_vec(all_scalar_mappings_for(matches))
                {
                    RecommendedVisualizers::new(
                        std::iter::once((series_line_id, mappings)).collect(),
                    )
                } else {
                    return None;
                };

                Some((data_result.entity_path.clone(), recommended))
            })
            .collect();

        Some(re_viewer_context::VisualizersSectionOutput {
            ui: Box::new(move |ui, ctx| {
                visualizers_section_ui(ui, ctx);
            }),
            add_options,
        })
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

        let line_series =
            system_output.visualizer_data::<SeriesLinesOutput>(SeriesLinesSystem::identifier())?;
        let point_series =
            system_output.visualizer_data::<SeriesPointsOutput>(SeriesPointsSystem::identifier())?;

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
        let plot_item_id_to_data_result_address: HashMap<egui::Id, DataResultInteractionAddress> =
            all_plot_series
                .iter()
                .map(|series| {
                    (
                        series.id(),
                        DataResultInteractionAddress {
                            view_id: query.view_id,
                            instance_path: series.instance_path.clone(),
                            visualizer: Some(series.visualizer_instruction_id),
                        },
                    )
                })
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

        let recording = ctx.recording();

        let timeline_range = recording
            .time_range_for(timeline.name())
            .unwrap_or(AbsoluteTimeRange::EVERYTHING);

        {
            // Get the full time range of data in the store for all visible entity paths.
            // This queries the store directly rather than looking at loaded data,
            // so it works even before any chunks are loaded.
            state.full_data_time_range = AbsoluteTimeRange::EMPTY;

            for data_result in view_query_result.tree.iter_data_results() {
                if data_result.tree_prefix_only || !data_result.is_visible() {
                    continue;
                }
                state.full_data_time_range = state
                    .full_data_time_range
                    .union(data_result_time_range(ctx, data_result, query.timeline));
            }
        }

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

        let min_axis_thickness = ui.tokens().small_icon_size.y;

        let legend_id = egui::Id::new(query.view_id).with("plot_legend");
        let legend_hovered = ui
            .ctx()
            .read_response(re_ui::plot_legend::legend_frame_id(legend_id))
            .is_some_and(|r| r.contains_pointer());

        ui.scope(|ui| {
            // use timeline_name as part of id, so that egui stores different pan/zoom for different timelines
            let plot_id_src = ("plot", &timeline_name);

            ui.style_mut().visuals.extreme_bg_color = background_color.into();

            let mut plot = Plot::new(plot_id_src)
                .id(plot_id)
                .show_grid(**show_grid)
                .auto_bounds(false)
                .allow_zoom(!zoom_lock)
                .allow_scroll(!legend_hovered)
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

            // Legend is rendered separately after plot.show() using our LegendWidget.

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
                inner: (),
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
                .and_then(|hovered_plot_item| {
                    plot_item_id_to_data_result_address.get(&hovered_plot_item)
                })
                .map(|address| re_viewer_context::Item::DataResult(address.clone()));
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
                paint_time_cursor(ctx, ui, &response, &transform, time_offset, time_x);
            }

            // Can determine whether we're resetting only now since we need to know whether there's a plot item hovered.
            let is_resetting = plot_double_clicked && hovered_data_result.is_none();

            if is_resetting {
                reset_view(ctx, time_range_property, &scalar_axis);

                ui.request_repaint(); // Make sure we get another frame with the view reset.
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
                        ui.request_repaint(); // Make sure we get another frame with this new range applied.
                    }

                    let new_y_range = transform_axis_range(transform, 1);

                    // Write new y_range if it has changed.
                    if new_y_range != y_range {
                        scalar_axis.save_blueprint_component(
                            ctx,
                            &ScalarAxis::descriptor_range(),
                            &new_y_range,
                        );
                        ui.request_repaint(); // Make sure we get another frame with this new range applied.
                    }
                }
            }

            // Render our legend overlay and sync visibility with blueprint.
            if *legend_visible.0 {
                // Build a map from label → whether any series with that label is globally hovered.
                // Multiple series can share a label (e.g. /spiral[0] and /spiral[1]), so we OR
                // across all of them.
                let prev_hovered_items = ctx.selection_state().hovered_items();
                let mut label_hovered: egui::ahash::HashMap<&str, bool> =
                    egui::ahash::HashMap::default();
                for series in all_plot_series
                    .iter()
                    .filter(|s| !matches!(s.kind, PlotSeriesKind::Clear))
                {
                    let address = plot_item_id_to_data_result_address[&series.id()].clone();
                    let is_hovered = prev_hovered_items
                        .contains_item(&re_viewer_context::Item::DataResult(address.clone()))
                        || prev_hovered_items.contains_item(&re_viewer_context::Item::DataResult(
                            address.as_entity_all(),
                        ));
                    if is_hovered {
                        label_hovered.insert(series.label.as_str(), true);
                    } else {
                        label_hovered.entry(series.label.as_str()).or_insert(false);
                    }
                }

                let legend_widget =
                    re_ui::plot_legend::LegendWidget::new(re_ui::plot_legend::LegendConfig {
                        position: legend_corner.into(),
                        id: legend_id,
                    });

                // Render the legend overlaid on the plot rect.
                let plot_rect = response.rect;
                let mut legend_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(plot_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Min)),
                );

                let legend_output = legend_widget.show_entries(
                    &mut legend_ui,
                    all_plot_series
                        .iter()
                        .filter(|s| !matches!(s.kind, PlotSeriesKind::Clear))
                        .map(|s| re_ui::plot_legend::LegendEntry {
                            id: s.id(),
                            label: s.label.clone(),
                            color: s.color,
                            visible: s.visible,
                            hovered: label_hovered
                                .get(s.label.as_str())
                                .copied()
                                .unwrap_or(false),
                        }),
                );

                // Legend hover → global item hover (only when no plot item is hovered).
                if hovered_data_result.is_none()
                    && let Some(hovered_id) = legend_output.hovered_id
                    && let Some(address) = plot_item_id_to_data_result_address.get(&hovered_id)
                {
                    ctx.selection_state()
                        .set_hovered(re_viewer_context::Item::DataResult(address.clone()));
                }

                let hidden_items = legend_output.hidden_ids;
                update_series_visibility_from_legend(ctx, query, &all_plot_series, &hidden_items);
            }

            Ok(())
        })
        .inner
    }
}

fn all_scalar_mappings_for(
    matches: &IntMap<ComponentIdentifier, DatatypeMatch>,
) -> Vec<RecommendedMappings> {
    let target = Scalars::descriptor_scalars().component;

    matches
        .iter()
        .sorted_by_key(|(k, _)| **k)
        .flat_map(|(source_component, match_info)| match match_info {
            DatatypeMatch::NativeSemantics { .. } => {
                Either::Left(std::iter::once(RecommendedMappings::new(
                    target,
                    VisualizerComponentSource::SourceComponent {
                        source_component: *source_component,
                        selector: String::new(),
                    },
                )))
            }

            DatatypeMatch::PhysicalDatatypeOnly { selectors, .. } => {
                if selectors.is_empty() {
                    Either::Left(std::iter::once(RecommendedMappings::new(
                        target,
                        VisualizerComponentSource::SourceComponent {
                            source_component: *source_component,
                            selector: String::new(),
                        },
                    )))
                } else {
                    Either::Right(selectors.iter().map(|(selector, _datatype)| {
                        RecommendedMappings::new(
                            target,
                            VisualizerComponentSource::SourceComponent {
                                source_component: *source_component,
                                selector: selector.to_string(),
                            },
                        )
                    }))
                }
            }
        })
        .collect()
}

/// Renders the active visualizer pills for the "Visualizers" section in the selection panel.
fn visualizers_section_ui(ui: &mut egui::Ui, ctx: &re_viewer_context::ViewContext<'_>) {
    list_item::list_item_scope(ui, "time_series_visualizers_ui", |ui| {
        re_tracing::profile_function!();
        let query_result = ctx.query_result;

        let handles = query_result.tree.data_results_by_path.values().sorted();
        for handle in handles {
            let Some(node) = query_result.tree.data_results.get(*handle) else {
                continue;
            };
            let pill_margin = egui::Margin::symmetric(8, 6);
            for instruction in &node.data_result.visualizer_instructions {
                if !node.data_result.visible {
                    continue;
                }

                ui.add_space(10.0);

                crate::visualizer_ui::visualizer_ui_element(
                    ui,
                    ctx,
                    node,
                    pill_margin,
                    instruction,
                );
            }
        }
    });
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
        DataType::UInt64 => 4,
        DataType::UInt32 => 6,
        DataType::UInt16 => 8,
        DataType::UInt8 => 10,
        DataType::Boolean => 11,
        _ => 100, // Any other type gets lowest priority
    }
}

const RECOMMENDED_DATATYPES: &[DataType] =
    &[DataType::Float64, DataType::Float32, DataType::Float16];

fn all_scalar_mappings(
    reason: &VisualizableReason,
) -> impl Iterator<Item = (ComponentIdentifier, VisualizerComponentSource)> {
    let re_viewer_context::VisualizableReason::SingleRequiredComponentMatch(m) = reason else {
        return Either::Left(std::iter::empty());
    };
    let matches = &m.matches;

    let target = Scalars::descriptor_scalars();

    // Flatten all (component, selector) pairs into a single comparable list
    // to find the globally best match across all components.
    let candidates = matches.iter().flat_map(|(source_component, match_info)| {
        let is_rerun_native_type = match_info.component_type() == target.component_type.as_ref();

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

fn paint_time_cursor(
    ctx: &ViewerContext<'_>,
    ui: &egui::Ui,
    response: &egui::Response,
    transform: &egui_plot::PlotTransform,
    time_offset: i64,
    mut time_x: f32,
) {
    let interact_radius = ui.style().interaction.resize_grab_radius_side;
    let line_rect = egui::Rect::from_x_y_ranges(time_x..=time_x, response.rect.y_range())
        .expand(interact_radius);

    let time_drag_id = ui.id().with("time_drag");
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let is_near = ui.rect_contains_pointer(line_rect);
    let is_being_dragged = ui.is_being_dragged(time_drag_id);

    if is_near || is_being_dragged {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }

    if is_near
        && !is_being_dragged
        && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
    {
        ui.set_dragged_id(time_drag_id);
    }

    if is_being_dragged && let Some(pointer_pos) = pointer_pos {
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

    let highlighted = is_near || is_being_dragged;
    let style = if is_being_dragged {
        &ui.visuals().widgets.active
    } else if highlighted {
        &ui.visuals().widgets.hovered
    } else {
        &ui.visuals().widgets.inactive
    };

    ui.paint_time_cursor_with_style(ui.painter(), style, time_x, response.rect.y_range());
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

fn update_series_visibility_from_legend(
    ctx: &ViewerContext<'_>,
    query: &ViewQuery<'_>,
    all_plot_series: &[&crate::PlotSeries],
    hidden_items: &egui::IdSet,
) {
    let Some(query_results) = ctx.query_results.get(&query.view_id) else {
        return;
    };

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
            PlotSeriesKind::Continuous | PlotSeriesKind::Stepped(_) => {
                Some(SeriesLines::descriptor_visible_series())
            }
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
                .map(Visible::from)
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
    scalar_range: &mut Option<Range1D>,
) {
    re_tracing::profile_function!();

    *scalar_range = None;

    for series in all_plot_series {
        let points = if series.visible {
            series
                .points
                .iter()
                .map(|p| {
                    if let Some(scalar_range) = scalar_range.as_mut() {
                        if p.1 < scalar_range.start() {
                            *scalar_range.start_mut() = p.1;
                        }
                        if p.1 > scalar_range.end() {
                            *scalar_range.end_mut() = p.1;
                        }
                    } else {
                        *scalar_range = Some(Range1D::new(p.1, p.1));
                    }

                    [(p.0.saturating_sub(time_offset)) as _, p.1]
                })
                .collect::<Vec<_>>()
        } else {
            continue; // Skip rendering hidden series.
        };

        let color = series.color;

        let interaction_highlight = highlights
            .entity_highlight(series.instance_path.entity_path.hash())
            .index_highlight(
                series.instance_path.instance,
                series.visualizer_instruction_id,
            );
        let highlight = interaction_highlight.any();

        match series.kind {
            PlotSeriesKind::Continuous => plot_ui.line(
                Line::new(&series.label, points)
                    .color(color)
                    .width(2.0 * series.radius_ui)
                    .highlight(highlight)
                    .id(series.id()),
            ),
            PlotSeriesKind::Stepped(mode) => {
                let stepped_points = to_stepped_points(&points, mode);
                plot_ui.line(
                    Line::new(&series.label, stepped_points)
                        .color(color)
                        .width(2.0 * series.radius_ui)
                        .highlight(highlight)
                        .id(series.id()),
                );
            }
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

fn to_stepped_points(points: &[[f64; 2]], mode: crate::StepMode) -> Vec<[f64; 2]> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let capacity = match mode {
        crate::StepMode::After | crate::StepMode::Before => points.len() * 2 - 1,
        crate::StepMode::Mid => points.len() * 3 - 2,
    };
    let mut stepped = Vec::with_capacity(capacity);
    match mode {
        crate::StepMode::After => {
            for pair in points.windows(2) {
                stepped.push(pair[0]);
                stepped.push([pair[1][0], pair[0][1]]);
            }
        }
        crate::StepMode::Before => {
            for pair in points.windows(2) {
                stepped.push(pair[0]);
                stepped.push([pair[0][0], pair[1][1]]);
            }
        }
        crate::StepMode::Mid => {
            for pair in points.windows(2) {
                let mid_t = (pair[0][0] + pair[1][0]) * 0.5;
                stepped.push(pair[0]);
                stepped.push([mid_t, pair[0][1]]);
                stepped.push([mid_t, pair[1][1]]);
            }
        }
    }
    if let Some(last) = points.last() {
        stepped.push(*last);
    }
    stepped
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

/// Add a relative margin to a range so the data doesn't touch the plot edges.
///
/// `fraction` is the fraction of the range to add on each side (e.g. 0.05 = 5%).
pub fn add_margin_to_range(range: Range1D, fraction: f64) -> Range1D {
    let span = range.end() - range.start();
    let margin = span * fraction;
    Range1D::new(range.start() - margin, range.end() + margin)
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

#[cfg(test)]
mod tests {
    use super::*;
    use re_viewer_context::SingleRequiredComponentMatch;

    #[test]
    fn test_help_view() {
        re_test_context::TestContext::test_help_view(|ctx| TimeSeriesView.help(ctx));
    }

    /// Regression: non-recommended physical datatype (`Int32`) must not cause
    /// `SeriesLinesSystem` to be recommended with an empty mapping.
    #[test]
    fn test_no_recommendation_for_non_recommended_datatype() {
        let entity_path = EntityPath::from("sensor/data");
        let reason =
            VisualizableReason::SingleRequiredComponentMatch(SingleRequiredComponentMatch {
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
            });
        let visualizers = [(SeriesLinesSystem::identifier(), &reason)];
        let indicated = PerVisualizerType::default();
        let result = TimeSeriesView.recommended_visualizers_for_entity(
            &entity_path,
            &visualizers,
            &indicated,
        );

        assert!(result.all_recommendations().is_empty());
    }

    /// `SeriesLinesSystem` should be recommended when the datatype is a recommended one, even if not indicated.
    #[test]
    fn test_recommendation_for_recommended_datatype() {
        let entity_path = EntityPath::from("sensor/data");
        let reason =
            VisualizableReason::SingleRequiredComponentMatch(SingleRequiredComponentMatch {
                target_component: Scalars::descriptor_scalars().component,
                matches: std::iter::once((
                    Scalars::descriptor_scalars().component,
                    DatatypeMatch::NativeSemantics {
                        arrow_datatype: DataType::Float64,
                        component_type: None,
                    },
                ))
                .collect(),
            });
        let visualizers = [(SeriesLinesSystem::identifier(), &reason)];
        let indicated = PerVisualizerType::default();
        let result = TimeSeriesView.recommended_visualizers_for_entity(
            &entity_path,
            &visualizers,
            &indicated,
        );

        assert!(
            result
                .all_recommendations()
                .contains_key(&SeriesLinesSystem::identifier())
        );
        let mappings = &result.all_recommendations()[&SeriesLinesSystem::identifier()];
        assert_eq!(mappings.len(), 1);
        assert!(
            mappings[0].contains_mapping_for_component(&Scalars::descriptor_scalars().component)
        );
    }
}
