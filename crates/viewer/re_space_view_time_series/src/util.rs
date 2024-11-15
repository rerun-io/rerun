use re_log_types::{EntityPath, ResolvedTimeRange};
use re_types::{
    components::AggregationPolicy,
    datatypes::{TimeRange, TimeRangeBoundary},
};
use re_viewer_context::{ViewQuery, ViewerContext};

use crate::{
    aggregation::{AverageAggregator, MinMaxAggregator},
    PlotPoint, PlotSeries, PlotSeriesKind, ScatterAttrs,
};

/// Find the number of time units per physical pixel.
pub fn determine_time_per_pixel(
    ctx: &ViewerContext<'_>,
    plot_mem: Option<&egui_plot::PlotMemory>,
) -> f64 {
    let egui_ctx = ctx.egui_ctx;

    // How many ui points per time unit?
    let points_per_time = plot_mem
        .as_ref()
        .map_or(1.0, |mem| mem.transform().dpos_dvalue_x());
    let pixels_per_time = egui_ctx.pixels_per_point() as f64 * points_per_time;

    // How many time units per physical pixel?
    1.0 / pixels_per_time.max(f64::EPSILON)
}

pub fn determine_time_range(
    time_cursor: re_log_types::TimeInt,
    time_offset: i64,
    data_result: &re_viewer_context::DataResult,
    plot_mem: Option<&egui_plot::PlotMemory>,
) -> ResolvedTimeRange {
    let query_range = data_result.query_range();

    // Latest-at doesn't make sense for time series and should also never happen.
    let visible_time_range = match query_range {
        re_viewer_context::QueryRange::TimeRange(time_range) => time_range.clone(),
        re_viewer_context::QueryRange::LatestAt => {
            re_log::error_once!(
                "Unexexpected LatestAt query for time series data result at path {:?}",
                data_result.entity_path
            );
            TimeRange {
                start: TimeRangeBoundary::AT_CURSOR,
                end: TimeRangeBoundary::AT_CURSOR,
            }
        }
    };

    let mut time_range =
        ResolvedTimeRange::from_relative_time_range(&visible_time_range, time_cursor);

    let is_auto_bounds = plot_mem.map_or(false, |mem| mem.auto_bounds.x || mem.auto_bounds.y);
    let plot_bounds = plot_mem.map(|mem| {
        let bounds = mem.bounds().range_x();
        let x_min = bounds.start().floor() as i64;
        let x_max = bounds.end().ceil() as i64;
        // We offset the time values of the plot so that unix timestamps don't run out of precision.
        (
            x_min.saturating_add(time_offset),
            x_max.saturating_add(time_offset),
        )
    });

    // If we're not in auto mode, which is the mode where the query drives the bounds of the plot,
    // then we want the bounds of the plots to drive the query!
    if !is_auto_bounds {
        if let Some((x_min, x_max)) = plot_bounds {
            time_range.set_min(i64::max(time_range.min().as_i64(), x_min));
            time_range.set_max(i64::min(time_range.max().as_i64(), x_max));
        }
    }

    time_range
}

// We have a bunch of raw points, and now we need to group them into individual series.
// A series is a continuous run of points with identical attributes: each time
// we notice a change in attributes, we need a new series.
#[allow(clippy::too_many_arguments)]
pub fn points_to_series(
    entity_path: &EntityPath,
    time_per_pixel: f64,
    points: Vec<PlotPoint>,
    store: &re_chunk_store::ChunkStore,
    query: &ViewQuery<'_>,
    series_label: String,
    aggregator: AggregationPolicy,
    all_series: &mut Vec<PlotSeries>,
) {
    re_tracing::profile_scope!("secondary", &entity_path.to_string());
    if points.is_empty() {
        return;
    }

    let (aggregation_factor, points) = apply_aggregation(aggregator, time_per_pixel, points, query);
    let min_time = store
        .entity_min_time(&query.timeline, entity_path)
        .map_or(points.first().map_or(0, |p| p.time), |time| time.as_i64());

    if points.len() == 1 {
        // Can't draw a single point as a continuous line, so fall back on scatter
        let mut kind = points[0].attrs.kind;
        if kind == PlotSeriesKind::Continuous {
            kind = PlotSeriesKind::Scatter(ScatterAttrs::default());
        }

        all_series.push(PlotSeries {
            label: series_label,
            color: points[0].attrs.color,
            radius_ui: points[0].attrs.radius_ui,
            kind,
            points: vec![(points[0].time, points[0].value)],
            entity_path: entity_path.clone(),
            aggregator,
            aggregation_factor,
            min_time,
        });
    } else {
        add_series_runs(
            series_label,
            points,
            entity_path,
            aggregator,
            aggregation_factor,
            min_time,
            all_series,
        );
    }
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

    // So it can be displayed in the UI by the SpaceViewClass.
    let num_points_before = points.len() as f64;

    // If the user logged multiples scalars per time stamp, we should aggregate them,
    // no matter what the aggregation duration (=zoom level) is.
    let multiple_values_per_time_stamp = || points.windows(2).any(|w| w[0].time == w[1].time);

    let should_aggregate = aggregator != AggregationPolicy::Off
        && (2.0 <= aggregation_duration || multiple_values_per_time_stamp());

    let points = if should_aggregate {
        re_tracing::profile_scope!("aggregate", aggregator.to_string());

        #[allow(clippy::match_same_arms)] // readability
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
        id = %query.space_view_id,
        ?aggregator,
        aggregation_duration,
        num_points_before,
        num_points_after,
        actual_aggregation_factor,
    );

    (actual_aggregation_factor, points)
}

#[allow(clippy::needless_pass_by_value)]
#[inline(never)] // Better callstacks on crashes
fn add_series_runs(
    series_label: String,
    points: Vec<PlotPoint>,
    entity_path: &EntityPath,
    aggregator: AggregationPolicy,
    aggregation_factor: f64,
    min_time: i64,
    all_series: &mut Vec<PlotSeries>,
) {
    re_tracing::profile_function!();

    let num_points = points.len();
    let mut attrs = points[0].attrs.clone();
    let mut series: PlotSeries = PlotSeries {
        label: series_label.clone(),
        color: attrs.color,
        radius_ui: attrs.radius_ui,
        points: Vec::with_capacity(num_points),
        kind: attrs.kind,
        entity_path: entity_path.clone(),
        aggregator,
        aggregation_factor,
        min_time,
    };

    for (i, p) in points.into_iter().enumerate() {
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
                    label: series_label.clone(),
                    color: attrs.color,
                    radius_ui: attrs.radius_ui,
                    kind: attrs.kind,
                    points: Vec::with_capacity(num_points - i),
                    entity_path: entity_path.clone(),
                    aggregator,
                    aggregation_factor,
                    min_time,
                },
            );

            let cur_continuous = matches!(attrs.kind, PlotSeriesKind::Continuous);
            let prev_continuous = matches!(prev_series.kind, PlotSeriesKind::Continuous);

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
