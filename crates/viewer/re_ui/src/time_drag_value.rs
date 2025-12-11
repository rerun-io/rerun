use std::ops::RangeInclusive;

use egui::{NumExt as _, Response};
use re_log_types::{TimeInt, TimeType, TimestampFormat};

/// Drag value widget for editing time values for both sequence and temporal timelines.
///
/// Compute and store various information about the time range related to how the UI should behave.
#[derive(Debug)]
pub struct TimeDragValue {
    /// Allowed range for value.
    pub range: RangeInclusive<i64>,

    /// For ranges with large offsets (e.g. `log_time`), this is a rounded time just before the
    /// first logged data, which can be used as offset in the UI.
    base_time: Option<i64>,

    /// For temporal timelines, this is a nice unit factor to use.
    unit_factor: i64,

    /// For temporal timelines, this is the unit symbol to display.
    unit_symbol: &'static str,

    /// This is a nice range of absolute times to use when editing an absolute time. The boundaries
    /// are extended to the nearest rounded unit to minimize glitches.
    abs_range: RangeInclusive<i64>,

    /// This is a nice range of relative times to use when editing an absolute time. The boundaries
    /// are extended to the nearest rounded unit to minimize glitches.
    rel_range: RangeInclusive<i64>,
}

impl TimeDragValue {
    pub fn from_abs_time_range(times: re_log_types::AbsoluteTimeRange) -> Self {
        Self::from_time_range(times.min.as_i64()..=times.max.as_i64())
    }

    pub fn from_time_range(range: RangeInclusive<i64>) -> Self {
        let span = range.end() - range.start();
        let base_time = time_range_base_time(*range.start(), span);
        let (unit_symbol, unit_factor) = unit_from_span(span);

        // `abs_range` is used by the DragValue when editing an absolute time, its bound expanded to
        // the nearest unit to minimize glitches.
        let abs_range =
            round_down(*range.start(), unit_factor)..=round_up(*range.end(), unit_factor);

        // `rel_range` is used by the DragValue when editing a relative time offset. It must have
        // enough margins either side to accommodate for all possible values of current time.
        let rel_range = round_down(-span, unit_factor)..=round_up(2 * span, unit_factor);

        Self {
            range,
            base_time,
            unit_factor,
            unit_symbol,
            abs_range,
            rel_range,
        }
    }

    /// Return the minimum time set for this drag value.
    pub fn min_time(&self) -> TimeInt {
        TimeInt::new_temporal(*self.range.start())
    }

    /// Return the maximum time set for this drag value.
    pub fn max_time(&self) -> TimeInt {
        TimeInt::new_temporal(*self.range.end())
    }

    /// Show a drag value widget, taking into account the time type.
    pub fn drag_value_ui(
        &self,
        ui: &mut egui::Ui,
        time_type: TimeType,
        time: &mut TimeInt,
        absolute: bool,
        low_bound_override: Option<TimeInt>,
        timestamp_format: TimestampFormat,
    ) -> Response {
        match time_type {
            TimeType::Sequence => {
                self.sequence_drag_value_ui(ui, time, absolute, low_bound_override)
            }

            TimeType::DurationNs | TimeType::TimestampNs => {
                // TODO(abey79): distinguish the two types?
                self.temporal_drag_value_ui(
                    ui,
                    time,
                    absolute,
                    low_bound_override,
                    timestamp_format,
                )
                .0
            }
        }
    }

    /// Show a sequence drag value widget.
    pub fn sequence_drag_value_ui(
        &self,
        ui: &mut egui::Ui,
        value: &mut TimeInt,
        absolute: bool,
        low_bound_override: Option<TimeInt>,
    ) -> Response {
        let mut time_range = if absolute {
            self.abs_range.clone()
        } else {
            self.rel_range.clone()
        };

        // speed must be computed before messing with time_range for consistency
        let span = time_range.end() - time_range.start();
        let speed = (span as f32 * 0.005).at_least(1.0);

        if let Some(low_bound_override) = low_bound_override {
            time_range =
                low_bound_override.as_i64().at_least(*time_range.start())..=*time_range.end();
        }

        let mut value_i64 = value.as_i64();
        let response = ui.add(
            egui::DragValue::new(&mut value_i64)
                .clamp_existing_to_range(false)
                .range(time_range)
                .speed(speed),
        );
        *value = TimeInt::new_temporal(value_i64);

        response
    }

    /// Show a temporal drag value widget.
    ///
    /// Feature rich:
    /// - scale to the proper units
    /// - display the base time if any
    /// - etc.
    ///
    /// Returns a tuple of the [`egui::DragValue`]'s [`egui::Response`], and the base time label's
    /// [`egui::Response`], if any.
    pub fn temporal_drag_value_ui(
        &self,
        ui: &mut egui::Ui,
        value: &mut TimeInt,
        absolute: bool,
        low_bound_override: Option<TimeInt>,
        timestamp_format: TimestampFormat,
    ) -> (Response, Option<Response>) {
        let mut time_range = if absolute {
            self.abs_range.clone()
        } else {
            self.rel_range.clone()
        };

        let factor = self.unit_factor as f32;
        let offset = if absolute {
            self.base_time.unwrap_or(0)
        } else {
            0
        };

        // speed must be computed before messing with time_range for consistency
        let speed = (time_range.end() - time_range.start()) as f32 / factor * 0.005;

        if let Some(low_bound_override) = low_bound_override {
            time_range =
                low_bound_override.as_i64().at_least(*time_range.start())..=*time_range.end();
        }

        let mut time_unit = (value.as_i64().saturating_sub(offset)) as f32 / factor;

        let time_range = (*time_range.start() - offset) as f32 / factor
            ..=(*time_range.end() - offset) as f32 / factor;

        let base_time_response = if absolute {
            self.base_time.map(|base_time| {
                ui.label(format!(
                    "{} + ",
                    // TODO(abey79): is this the correct TimeType? https://github.com/rerun-io/rerun/pull/9292#discussion_r1998445676
                    TimeType::DurationNs.format(TimeInt::new_temporal(base_time), timestamp_format)
                ))
            })
        } else {
            None
        };

        let drag_value_response = ui.add(
            egui::DragValue::new(&mut time_unit)
                .clamp_existing_to_range(false)
                .range(time_range)
                .speed(speed)
                .suffix(self.unit_symbol),
        );

        *value = TimeInt::new_temporal((time_unit * factor).round() as i64 + offset);

        (drag_value_response, base_time_response)
    }
}

fn unit_from_span(span: i64) -> (&'static str, i64) {
    if span / 1_000_000_000 > 0 {
        ("s", 1_000_000_000)
    } else if span / 1_000_000 > 0 {
        ("ms", 1_000_000)
    } else if span / 1_000 > 0 {
        ("μs", 1_000)
    } else {
        ("ns", 1)
    }
}

/// Value of the start time over time span ratio above which an explicit offset is handled.
static SPAN_TO_START_TIME_OFFSET_THRESHOLD: i64 = 10;

fn time_range_base_time(min_time: i64, span: i64) -> Option<i64> {
    if min_time <= 0 {
        return None;
    }

    if span.saturating_mul(SPAN_TO_START_TIME_OFFSET_THRESHOLD) < min_time {
        let factor = if span / 1_000_000 > 0 {
            1_000_000_000
        } else if span / 1_000 > 0 {
            1_000_000
        } else {
            1_000
        };

        Some(min_time - (min_time % factor))
    } else {
        None
    }
}

fn round_down(value: i64, factor: i64) -> i64 {
    value - (value.rem_euclid(factor))
}

fn round_up(value: i64, factor: i64) -> i64 {
    let val = round_down(value, factor);

    if val == value { val } else { val + factor }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_round_down() {
        assert_eq!(round_down(2200, 1000), 2000);
        assert_eq!(round_down(2000, 1000), 2000);
        assert_eq!(round_down(-2200, 1000), -3000);
        assert_eq!(round_down(-3000, 1000), -3000);
        assert_eq!(round_down(0, 1000), 0);
    }

    #[test]
    fn test_round_up() {
        assert_eq!(round_up(2200, 1000), 3000);
        assert_eq!(round_up(2000, 1000), 2000);
        assert_eq!(round_up(-2200, 1000), -2000);
        assert_eq!(round_up(-3000, 1000), -3000);
        assert_eq!(round_up(0, 1000), 0);
    }
}
