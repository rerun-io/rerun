use re_log_types::{EntityPath, TimeInt, TimeRange};
use re_types::datatypes::Utf8;
use re_viewer_context::{external::re_entity_db::TimeSeriesAggregator, ViewQuery, ViewerContext};

use crate::{
    aggregation::{AverageAggregator, MinMaxAggregator},
    PlotPoint, PlotSeries, PlotSeriesKind, ScatterAttrs,
};

/// Find the plot bounds and the per-ui-point delta from egui.
pub fn determine_plot_bounds_and_time_per_pixel(
    ctx: &ViewerContext<'_>,
    query: &ViewQuery<'_>,
) -> (Option<egui_plot::PlotBounds>, f64) {
    let egui_ctx = &ctx.re_ui.egui_ctx;

    let plot_mem = egui_plot::PlotMemory::load(egui_ctx, crate::plot_id(query.space_view_id));
    let plot_bounds = plot_mem.as_ref().map(|mem| *mem.bounds());

    // How many ui points per time unit?
    let points_per_time = plot_mem
        .as_ref()
        .map_or(1.0, |mem| mem.transform().dpos_dvalue_x());
    let pixels_per_time = egui_ctx.pixels_per_point() as f64 * points_per_time;
    // How many time units per physical pixel?
    let time_per_pixel = 1.0 / pixels_per_time.max(f64::EPSILON);
    (plot_bounds, time_per_pixel)
}

pub fn determine_time_range(
    query: &ViewQuery<'_>,
    data_result: &re_viewer_context::DataResult,
    plot_bounds: Option<egui_plot::PlotBounds>,
    enable_query_clamping: bool,
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
    if enable_query_clamping {
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

// We have a bunch of raw points, and now we need to group them into individual series.
// A series is a continuous run of points with identical attributes: each time
// we notice a change in attributes, we need a new series.
pub fn points_to_series(
    data_result: &re_viewer_context::DataResult,
    time_per_pixel: f64,
    points: Vec<PlotPoint>,
    store: &re_data_store::DataStore,
    query: &ViewQuery<'_>,
    series_name: Option<Utf8>,
    all_series: &mut Vec<PlotSeries>,
) {
    re_tracing::profile_scope!("secondary", &data_result.entity_path.to_string());
    if points.is_empty() {
        return;
    }

    let aggregator = *data_result
        .accumulated_properties()
        .time_series_aggregator
        .get();
    let (aggregation_factor, points) = apply_aggregation(aggregator, time_per_pixel, points, query);
    let min_time = store
        .entity_min_time(&query.timeline, &data_result.entity_path)
        .map_or(points.first().map_or(0, |p| p.time), |time| time.as_i64());

    let series_label = series_name.unwrap_or_else(|| {
        let same_label = |points: &[PlotPoint]| -> Option<Utf8> {
            let label = points[0].attrs.label.as_ref()?;
            (points.iter().all(|p| p.attrs.label.as_ref() == Some(label))).then(|| label.clone())
        };
        same_label(&points).unwrap_or_else(|| data_result.entity_path.to_string().into())
    });
    if points.len() == 1 {
        // Can't draw a single point as a continuous line, so fall back on scatter
        let mut kind = points[0].attrs.kind;
        if kind == PlotSeriesKind::Continuous {
            kind = PlotSeriesKind::Scatter(ScatterAttrs::default());
        }

        all_series.push(PlotSeries {
            label: series_label,
            color: points[0].attrs.color,
            width: 2.0 * points[0].attrs.marker_size,
            kind,
            points: vec![(points[0].time, points[0].value)],
            entity_path: data_result.entity_path.clone(),
            aggregator,
            aggregation_factor,
            min_time,
        });
    } else {
        add_series_runs(
            &series_label,
            points,
            &data_result.entity_path,
            aggregator,
            aggregation_factor,
            min_time,
            all_series,
        );
    }
}

/// Apply the given aggregation to the provided points.
pub fn apply_aggregation(
    aggregator: TimeSeriesAggregator,
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

    let should_aggregate = aggregator != TimeSeriesAggregator::Off
        && (2.0 <= aggregation_duration || multiple_values_per_time_stamp());

    let points = if should_aggregate {
        re_tracing::profile_scope!("aggregate", aggregator.to_string());

        #[allow(clippy::match_same_arms)] // readability
        match aggregator {
            TimeSeriesAggregator::Off => points,
            TimeSeriesAggregator::Average => {
                AverageAggregator::aggregate(aggregation_duration, &points)
            }
            TimeSeriesAggregator::Min => {
                MinMaxAggregator::Min.aggregate(aggregation_duration, &points)
            }
            TimeSeriesAggregator::Max => {
                MinMaxAggregator::Max.aggregate(aggregation_duration, &points)
            }
            TimeSeriesAggregator::MinMax => {
                MinMaxAggregator::MinMax.aggregate(aggregation_duration, &points)
            }
            TimeSeriesAggregator::MinMaxAverage => {
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

#[inline(never)] // Better callstacks on crashes
fn add_series_runs(
    series_label: &Utf8,
    points: Vec<PlotPoint>,
    entity_path: &EntityPath,
    aggregator: TimeSeriesAggregator,
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
        width: 2.0 * attrs.marker_size,
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
                    width: 2.0 * attrs.marker_size,
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

/// Returns the least number greater than `self`.
///
/// Unstable feature in Rust. This is a copy of the implementation from the standard library.
///
/// Let `TINY` be the smallest representable positive `f64`. Then,
///  - if `self.is_nan()`, this returns `self`;
///  - if `self` is [`NEG_INFINITY`], this returns [`MIN`];
///  - if `self` is `-TINY`, this returns -0.0;
///  - if `self` is -0.0 or +0.0, this returns `TINY`;
///  - if `self` is [`MAX`] or [`INFINITY`], this returns [`INFINITY`];
///  - otherwise the unique least value greater than `self` is returned.
///
/// The identity `x.next_up() == -(-x).next_down()` holds for all non-NaN `x`. When `x`
/// is finite `x == x.next_up().next_down()` also holds.
///
/// ```ignore
/// #![feature(float_next_up_down)]
/// // f64::EPSILON is the difference between 1.0 and the next number up.
/// assert_eq!(1.0f64.next_up(), 1.0 + f64::EPSILON);
/// // But not for most numbers.
/// assert!(0.1f64.next_up() < 0.1 + f64::EPSILON);
/// assert_eq!(9007199254740992f64.next_up(), 9007199254740994.0);
/// ```
///
/// [`NEG_INFINITY`]: f64::NEG_INFINITY
/// [`INFINITY`]: f64::INFINITY
/// [`MIN`]: f64::MIN
/// [`MAX`]: f64::MAX
pub fn next_up_f64(this: f64) -> f64 {
    // We must use strictly integer arithmetic to prevent denormals from
    // flushing to zero after an arithmetic operation on some platforms.
    const TINY_BITS: u64 = 0x1; // Smallest positive f64.
    const CLEAR_SIGN_MASK: u64 = 0x7fff_ffff_ffff_ffff;

    let bits = this.to_bits();
    if this.is_nan() || bits == f64::INFINITY.to_bits() {
        return this;
    }

    let abs = bits & CLEAR_SIGN_MASK;
    let next_bits = if abs == 0 {
        TINY_BITS
    } else if bits == abs {
        bits + 1
    } else {
        bits - 1
    };
    f64::from_bits(next_bits)
}
