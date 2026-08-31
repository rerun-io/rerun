use re_log_types::AbsoluteTimeRange;
use re_log_types::external::arrow;
use re_sdk_types::blueprint::archetypes::TimeAxis;
use re_sdk_types::blueprint::components::{LinkAxis, VisualizerInstructionId};
use re_sdk_types::components::AggregationPolicy;
use re_viewer_context::external::re_entity_db::InstancePath;
use re_viewer_context::{ViewContext, ViewQuery, ViewerContext};
use re_viewport_blueprint::{ViewProperty, ViewPropertyQueryError};

use crate::aggregation::{AverageAggregator, MinMaxAggregator};
use crate::{PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind};

pub fn series_supported_encodings() -> impl IntoIterator<Item = arrow::datatypes::DataType> {
    [
        arrow::datatypes::DataType::Float32,
        arrow::datatypes::DataType::Float64,
        arrow::datatypes::DataType::Int8,
        arrow::datatypes::DataType::Int16,
        arrow::datatypes::DataType::Int32,
        arrow::datatypes::DataType::Int64,
        arrow::datatypes::DataType::UInt8,
        arrow::datatypes::DataType::UInt16,
        arrow::datatypes::DataType::UInt32,
        arrow::datatypes::DataType::UInt64,
        arrow::datatypes::DataType::Boolean,
    ]
}

/// The overlap of an entity's query range with the range we have data on the entity for in the store.
pub fn data_result_time_range(
    ctx: &ViewerContext<'_>,
    data_result: &re_viewer_context::DataResult,
    timeline: re_log_types::TimelineName,
) -> AbsoluteTimeRange {
    let query_range = match data_result.query_range() {
        re_viewer_context::QueryRange::TimeRange(time_range) => {
            re_view::resolve_visible_time_range(ctx, time_range)
        }

        re_viewer_context::QueryRange::LatestAt => {
            // Latest-at doesn't make sense for time series and should also never happen.
            re_log::debug_warn_once!(
                "Unexpected LatestAt query for time series data result at path {:?}",
                data_result.entity_path
            );
            AbsoluteTimeRange::EVERYTHING
        }
    };

    let data_range = ctx
        .recording()
        .storage_engine()
        .store()
        .entity_time_range(&timeline, &data_result.entity_path);

    query_range
        .intersection(data_range.unwrap_or(AbsoluteTimeRange::EMPTY))
        .unwrap_or(AbsoluteTimeRange::EMPTY)
}

/// The range we should be using for queries in time series visualizers.
///
/// This cuts the configured time range with what the view is looking at right now.
pub fn determine_query_range(
    ctx: &ViewContext<'_>,
    configured_time_range: AbsoluteTimeRange,
) -> Result<AbsoluteTimeRange, ViewPropertyQueryError> {
    let current_time = ctx
        .viewer_ctx
        .time_ctrl
        .time_int()
        .unwrap_or(re_log_types::TimeInt::ZERO);

    let time_axis = ViewProperty::from_archetype::<TimeAxis>(ctx);

    let link_x_axis =
        time_axis.component_or_fallback::<LinkAxis>(ctx, TimeAxis::descriptor_link().component)?;

    let time_range_property = match link_x_axis {
        LinkAxis::Independent => &time_axis,
        LinkAxis::LinkToGlobal => &ViewProperty::from_archetype_for_view::<TimeAxis>(
            ctx.viewer_ctx,
            re_viewer_context::GLOBAL_VIEW_ID,
        ),
    };

    let view_time_range = time_range_property
        .component_or_fallback::<re_sdk_types::blueprint::components::TimeRange>(
            ctx,
            re_sdk_types::blueprint::archetypes::TimeAxis::descriptor_view_range().component,
        )?;

    let view_time_range =
        AbsoluteTimeRange::from_relative_time_range(&view_time_range, current_time);

    Ok(view_time_range
        .intersection(configured_time_range)
        .unwrap_or(AbsoluteTimeRange::EMPTY))
}

/// Everything about one series that a run of points inherits.
///
/// A series may be split into several [`PlotSeries`] runs, each of which is built from this.
pub struct SeriesProperties {
    pub instance_path: InstancePath,
    pub label: String,

    /// Unit of the values, e.g. `"Pa"`, shown in the legend and in tooltips.
    pub unit: Option<String>,

    pub visible: bool,

    /// Whether this series has any non-zero variance, and therefore an error band.
    ///
    /// Known from the variance column, so runs never have to look for it point by point.
    pub has_variances: bool,

    pub visualizer_instruction_id: VisualizerInstructionId,
}

// We have a bunch of raw points, and now we need to group them into individual series.
// A series is a continuous run of points with identical attributes: each time
// we notice a change in attributes, we need a new series.
pub fn points_to_series(
    run: SeriesProperties,
    time_per_pixel: f64,
    points: Vec<PlotPoint>,
    store: &re_chunk_store::ChunkStore,
    query: &ViewQuery<'_>,
    aggregator: AggregationPolicy,
    all_series: &mut Vec<PlotSeries>,
) {
    re_tracing::profile_function!(&run.instance_path.to_string());

    if points.is_empty() {
        // No values being present is not an error, maybe data comes in later!
        return;
    }

    let (aggregation_factor, points) = apply_aggregation(aggregator, time_per_pixel, points, query);
    let min_time = store
        .entity_min_time(&query.timeline, &run.instance_path.entity_path)
        .map_or_else(
            || points.first().map_or(0, |p| p.time),
            |time| time.as_i64(),
        );

    add_series_runs(
        run,
        points,
        aggregator,
        aggregation_factor,
        min_time,
        all_series,
    );
}

/// Apply the given aggregation to the provided points.
pub fn apply_aggregation(
    aggregator: AggregationPolicy,
    time_per_pixel: f64,
    points: Vec<PlotPoint>,
    query: &ViewQuery<'_>,
) -> (f64, Vec<PlotPoint>) {
    // Aggregate over this many time units.
    //
    // MinMax does zig-zag between min and max, which causes a very jagged look.
    // It can be mitigated by lowering the aggregation duration, but that causes
    // a lot more work for the tessellator and renderer.
    // TODO(#4969): output a thicker line instead of zig-zagging.
    let aggregation_duration = time_per_pixel; // aggregate all points covering one physical pixel

    // So it can be displayed in the UI by the ViewClass.
    let num_points_before = points.len() as f64;

    // If the user logged multiples scalars per time stamp, we should aggregate them,
    // no matter what the aggregation duration (=zoom level) is.
    let multiple_values_per_time_stamp = || points.array_windows().any(|[a, b]| a.time == b.time);

    let should_aggregate = aggregator != AggregationPolicy::Off
        && (2.0 <= aggregation_duration || multiple_values_per_time_stamp());

    let points = if should_aggregate {
        re_tracing::profile_scope!("aggregate", aggregator.to_string());

        match aggregator {
            AggregationPolicy::Off => points,
            AggregationPolicy::Average => {
                AverageAggregator::aggregate(aggregation_duration, &points)
            }
            AggregationPolicy::Min => {
                MinMaxAggregator::Min.aggregate(aggregation_duration, &points)
            }
            AggregationPolicy::Max => {
                MinMaxAggregator::Max.aggregate(aggregation_duration, &points)
            }
            AggregationPolicy::MinMax => {
                MinMaxAggregator::MinMax.aggregate(aggregation_duration, &points)
            }
            AggregationPolicy::MinMaxAverage => {
                MinMaxAggregator::MinMaxAverage.aggregate(aggregation_duration, &points)
            }
        }
    } else {
        points
    };

    let num_points_after = points.len() as f64;
    let actual_aggregation_factor = num_points_before / num_points_after;

    re_log::trace!(
        id = %query.view_id,
        ?aggregator,
        aggregation_duration,
        num_points_before,
        num_points_after,
        actual_aggregation_factor,
    );

    (actual_aggregation_factor, points)
}

#[expect(clippy::needless_pass_by_value)]
#[inline(never)] // Better callstacks on crashes
fn add_series_runs(
    run: SeriesProperties,
    points: Vec<PlotPoint>,
    aggregator: AggregationPolicy,
    aggregation_factor: f64,
    min_time: i64,
    all_series: &mut Vec<PlotSeries>,
) {
    re_tracing::profile_function!();

    let num_points = points.len();

    // Series without a band carry no variances at all.
    let push = |series: &mut PlotSeries, time: i64, value: f64, variance: f32| {
        series.push_point(time, value);
        if run.has_variances {
            series.variances.push(variance);
        }
    };

    let new_series = |attrs: &PlotPointAttrs, capacity: usize| PlotSeries {
        instance_path: run.instance_path.clone(),
        visible: run.visible,
        label: run.label.clone(),
        color: attrs.color,
        radius_ui: attrs.radius_ui,
        kind: attrs.kind,
        points: Vec::with_capacity(capacity),
        value_range: None,
        aggregator,
        aggregation_factor,
        min_time,
        visualizer_instruction_id: run.visualizer_instruction_id,
        variances: Vec::new(),
        unit: run.unit.clone(),
    };

    let mut attrs = points[0].attrs.clone();
    let mut series: PlotSeries = new_series(&attrs, num_points);

    for (i, p) in points.into_iter().enumerate() {
        if p.attrs == attrs {
            // Same attributes, just add to the current series.
            push(&mut series, p.time, p.value, p.variance);
        } else {
            // Attributes changed since last point, break up the current run into a
            // its own series, and start the next one.

            let variance = p.variance;
            attrs = p.attrs;
            let prev_series = std::mem::replace(&mut series, new_series(&attrs, num_points - i));

            let cur_continuous = matches!(
                attrs.kind,
                PlotSeriesKind::Continuous | PlotSeriesKind::Stepped(_)
            );
            let prev_continuous = matches!(
                prev_series.kind,
                PlotSeriesKind::Continuous | PlotSeriesKind::Stepped(_)
            );

            #[expect(clippy::unwrap_used)] // prev_series.points can't be empty here
            let prev_point = *prev_series.points.last().unwrap();
            let prev_variance = prev_series.variances.last().copied().unwrap_or(0.0);
            all_series.push(prev_series);

            // If the previous point was continuous and the current point is continuous
            // too, then we want the 2 segments to appear continuous even though they
            // are actually split from a data standpoint.
            if cur_continuous && prev_continuous {
                push(&mut series, prev_point.0, prev_point.1, prev_variance);
            }

            // Add the point that triggered the split to the new segment.
            push(&mut series, p.time, p.value, variance);
        }
    }

    if !series.points.is_empty() {
        all_series.push(series);
    }
}
