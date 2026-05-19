//! Aggregation of [`PlotPoint`]s for time series visualization.
//!
//! Points are only aggregated within the same window if they share the same
//! visual attributes (color, series kind) and fall within the window's time span.
//!
//! All aggregators share the same non-finite handling contract:
//! Non-finite values (`NaN`, `+inf`, `-inf`) are **non-aggregatable**: they always form their own
//! single-point window and are emitted as-is. A non-finite value also breaks any adjacent
//! aggregation window.
//! Note that we could have alternatively treated non-finite values as `[crate::PlotSeriesKind::Clear]`,
//! but not only is the semantic of this different (it interrupts a line exactly _at_ the clear),
//! it also turns out a lot slower (as of writing) since it will cause emitting a lot of individual plot series (see `points_to_series`).
//!
//! The first and last output points are time-aligned to the input's time bounds
//! to prevent visual glitches at the edges.

use crate::{PlotPoint, PlotPointAttrs};

/// Implements aggregation behaviors for `Average`.
pub struct AverageAggregator;

impl AverageAggregator {
    /// `aggregation_factor`: the width of the aggregation window.
    ///
    /// Adjacent plot points may have the same `PlotPoint::time`,
    /// if data was logged multiple times on the same time stamp.
    #[inline]
    pub fn aggregate(aggregation_factor: f64, points: &[PlotPoint]) -> Vec<PlotPoint> {
        let min_time = points.first().map_or(i64::MIN, |p| p.time);
        let max_time = points.last().map_or(i64::MAX, |p| p.time);

        let mut aggregated =
            Vec::with_capacity((points.len() as f64 / aggregation_factor) as usize);

        // NOTE: `floor()` since we handle fractional tails separately.
        let window_size = usize::max(1, aggregation_factor.floor() as usize);
        let aggregation_factor_fract = aggregation_factor.fract();

        let mut i = 0;
        while i < points.len() {
            // Non-finite values are non-aggregatable — emit solo and move on.
            if !points[i].value.is_finite() {
                aggregated.push(points[i].clone());
                i += 1;
                continue;
            }

            // How many points to combine together this time.
            let mut j = 0;

            let mut ratio = 0.0;
            let mut acc = points[i + j].clone();
            acc.value = 0.0;
            acc.attrs.radius_ui = 0.0;

            while i + j < points.len()
                && points[i + j].value.is_finite()
                && are_aggregatable(&points[i], &points[i + j], window_size)
            {
                let point = &points[i + j];

                acc.value += point.value;
                acc.attrs.radius_ui += point.attrs.radius_ui;

                ratio += 1.0;
                j += 1;
            }

            // Do a weighted average for the fractional tail.
            if aggregation_factor_fract > 0.0
                && i + j < points.len()
                && points[i + j].value.is_finite()
                && are_aggregatable(&points[i], &points[i + j], window_size)
            {
                let point = &points[i + j];

                let w = aggregation_factor_fract;
                acc.value += point.value * w;
                acc.attrs.radius_ui += (point.attrs.radius_ui as f64 * w) as f32;

                ratio += aggregation_factor_fract;
                j += 1;
            }

            acc.value /= ratio;
            acc.attrs.radius_ui = (acc.attrs.radius_ui as f64 / ratio) as _;

            aggregated.push(acc);

            i += j;
        }

        // Force align the start and end timestamps to prevent jarring visual glitches.
        if let Some(p) = aggregated.first_mut() {
            p.time = min_time;
        }
        if let Some(p) = aggregated.last_mut() {
            p.time = max_time;
        }

        aggregated
    }
}

/// Implements aggregation behaviors for `Min`, `Max`, `MinMax`, and `MinMaxAverage`.
pub enum MinMaxAggregator {
    /// Keep only the maximum values in the range.
    Max,

    /// Keep only the minimum values in the range.
    Min,

    /// Keep both the minimum and maximum values in the range.
    ///
    /// This will yield two aggregated points instead of one, effectively creating a vertical line.
    MinMax,

    /// Find both the minimum and maximum values in the range, then use the average of those.
    MinMaxAverage,
}

impl MinMaxAggregator {
    /// Adjacent plot points may have the same `PlotPoint::time`,
    /// if data was logged multiple times on the same time stamp.
    #[inline]
    pub fn aggregate(&self, aggregation_window_size: f64, points: &[PlotPoint]) -> Vec<PlotPoint> {
        // NOTE: `round()` since this can only handle discrete window sizes.
        let window_size = usize::max(1, aggregation_window_size.round() as usize);

        let min_time = points.first().map_or(i64::MIN, |p| p.time);
        let max_time = points.last().map_or(i64::MAX, |p| p.time);

        let capacity = (points.len() as f64 / window_size as f64) as usize;
        let mut aggregated = match self {
            Self::MinMax => Vec::with_capacity(capacity * 2),
            _ => Vec::with_capacity(capacity),
        };

        let mut i = 0;
        while i < points.len() {
            // Non-finite values are non-aggregatable — emit solo and move on.
            if !points[i].value.is_finite() {
                aggregated.push(points[i].clone());
                i += 1;
                continue;
            }

            // How many points to combine together this time.
            let mut j = 0;

            let mut acc_min = points[i + j].clone();
            let mut acc_max = points[i + j].clone();
            j += 1;

            while i + j < points.len()
                && points[i + j].value.is_finite()
                && are_aggregatable(&points[i], &points[i + j], window_size)
            {
                let point = &points[i + j];

                match self {
                    Self::MinMax | Self::MinMaxAverage => {
                        acc_min.value = f64::min(acc_min.value, point.value);
                        acc_min.attrs.radius_ui =
                            f32::min(acc_min.attrs.radius_ui, point.attrs.radius_ui);
                        acc_max.value = f64::max(acc_max.value, point.value);
                        acc_max.attrs.radius_ui =
                            f32::max(acc_max.attrs.radius_ui, point.attrs.radius_ui);
                    }
                    Self::Min => {
                        acc_min.value = f64::min(acc_min.value, point.value);
                        acc_min.attrs.radius_ui =
                            f32::min(acc_min.attrs.radius_ui, point.attrs.radius_ui);
                    }
                    Self::Max => {
                        acc_max.value = f64::max(acc_max.value, point.value);
                        acc_max.attrs.radius_ui =
                            f32::max(acc_max.attrs.radius_ui, point.attrs.radius_ui);
                    }
                }

                j += 1;
            }

            match self {
                Self::MinMax => {
                    aggregated.push(acc_min);
                    // Avoid pushing the same point twice.
                    if j > 1 {
                        aggregated.push(acc_max);
                    }
                }
                Self::MinMaxAverage => {
                    // Don't average a single point with itself.
                    if j > 1 {
                        acc_min.value = (acc_min.value + acc_max.value) * 0.5;
                        acc_min.attrs.radius_ui =
                            (acc_min.attrs.radius_ui + acc_max.attrs.radius_ui) * 0.5;
                    }
                    aggregated.push(acc_min);
                }
                Self::Min => {
                    aggregated.push(acc_min);
                }
                Self::Max => {
                    aggregated.push(acc_max);
                }
            }

            i += j;
        }

        // Force align the start and end timestamps to prevent jarring visual glitches.
        if let Some(p) = aggregated.first_mut() {
            p.time = min_time;
        }
        if let Some(p) = aggregated.last_mut() {
            p.time = max_time;
        }

        aggregated
    }
}

/// Are two [`PlotPoint`]s safe to aggregate?
fn are_aggregatable(point1: &PlotPoint, point2: &PlotPoint, window_size: usize) -> bool {
    let PlotPoint {
        time,
        value: _,
        attrs,
    } = point1;
    let PlotPointAttrs {
        color,
        radius_ui: _,
        kind,
    } = attrs;

    // We cannot aggregate two points that don't live in the same aggregation window to start with.
    // This is very common with e.g. sparse datasets.
    time.abs_diff(point2.time) <= window_size as u64
        && *color == point2.attrs.color
        && *kind == point2.attrs.kind
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PlotSeriesKind;

    fn pt(time: i64, value: f64) -> PlotPoint {
        PlotPoint {
            time,
            value,
            attrs: PlotPointAttrs {
                color: egui::Color32::WHITE,
                radius_ui: 1.0,
                kind: PlotSeriesKind::Continuous,
            },
        }
    }

    fn values(points: &[PlotPoint]) -> Vec<f64> {
        points.iter().map(|p| p.value).collect()
    }

    /// All non-finite `f64` values we treat as non-aggregatable.
    const NON_FINITE_VALUES: &[(&str, f64)] = &[
        ("nan", f64::NAN),
        ("pos_inf", f64::INFINITY),
        ("neg_inf", f64::NEG_INFINITY),
    ];

    pub const ALL_MIN_MAX_AGGREGATORS: &[MinMaxAggregator] = &[
        MinMaxAggregator::Min,
        MinMaxAggregator::Max,
        MinMaxAggregator::MinMax,
        MinMaxAggregator::MinMaxAverage,
    ];

    /// Human-readable name, useful for test diagnostics.
    pub const fn min_max_aggreagtor_label(aggregator: &MinMaxAggregator) -> &'static str {
        match aggregator {
            MinMaxAggregator::Min => "min",
            MinMaxAggregator::Max => "max",
            MinMaxAggregator::MinMax => "minmax",
            MinMaxAggregator::MinMaxAverage => "minmax_average",
        }
    }

    /// Run all aggregator types on `points` with given window size.
    /// Returns `(label, result)` pairs covering Average + all [`MinMaxAggregator`] variants.
    fn aggregate_all(window: f64, points: &[PlotPoint]) -> Vec<(&'static str, Vec<PlotPoint>)> {
        let mut results = vec![("average", AverageAggregator::aggregate(window, points))];
        for variant in ALL_MIN_MAX_AGGREGATORS {
            results.push((
                min_max_aggreagtor_label(variant),
                variant.aggregate(window, points),
            ));
        }
        results
    }

    // =======================================================================
    // Shared properties — iterated over ALL aggregator types.
    // =======================================================================

    #[test]
    fn all_empty_input() {
        for (label, result) in aggregate_all(2.0, &[]) {
            assert!(result.is_empty(), "{label}: expected empty output");
        }
    }

    #[test]
    fn all_single_point_preserves_value() {
        let points = vec![pt(0, 42.0)];
        for (label, result) in aggregate_all(2.0, &points) {
            assert_eq!(result.len(), 1, "{label}: expected 1 output point");
            assert_eq!(result[0].value, 42.0, "{label}: value mismatch");
        }
    }

    #[test]
    fn all_non_finite_breaks_window() {
        // Non-finite at start should be emitted solo, remaining points aggregated separately.
        for &(nf_name, nf) in NON_FINITE_VALUES {
            let points = vec![pt(0, nf), pt(1, 5.0), pt(2, 3.0)];
            for (label, result) in aggregate_all(3.0, &points) {
                assert!(
                    result.len() >= 2,
                    "{label}/{nf_name}: non-finite should break into separate window"
                );
                assert!(
                    !result[0].value.is_finite(),
                    "{label}/{nf_name}: first output should be non-finite"
                );
                assert!(
                    result[1].value.is_finite(),
                    "{label}/{nf_name}: second output should be finite"
                );
            }
        }
    }

    #[test]
    fn all_each_non_finite_emitted_individually() {
        for &(nf_name, nf) in NON_FINITE_VALUES {
            let points = vec![pt(0, nf), pt(1, nf)];
            for (label, result) in aggregate_all(2.0, &points) {
                assert_eq!(
                    result.len(),
                    2,
                    "{label}/{nf_name}: each non-finite should be its own point"
                );
                assert!(
                    !result[0].value.is_finite(),
                    "{label}/{nf_name}: first should be non-finite"
                );
                assert!(
                    !result[1].value.is_finite(),
                    "{label}/{nf_name}: second should be non-finite"
                );
            }
        }
    }

    #[test]
    fn all_single_non_finite_emits_non_finite() {
        for &(nf_name, nf) in NON_FINITE_VALUES {
            let points = vec![pt(0, nf)];
            for (label, result) in aggregate_all(2.0, &points) {
                assert_eq!(
                    result.len(),
                    1,
                    "{label}/{nf_name}: expected 1 output point"
                );
                assert!(
                    !result[0].value.is_finite(),
                    "{label}/{nf_name}: expected non-finite output"
                );
            }
        }
    }

    #[test]
    fn all_preserve_time_bounds() {
        let points = vec![pt(100, 1.0), pt(101, 2.0), pt(102, 3.0), pt(103, 4.0)];
        for (label, result) in aggregate_all(2.0, &points) {
            assert_eq!(
                result.first().unwrap().time,
                100,
                "{label}: first time mismatch"
            );
            assert_eq!(
                result.last().unwrap().time,
                103,
                "{label}: last time mismatch"
            );
        }
    }

    #[test]
    fn all_non_finite_island_between_real_windows() {
        for &(nf_name, nf) in NON_FINITE_VALUES {
            let points = vec![
                pt(0, 1.0),
                pt(1, 2.0),
                // gap → new window
                pt(10, nf),
                pt(11, nf),
                // gap → new window
                pt(20, 3.0),
                pt(21, 4.0),
            ];
            for (label, result) in aggregate_all(2.0, &points) {
                let vals: Vec<bool> = result.iter().map(|p| !p.value.is_finite()).collect();

                // Structure: [finite…, non-finite, non-finite, finite…]
                let nf_positions: Vec<usize> = vals
                    .iter()
                    .enumerate()
                    .filter(|&(_, n)| *n)
                    .map(|(i, _)| i)
                    .collect();
                assert_eq!(
                    nf_positions.len(),
                    2,
                    "{label}/{nf_name}: expected exactly 2 solo non-finite points, got {vals:?}"
                );
                // Everything before first non-finite should be finite.
                assert!(
                    vals[..nf_positions[0]].iter().all(|&n| !n),
                    "{label}/{nf_name}: points before non-finite island should be finite"
                );
                // Everything after last non-finite should be finite.
                assert!(
                    vals[nf_positions[1] + 1..].iter().all(|&n| !n),
                    "{label}/{nf_name}: points after non-finite island should be finite"
                );
            }
        }
    }

    // =======================================================================
    // Average-specific tests
    // =======================================================================

    #[test]
    fn average_window_of_two() {
        let points = vec![pt(0, 4.0), pt(1, 6.0)];
        let result = AverageAggregator::aggregate(2.0, &points);
        assert_eq!(values(&result), vec![5.0]);
    }

    #[test]
    fn average_multiple_windows() {
        // window_size=2 → groups: [10, 20], [30, 40]
        let points = vec![pt(0, 10.0), pt(1, 20.0), pt(3, 30.0), pt(4, 40.0)];
        let result = AverageAggregator::aggregate(2.0, &points);
        assert_eq!(values(&result), vec![15.0, 35.0]);
    }

    #[test]
    fn average_non_finite_at_start_then_real_aggregated() {
        // [non-finite, 10.0, 20.0] → non-finite solo, then [10.0, 20.0] averaged.
        for &(nf_name, nf) in NON_FINITE_VALUES {
            let points = vec![pt(0, nf), pt(1, 10.0), pt(2, 20.0)];
            let result = AverageAggregator::aggregate(3.0, &points);
            assert_eq!(result.len(), 2, "{nf_name}");
            assert!(!result[0].value.is_finite(), "{nf_name}");
            assert_eq!(result[1].value, 15.0, "{nf_name}"); // (10+20)/2
        }
    }

    #[test]
    fn average_non_finite_window_then_real_window() {
        for &(nf_name, nf) in NON_FINITE_VALUES {
            let points = vec![pt(0, nf), pt(1, nf), pt(10, 6.0), pt(11, 8.0)];
            let result = AverageAggregator::aggregate(2.0, &points);
            assert_eq!(result.len(), 3, "{nf_name}"); // nf, nf, avg(6,8)
            assert!(!result[0].value.is_finite(), "{nf_name}");
            assert!(!result[1].value.is_finite(), "{nf_name}");
            assert_eq!(result[2].value, 7.0, "{nf_name}");
        }
    }

    // =======================================================================
    // MinMaxAggregator variant-specific tests
    // =======================================================================

    #[test]
    fn min_picks_minimum() {
        let points = vec![pt(0, 10.0), pt(1, 3.0), pt(2, 7.0)];
        let result = MinMaxAggregator::Min.aggregate(3.0, &points);
        assert_eq!(values(&result), vec![3.0]);
    }

    #[test]
    fn max_picks_maximum() {
        let points = vec![pt(0, 10.0), pt(1, 3.0), pt(2, 7.0)];
        let result = MinMaxAggregator::Max.aggregate(3.0, &points);
        assert_eq!(values(&result), vec![10.0]);
    }

    #[test]
    fn minmax_emits_two_points() {
        let points = vec![pt(0, 10.0), pt(1, 3.0), pt(2, 7.0)];
        let result = MinMaxAggregator::MinMax.aggregate(3.0, &points);
        assert_eq!(values(&result), vec![3.0, 10.0]);
    }

    #[test]
    fn minmax_single_point_no_duplicate() {
        let points = vec![pt(0, 5.0)];
        let result = MinMaxAggregator::MinMax.aggregate(3.0, &points);
        assert_eq!(values(&result), vec![5.0]);
    }

    #[test]
    fn minmax_average_averages_extremes() {
        let points = vec![pt(0, 10.0), pt(1, 2.0), pt(2, 7.0)];
        let result = MinMaxAggregator::MinMaxAverage.aggregate(3.0, &points);
        assert_eq!(values(&result), vec![6.0]); // (2+10)/2
    }
}
