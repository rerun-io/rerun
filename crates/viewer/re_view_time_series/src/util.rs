use re_log_types::AbsoluteTimeRange;
use re_log_types::external::arrow;
use re_sdk_types::blueprint::archetypes::TimeAxis;
use re_sdk_types::blueprint::components::{LinkAxis, VisualizerInstructionId};
use re_sdk_types::components::AggregationPolicy;
use re_sdk_types::datatypes::TimeRange;
use re_viewer_context::external::re_entity_db::InstancePath;
use re_viewer_context::{ViewContext, ViewQuery, ViewerContext};
use re_viewport_blueprint::{ViewProperty, ViewPropertyQueryError};

use crate::aggregation::{AverageAggregator, MinMaxAggregator};
use crate::{PlotPoint, PlotSeries, PlotSeriesKind, ScatterAttrs};

pub fn series_supported_datatypes() -> impl IntoIterator<Item = arrow::datatypes::DataType> {
    [
        arrow::datatypes::DataType::Float32,
        arrow::datatypes::DataType::Float64,
        arrow::datatypes::DataType::Int8,
        arrow::datatypes::DataType::Int32,
        arrow::datatypes::DataType::Int64,
        arrow::datatypes::DataType::UInt8,
        arrow::datatypes::DataType::UInt32,
        arrow::datatypes::DataType::UInt64,
        // TODO(andreas): Support bool types?
    ]
}

/// Find the number of time units per physical pixel.
pub fn determine_time_per_pixel(
    ctx: &ViewerContext<'_>,
    plot_mem: Option<&egui_plot::PlotMemory>,
) -> f64 {
    let egui_ctx = ctx.egui_ctx();

    // How many ui points per time unit?
    let points_per_time = plot_mem
        .as_ref()
        .map_or(1.0, |mem| mem.transform().dpos_dvalue_x());
    let pixels_per_time = egui_ctx.pixels_per_point() as f64 * points_per_time;

    // How many time units per physical pixel?
    1.0 / pixels_per_time.max(f64::EPSILON)
}

/// The views' visible time range
pub fn determine_visible_time_range(
    ctx: &ViewContext<'_>,
    data_result: &re_viewer_context::DataResult,
) -> AbsoluteTimeRange {
    let current_time = ctx
        .viewer_ctx
        .time_ctrl
        .time_int()
        .unwrap_or(re_log_types::TimeInt::ZERO);

    let visible_time_range = match data_result.query_range() {
        re_viewer_context::QueryRange::TimeRange(time_range) => *time_range,

        re_viewer_context::QueryRange::LatestAt => {
            // Latest-at doesn't make sense for time series and should also never happen.
            re_log::debug_warn_once!(
                "Unexpected LatestAt query for time series data result at path {:?}",
                data_result.entity_path
            );
            TimeRange::EVERYTHING
        }
    };

    AbsoluteTimeRange::from_relative_time_range(&visible_time_range, current_time)
}

/// The currently visible range of data
pub fn determine_time_range(
    ctx: &ViewContext<'_>,
    visible_time_range: AbsoluteTimeRange,
) -> Result<AbsoluteTimeRange, ViewPropertyQueryError> {
    let current_time = ctx
        .viewer_ctx
        .time_ctrl
        .time_int()
        .unwrap_or(re_log_types::TimeInt::ZERO);

    let time_axis = ViewProperty::from_archetype::<TimeAxis>(
        ctx.viewer_ctx.blueprint_db(),
        ctx.viewer_ctx.blueprint_query,
        ctx.view_id,
    );

    let link_x_axis =
        time_axis.component_or_fallback::<LinkAxis>(ctx, TimeAxis::descriptor_link().component)?;

    let time_range_property = match link_x_axis {
        LinkAxis::Independent => &time_axis,
        LinkAxis::LinkToGlobal => &ViewProperty::from_archetype::<TimeAxis>(
            ctx.blueprint_db(),
            ctx.blueprint_query(),
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
        .intersection(visible_time_range)
        .unwrap_or(AbsoluteTimeRange::EMPTY))
}

// We have a bunch of raw points, and now we need to group them into individual series.
// A series is a continuous run of points with identical attributes: each time
// we notice a change in attributes, we need a new series.
#[expect(clippy::too_many_arguments)]
pub fn points_to_series(
    instance_path: InstancePath,
    visible_time_range: AbsoluteTimeRange,
    time_per_pixel: f64,
    visible: bool,
    points: Vec<PlotPoint>,
    store: &re_chunk_store::ChunkStore,
    query: &ViewQuery<'_>,
    series_label: String,
    aggregator: AggregationPolicy,
    all_series: &mut Vec<PlotSeries>,
    visualizer_instruction_id: VisualizerInstructionId,
) -> Result<(), String> {
    re_tracing::profile_function!(&instance_path.to_string());

    if points.is_empty() {
        // No values being present is not an error, maybe data comes in later!
        return Ok(());
    }

    // Filter out static times if any slipped in.
    // It's enough to check the first one since an entire column has to be either temporal or static.
    if let Some(first) = points.first()
        && first.time == re_log_types::TimeInt::STATIC.as_i64()
    {
        return Err("Can't plot data that was logged statically in a time series since there's no temporal dimension.".to_owned());
    }

    let (aggregation_factor, points) = apply_aggregation(aggregator, time_per_pixel, points, query);
    let min_time = store
        .entity_min_time(&query.timeline, &instance_path.entity_path)
        .map_or_else(
            || points.first().map_or(0, |p| p.time),
            |time| time.as_i64(),
        );

    if points.len() == 1 {
        // Can't draw a single point as a continuous line, so fall back on scatter
        let mut kind = points[0].attrs.kind;
        if kind == PlotSeriesKind::Continuous {
            kind = PlotSeriesKind::Scatter(ScatterAttrs::default());
        }

        all_series.push(PlotSeries {
            instance_path,
            visible_time_range,
            visible,
            label: series_label,
            color: points[0].attrs.color,
            radius_ui: points[0].attrs.radius_ui,
            kind,
            points: vec![(points[0].time, points[0].value)],
            aggregator,
            aggregation_factor,
            min_time,
            visualizer_instruction_id,
        });
    } else {
        add_series_runs(
            instance_path,
            visible_time_range,
            visible,
            series_label,
            points,
            aggregator,
            aggregation_factor,
            min_time,
            all_series,
            visualizer_instruction_id,
        );
    }

    Ok(())
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
    let multiple_values_per_time_stamp = || points.windows(2).any(|w| w[0].time == w[1].time);

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

#[expect(clippy::too_many_arguments)]
#[expect(clippy::needless_pass_by_value)]
#[inline(never)] // Better callstacks on crashes
fn add_series_runs(
    instance_path: InstancePath,
    visible_time_range: AbsoluteTimeRange,
    visible: bool,
    series_label: String,
    points: Vec<PlotPoint>,
    aggregator: AggregationPolicy,
    aggregation_factor: f64,
    min_time: i64,
    all_series: &mut Vec<PlotSeries>,
    visualizer_instruction_id: VisualizerInstructionId,
) {
    re_tracing::profile_function!();

    let num_points = points.len();
    let mut attrs = points[0].attrs.clone();
    let mut series: PlotSeries = PlotSeries {
        instance_path: instance_path.clone(),
        visible_time_range,
        visible,
        label: series_label.clone(),
        color: attrs.color,
        radius_ui: attrs.radius_ui,
        points: Vec::with_capacity(num_points),
        kind: attrs.kind,
        aggregator,
        aggregation_factor,
        min_time,
        visualizer_instruction_id,
    };

    for (i, p) in points.into_iter().enumerate() {
        #[expect(clippy::branches_sharing_code)]
        if p.attrs == attrs {
            // Same attributes, just add to the current series.

            series.points.push((p.time, p.value));
        } else {
            // Attributes changed since last point, break up the current run into a
            // its own series, and start the next one.

            attrs = p.attrs;
            let prev_series = std::mem::replace(
                &mut series,
                PlotSeries {
                    instance_path: instance_path.clone(),
                    visible_time_range,
                    visible,
                    label: series_label.clone(),
                    color: attrs.color,
                    radius_ui: attrs.radius_ui,
                    kind: attrs.kind,
                    points: Vec::with_capacity(num_points - i),
                    aggregator,
                    aggregation_factor,
                    min_time,
                    visualizer_instruction_id,
                },
            );

            let cur_continuous = matches!(attrs.kind, PlotSeriesKind::Continuous);
            let prev_continuous = matches!(prev_series.kind, PlotSeriesKind::Continuous);

            #[expect(clippy::unwrap_used)] // prev_series.points can't be empty here
            let prev_point = *prev_series.points.last().unwrap();
            all_series.push(prev_series);

            // If the previous point was continuous and the current point is continuous
            // too, then we want the 2 segments to appear continuous even though they
            // are actually split from a data standpoint.
            if cur_continuous && prev_continuous {
                series.points.push(prev_point);
            }

            // Add the point that triggered the split to the new segment.
            series.points.push((p.time, p.value));
        }
    }

    if !series.points.is_empty() {
        all_series.push(series);
    }
}
