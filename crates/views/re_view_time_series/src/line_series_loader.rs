//! Shared loading of line-rendered scalar series.
//!
//! Both [`crate::line_visualizer_system::SeriesLinesSystem`] and
//! [`crate::measurements_visualizer_system::MeasurementsSeriesSystem`] draw a line per series and
//! differ only in where their values and styling come from, which is captured in [`LineSeriesSource`].

use itertools::Itertools as _;
use re_log_types::TimeInt;
use re_sdk_types::components::{AggregationPolicy, InterpolationMode, StrokeWidth};
use re_sdk_types::reflection::Enum as _;
use re_sdk_types::{ComponentDescriptor, ComponentIdentifier, archetypes};
use re_view::{ChunksWithComponent, collect_recursive_clears, range_with_blueprint_resolved_data};
use re_viewer_context::external::re_entity_db::InstancePath;
use re_viewer_context::{ViewContext, ViewQuery, ViewerReportSeverity, typed_fallback_for};

use crate::series_query::{
    allocate_plot_points, collect_colors, collect_radius_ui, collect_scalars, collect_series_name,
    collect_series_visibility, determine_num_series,
};
use crate::{PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind, util};

/// Per-visualizer query configuration for [`load_line_series_with_styling`].
pub(crate) struct LineSeriesSource {
    /// Descriptor of the required scalar value column (e.g. `Scalars.scalars` or `Measurements.values`).
    pub value_descriptor: ComponentDescriptor,

    /// All component identifiers to fetch in the range query.
    pub queried_components: Vec<ComponentIdentifier>,

    /// Where the styling columns come from: `SeriesLines`, or the value archetype itself.
    pub styling: LineSeriesStyling,

    /// If set, [`PlotSeries::variances`] is filled from this column and drawn as a band.
    pub variance_descriptor: Option<ComponentDescriptor>,

    /// If set, [`PlotSeries::unit`] is filled from this column and shown in the legend and tooltip.
    pub unit_descriptor: Option<ComponentDescriptor>,
}

/// The styling columns a line series is drawn with.
#[derive(Clone)]
pub(crate) struct LineSeriesStyling {
    pub colors: ComponentDescriptor,
    pub widths: ComponentDescriptor,
    pub names: ComponentDescriptor,
    pub visible_series: ComponentDescriptor,
    pub aggregation_policy: ComponentDescriptor,
    pub interpolation_mode: ComponentDescriptor,
}

impl LineSeriesStyling {
    /// All styling columns, in no particular order.
    pub fn component_identifiers(&self) -> impl Iterator<Item = ComponentIdentifier> {
        let Self {
            colors,
            widths,
            names,
            visible_series,
            aggregation_policy,
            interpolation_mode,
        } = self;

        [
            colors,
            widths,
            names,
            visible_series,
            aggregation_policy,
            interpolation_mode,
        ]
        .into_iter()
        .map(|descriptor| descriptor.component)
    }

    pub fn series_lines() -> Self {
        Self {
            colors: archetypes::SeriesLines::descriptor_colors(),
            widths: archetypes::SeriesLines::descriptor_widths(),
            names: archetypes::SeriesLines::descriptor_names(),
            visible_series: archetypes::SeriesLines::descriptor_visible_series(),
            aggregation_policy: archetypes::SeriesLines::descriptor_aggregation_policy(),
            interpolation_mode: archetypes::SeriesLines::descriptor_interpolation_mode(),
        }
    }

    pub fn measurements() -> Self {
        Self {
            colors: archetypes::Measurements::descriptor_colors(),
            widths: archetypes::Measurements::descriptor_widths(),
            names: archetypes::Measurements::descriptor_names(),
            visible_series: archetypes::Measurements::descriptor_visible_series(),
            aggregation_policy: archetypes::Measurements::descriptor_aggregation_policy(),
            interpolation_mode: archetypes::Measurements::descriptor_interpolation_mode(),
        }
    }
}

/// Shared `load_series` implementation for line-rendered scalar archetypes.
///
/// Everything that differs per visualizer is captured in [`LineSeriesSource`].
pub(crate) fn load_line_series_with_styling(
    ctx: &ViewContext<'_>,
    view_query: &ViewQuery<'_>,
    time_per_pixel: f64,
    data_result: &re_viewer_context::DataResult,
    instruction: &re_viewer_context::VisualizerInstruction,
    output: &re_viewer_context::VisualizerExecutionOutput,
    source: &LineSeriesSource,
) -> Vec<PlotSeries> {
    re_tracing::profile_function!(data_result.entity_path.to_string());

    let current_query = ctx.current_query();
    let query_ctx = ctx.query_context(data_result, current_query.clone(), instruction.id);

    let data_time_range =
        util::data_result_time_range(ctx.viewer_ctx, data_result, view_query.timeline);
    let query_range = match util::determine_query_range(ctx, data_time_range) {
        Ok(range) => range,
        Err(err) => {
            output.report_unspecified_source(
                instruction.id,
                ViewerReportSeverity::Error,
                format!("Failed to determine query range: {err}"),
            );
            return Vec::new();
        }
    };
    let query = re_chunk_store::RangeQuery::new(view_query.timeline, query_range)
        // We must fetch data with extended bounds, otherwise the query clamping would
        // cut-off the data early at the edge of the view.
        .include_extended_bounds(true);

    let mut results = range_with_blueprint_resolved_data(
        ctx,
        None,
        &query,
        data_result,
        source.queried_components.iter().copied(),
        instruction,
    );

    // The plot view visualizes scalar data within a specific time range, without any kind
    // of time-alignment / bootstrapping behavior:
    // * For the scalar themselves, this is what you want: if you're trying to plot some
    //   data between t=100 and t=200, you don't want to display a point from t=20 (and
    //   _extended bounds_ will take care of lines crossing the limit).
    // * For the secondary components (colors, radii, names, etc), this is a problem
    //   though: you don't want your plot to change color depending on what the currently
    //   visible time range is! Secondary components have to be bootstrapped.
    //
    // Bootstrapping is now handled automatically by the query system for the components
    // we specified when calling range_with_blueprint_resolved_data.
    //
    // Styling and units bootstrap; values and variances are data, drawn only where logged.
    let bootstrap_components = std::iter::chain(
        source.styling.component_identifiers(),
        source.unit_descriptor.iter().map(|d| d.component),
    );

    results.merge_bootstrapped_data(re_view::latest_at_with_blueprint_resolved_data(
        ctx,
        None,
        &re_chunk_store::LatestAtQuery::new(query.timeline, query.range.min()),
        data_result,
        bootstrap_components,
        Some(instruction),
    ));

    // Wrap results for convenient error-reporting iteration
    let results = re_view::BlueprintResolvedResults::Range(query.clone(), results);
    let results = re_view::VisualizerInstructionQueryResults::new(instruction, &results, output);

    // If we have no scalars, we can't do anything.
    let scalar_component = source.value_descriptor.component;
    let scalar_iter = results.iter_required(scalar_component);
    let all_scalar_chunks = scalar_iter.chunks();

    // Filter out static times if any slipped in.
    // It's enough to check the first one chunk since an entire column has to be either temporal or static.
    let empty_chunks;
    let all_scalar_chunks = if let Some(chunk) = all_scalar_chunks.chunks.first()
        && chunk.is_static()
    {
        results.report_for_component(scalar_component, ViewerReportSeverity::Error, "Can't plot data that was logged statically in a time series since there's no temporal dimension");
        empty_chunks = ChunksWithComponent::empty(scalar_component);
        &empty_chunks // Proceed with empty data so we catch other errors as well.
    } else {
        all_scalar_chunks
    };

    // All the default values for a `PlotPoint`, accounting for both overrides and default values.
    // We know there's only a single value fallback for stroke width, so this is fine, albeit a bit hacky in case we add an array fallback later.
    let fallback_stroke_width: StrokeWidth =
        typed_fallback_for(&query_ctx, source.styling.widths.component);

    let interpolation_mode = results
        .iter_optional(source.styling.interpolation_mode.component)
        .slice::<u8>()
        .next()
        .and_then(|(_, s)| InterpolationMode::from_integer_slice(s).next()?)
        .unwrap_or_default();

    let plot_kind = match interpolation_mode {
        InterpolationMode::Linear => PlotSeriesKind::Continuous,
        InterpolationMode::StepAfter => PlotSeriesKind::Stepped(crate::StepMode::After),
        InterpolationMode::StepBefore => PlotSeriesKind::Stepped(crate::StepMode::Before),
        InterpolationMode::StepMid => PlotSeriesKind::Stepped(crate::StepMode::Mid),
    };

    let default_point = PlotPoint {
        time: 0,
        value: 0.0,
        variance: 0.0,
        attrs: PlotPointAttrs {
            // Filled out later.
            color: egui::Color32::DEBUG_COLOR,
            radius_ui: 0.5 * *fallback_stroke_width.0,
            kind: plot_kind,
        },
    };

    let num_series = determine_num_series(
        all_scalar_chunks,
        &results,
        source.value_descriptor.component,
    );
    let mut points_per_series =
        allocate_plot_points(&query, &default_point, all_scalar_chunks, num_series);

    collect_scalars(all_scalar_chunks, &results, &mut points_per_series);

    collect_colors(
        &query,
        &results,
        all_scalar_chunks,
        &mut points_per_series,
        &source.styling.colors,
    );
    collect_radius_ui(
        &query,
        &results,
        all_scalar_chunks,
        &mut points_per_series,
        &source.styling.widths,
        0.5,
    );

    let has_variances = source
        .variance_descriptor
        .as_ref()
        .is_some_and(|variance_descriptor| {
            crate::series_query::collect_variances(
                &query,
                &results,
                all_scalar_chunks,
                &mut points_per_series,
                variance_descriptor,
            )
        });

    // Now convert the `PlotPoints` into `Vec<PlotSeries>`
    let aggregator = results
        .iter_optional(source.styling.aggregation_policy.component)
        .slice::<u8>()
        .next()
        .and_then(|(_, s)| AggregationPolicy::from_integer_slice(s).next()?)
        // TODO(andreas): Relying on the default==placeholder here instead of going through a fallback provider.
        //                This is fine, because we know there's no `TypedFallbackProvider`, but wrong if one were to be added.
        .unwrap_or_default();

    // NOTE: The chunks themselves are already sorted as best as possible (hint: overlap)
    // by the query engine.
    let all_chunks_sorted_and_not_overlapped =
        all_scalar_chunks.iter().tuple_windows().all(|(lhs, rhs)| {
            let lhs_time_max = lhs
                .chunk
                .timelines()
                .get(query.timeline())
                .map_or(TimeInt::MAX, |time_column| time_column.time_range().max());
            let rhs_time_min = rhs
                .chunk
                .timelines()
                .get(query.timeline())
                .map_or(TimeInt::MIN, |time_column| time_column.time_range().min());
            lhs_time_max <= rhs_time_min
        });

    let has_discontinuities = {
        // Find all clears that may apply, in order to render discontinuities properly.

        re_tracing::profile_scope!("discontinuities");

        let cleared_indices = collect_recursive_clears(ctx, &query, &data_result.entity_path);
        let has_discontinuities = !cleared_indices.is_empty();

        for points in &mut points_per_series {
            points.extend(cleared_indices.iter().map(|(data_time, _)| PlotPoint {
                time: data_time.as_i64(),
                value: 0.0,
                variance: 0.0,
                attrs: PlotPointAttrs {
                    color: egui::Color32::TRANSPARENT,
                    radius_ui: 0.0,
                    kind: PlotSeriesKind::Clear,
                },
            }));
        }

        has_discontinuities
    };

    // This is _almost_ sorted already: all the individual chunks are sorted, but we still
    // have to deal with overlapped chunks, or discontinuities introduced by query-time clears.
    if !all_chunks_sorted_and_not_overlapped || has_discontinuities {
        re_tracing::profile_scope!("sort");
        for points in &mut points_per_series {
            re_tracing::profile_scope!("sort_by_key", points.len().to_string());
            points.sort_by_key(|p| p.time);
        }
    }

    let series_visibility =
        collect_series_visibility(&results, num_series, &source.styling.visible_series);
    let series_names = collect_series_name(&results, num_series, &source.styling.names);
    let series_units = source.unit_descriptor.as_ref().map_or_else(
        || vec![None; num_series],
        |unit_descriptor| {
            crate::series_query::collect_series_units(&results, num_series, unit_descriptor)
        },
    );

    let mut series = Vec::with_capacity(num_series);

    re_log::debug_assert!(
        points_per_series.len() <= series_names.len(),
        "Number of series names {} after processing should be at least the number of series allocated {}",
        series_names.len(),
        points_per_series.len()
    );
    for (instance, (points, label, unit, visible)) in itertools::izip!(
        points_per_series,
        series_names,
        series_units,
        series_visibility
    )
    .enumerate()
    {
        let instance_path = if num_series == 1 {
            InstancePath::entity_all(data_result.entity_path.clone())
        } else {
            InstancePath::instance(data_result.entity_path.clone(), instance as u64)
        };

        util::points_to_series(
            util::SeriesProperties {
                instance_path,
                label,
                unit,
                visible,
                has_variances,
                visualizer_instruction_id: instruction.id,
            },
            time_per_pixel,
            points,
            ctx.recording_engine().store(),
            view_query,
            aggregator,
            &mut series,
        );
    }

    series
}
