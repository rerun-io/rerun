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
            // How many points to combine together this time.
            let mut j = 0;

            let mut ratio = 1.0;
            let mut acc = points[i + j].clone();
            j += 1;

            while i + j < points.len() && are_aggregatable(&points[i], &points[i + j], window_size)
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
            // How many points to combine together this time.
            let mut j = 0;

            let mut acc_min = points[i + j].clone();
            let mut acc_max = points[i + j].clone();
            j += 1;

            while i + j < points.len() && are_aggregatable(&points[i], &points[i + j], window_size)
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
