use re_log_types::{EntityPath, TimeInt, TimeRange};
use re_viewer_context::{external::re_entity_db::TimeSeriesAggregator, ViewQuery};

use crate::{
    aggregation::{AverageAggregator, MinMaxAggregator},
    PlotPoint, PlotSeries, PlotSeriesKind,
};

pub fn determine_time_range(
    query: &ViewQuery<'_>,
    data_result: &re_viewer_context::DataResult,
    plot_bounds: Option<egui_plot::PlotBounds>,
) -> TimeRange {
    let visible_history = match query.timeline.typ() {
        re_log_types::TimeType::Time => data_result.accumulated_properties().visible_history.nanos,
        re_log_types::TimeType::Sequence => {
            data_result
                .accumulated_properties()
                .visible_history
                .sequences
        }
    };

    let mut time_range = if data_result.accumulated_properties().visible_history.enabled {
        visible_history.time_range(query.latest_at)
    } else {
        TimeRange::new(TimeInt::MIN, TimeInt::MAX)
    };

    // TODO(cmc): We would love to reduce the query to match the actual plot bounds, but because
    // the plot widget handles zoom after we provide it with data for the current frame,
    // this results in an extremely jarring frame delay.
    // Just try it out and you'll see what I mean.
    if false {
        if let Some(plot_bounds) = plot_bounds {
            time_range.min = TimeInt::max(
                time_range.min,
                (plot_bounds.range_x().start().floor() as i64).into(),
            );
            time_range.max = TimeInt::min(
                time_range.max,
                (plot_bounds.range_x().end().ceil() as i64).into(),
            );
        }
    }
    time_range
}

// We have a bunch of raw points, and now we need to group them into individual line segments.
// A line segment is a continuous run of points with identical attributes: each time
// we notice a change in attributes, we need a new line segment.
pub fn points_to_lines(
    data_result: &re_viewer_context::DataResult,
    plot_value_delta: f64,
    points: Vec<PlotPoint>,
    store: &re_data_store::DataStore,
    query: &ViewQuery<'_>,
    lines: &mut Vec<PlotSeries>,
) {
    re_tracing::profile_scope!("secondary", &data_result.entity_path.to_string());
    if points.is_empty() {
        return;
    }

    let aggregator = *data_result
        .accumulated_properties()
        .time_series_aggregator
        .get();
    let (aggregation_factor, points) = apply_aggregation(aggregator, plot_value_delta, points);
    let min_time = store
        .entity_min_time(&query.timeline, &data_result.entity_path)
        .map_or(points.first().map_or(0, |p| p.time), |time| time.as_i64());
    let same_label = |points: &[PlotPoint]| -> Option<String> {
        let label = points[0].attrs.label.as_ref()?;
        (points.iter().all(|p| p.attrs.label.as_ref() == Some(label))).then(|| label.clone())
    };
    let line_label = same_label(&points).unwrap_or_else(|| data_result.entity_path.to_string());
    if points.len() == 1 {
        // Can't draw a single point as a continuous line, so fall back on scatter
        let mut kind = points[0].attrs.kind;
        if kind == PlotSeriesKind::Continuous {
            kind = PlotSeriesKind::Scatter;
        }

        lines.push(PlotSeries {
            label: line_label,
            color: points[0].attrs.color,
            width: 2.0 * points[0].attrs.radius,
            kind,
            points: vec![(points[0].time, points[0].value)],
            entity_path: data_result.entity_path.clone(),
            aggregator,
            aggregation_factor,
            min_time,
        });
    } else {
        add_line_segments(
            &line_label,
            points,
            &data_result.entity_path,
            aggregator,
            aggregation_factor,
            min_time,
            lines,
        );
    }
}

/// Apply the given aggregation to the provided points.
pub fn apply_aggregation(
    aggregator: TimeSeriesAggregator,
    plot_value_delta: f64,
    points: Vec<PlotPoint>,
) -> (f64, Vec<PlotPoint>) {
    let aggregation_factor = plot_value_delta;
    let num_points_before = points.len() as f64;
    let points = if aggregation_factor > 2.0 {
        re_tracing::profile_scope!("aggregate", aggregator.to_string());

        #[allow(clippy::match_same_arms)] // readability
        match aggregator {
            TimeSeriesAggregator::Off => points,
            TimeSeriesAggregator::Average => {
                AverageAggregator::aggregate(aggregation_factor, &points)
            }
            TimeSeriesAggregator::Min => {
                MinMaxAggregator::Min.aggregate(aggregation_factor, &points)
            }
            TimeSeriesAggregator::Max => {
                MinMaxAggregator::Max.aggregate(aggregation_factor, &points)
            }
            TimeSeriesAggregator::MinMax => {
                MinMaxAggregator::MinMax.aggregate(aggregation_factor, &points)
            }
            TimeSeriesAggregator::MinMaxAverage => {
                MinMaxAggregator::MinMaxAverage.aggregate(aggregation_factor, &points)
            }
        }
    } else {
        points
    };
    let num_points_after = points.len() as f64;
    let actual_aggregation_factor = num_points_before / num_points_after;

    (actual_aggregation_factor, points)
}

#[inline(never)] // Better callstacks on crashes
fn add_line_segments(
    line_label: &str,
    points: Vec<PlotPoint>,
    entity_path: &EntityPath,
    aggregator: TimeSeriesAggregator,
    aggregation_factor: f64,
    min_time: i64,
    lines: &mut Vec<PlotSeries>,
) {
    re_tracing::profile_function!();

    let num_points = points.len();
    let mut attrs = points[0].attrs.clone();
    let mut line: PlotSeries = PlotSeries {
        label: line_label.to_owned(),
        color: attrs.color,
        width: 2.0 * attrs.radius,
        points: Vec::with_capacity(num_points),
        kind: attrs.kind,
        entity_path: entity_path.clone(),
        aggregator,
        aggregation_factor,
        min_time,
    };

    for (i, p) in points.into_iter().enumerate() {
        if p.attrs == attrs {
            // Same attributes, just add to the current line segment.

            line.points.push((p.time, p.value));
        } else {
            // Attributes changed since last point, break up the current run into a
            // line segment, and start the next one.

            attrs = p.attrs;
            let prev_line = std::mem::replace(
                &mut line,
                PlotSeries {
                    label: line_label.to_owned(),
                    color: attrs.color,
                    width: 2.0 * attrs.radius,
                    kind: attrs.kind,
                    points: Vec::with_capacity(num_points - i),
                    entity_path: entity_path.clone(),
                    aggregator,
                    aggregation_factor,
                    min_time,
                },
            );

            let cur_continuous = matches!(attrs.kind, PlotSeriesKind::Continuous);
            let prev_continuous = matches!(prev_line.kind, PlotSeriesKind::Continuous);

            let prev_point = *prev_line.points.last().unwrap();
            lines.push(prev_line);

            // If the previous point was continuous and the current point is continuous
            // too, then we want the 2 segments to appear continuous even though they
            // are actually split from a data standpoint.
            if cur_continuous && prev_continuous {
                line.points.push(prev_point);
            }

            // Add the point that triggered the split to the new segment.
            line.points.push((p.time, p.value));
        }
    }

    if !line.points.is_empty() {
        lines.push(line);
    }
}
