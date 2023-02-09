use std::ops::RangeInclusive;

use egui::{lerp, remap, NumExt};

use re_log_types::{TimeInt, TimeRange, TimeRangeF, TimeReal};

use crate::{misc::time_control::PlayState, TimeView, ViewerContext};

/// The ideal gap between time segments.
///
/// This is later shrunk via [`GAP_EXPANSION_FRACTION`].
const MAX_GAP: f32 = 40.0;

/// How much of the gap use up to expand segments visually to either side?
const GAP_EXPANSION_FRACTION: f32 = 1.0 / 4.0;

/// Sze of the gap between time segments.
pub fn gap_width(x_range: &RangeInclusive<f32>, segments: &[TimeRange]) -> f32 {
    let num_gaps = segments.len().saturating_sub(1);
    if num_gaps == 0 {
        // gap width doesn't matter when there are no gaps
        MAX_GAP
    } else {
        // shrink gaps if there are a lot of them
        let width = *x_range.end() - *x_range.start();
        (width / (num_gaps as f32)).at_most(MAX_GAP)
    }
}

#[derive(Debug)]
pub struct Segment {
    /// Matches [`Self::time`] (linear transform).
    pub x: RangeInclusive<f32>,

    /// Matches [`Self::x`] (linear transform).
    pub time: TimeRangeF,

    /// does NOT match any of the above. Instead this is a tight bound.
    pub tight_time: TimeRange,
}

/// Represents a compressed view of time.
/// It does so by breaking up the timeline in linear segments.
///
/// Recreated each frame.
#[derive(Debug)]
pub struct TimeRangesUi {
    /// The total x-range we are viewing
    x_range: RangeInclusive<f32>,

    time_view: TimeView,

    /// x ranges matched to time ranges
    pub segments: Vec<Segment>,

    /// x distance per time unit
    points_per_time: f32,
}

impl Default for TimeRangesUi {
    /// Safe, meaningless default
    fn default() -> Self {
        Self {
            x_range: 0.0..=1.0,
            time_view: TimeView {
                min: TimeReal::from(0),
                time_spanned: 1.0,
            },
            segments: vec![],
            points_per_time: 1.0,
        }
    }
}

impl TimeRangesUi {
    pub fn new(x_range: RangeInclusive<f32>, time_view: TimeView, segments: &[TimeRange]) -> Self {
        crate::profile_function!();

        //        <------- time_view ------>
        //        <-------- x_range ------->
        //        |                        |
        //    [segment] [long segment]
        //             ^ gap

        let gap_width = gap_width(&x_range, segments);
        let width = *x_range.end() - *x_range.start();
        let points_per_time = width / time_view.time_spanned as f32;
        let points_per_time = if points_per_time > 0.0 && points_per_time.is_finite() {
            points_per_time
        } else {
            1.0
        };

        let mut left = 0.0; // we will translate things left/right later
        let ranges = segments
            .iter()
            .map(|range| {
                let range_width = range.length().as_f32() * points_per_time;
                let right = left + range_width;
                let x_range = left..=right;
                left = right + gap_width;

                let tight_time = *range;

                // expand each span outwards a bit to make selection of outer data points easier.
                // Also gives zero-width segments some width!
                let expansion = GAP_EXPANSION_FRACTION * gap_width;
                let x_range = (*x_range.start() - expansion)..=(*x_range.end() + expansion);

                let range = if range.min == range.max {
                    TimeRangeF::from(*range) // don't expand zero-width segments (e.g. `TimeInt::BEGINNING`).
                } else {
                    let time_expansion = TimeReal::from(expansion / points_per_time);
                    TimeRangeF::new(range.min - time_expansion, range.max + time_expansion)
                };

                Segment {
                    x: x_range,
                    time: range,
                    tight_time,
                }
            })
            .collect();

        let mut slf = Self {
            x_range: x_range.clone(),
            time_view,
            segments: ranges,
            points_per_time,
        };

        if let Some(time_start_x) = slf.x_from_time(time_view.min) {
            // Now move things left/right to align `x_range` and `view_range`:
            let x_translate = *x_range.start() - time_start_x;
            for segment in &mut slf.segments {
                segment.x = (*segment.x.start() + x_translate)..=(*segment.x.end() + x_translate);
            }
        }

        slf
    }

    /// Clamp the time to the valid ranges.
    ///
    /// Used when user is dragging the time handle.
    pub fn clamp_time(&self, mut time: TimeReal) -> TimeReal {
        if let (Some(first), Some(last)) = (self.segments.first(), self.segments.last()) {
            time = time.clamp(
                TimeReal::from(first.tight_time.min),
                TimeReal::from(last.tight_time.max),
            );

            // Special: don't allow users dragging time between
            // BEGINNING (-∞ = timeless data) and some real time.
            // Otherwise we get weird times (e.g. dates in 1923).
            // Selecting times between other segments is not as problematic, as all other segments are
            // real times, so interpolating between them always produces valid times.
            // By disallowing times between BEGINNING and the first real segment,
            // we also disallow users dragging the time to be between -∞ and the
            // real beginning of their data. That further highlights the specialness of -∞.
            // Furthermore, we want users to have a smooth experience dragging the time handle anywhere else.
            if first.tight_time == TimeRange::point(TimeInt::BEGINNING) {
                if let Some(second) = self.segments.get(1) {
                    if TimeInt::BEGINNING < time && time < second.tight_time.min {
                        time = TimeReal::from(second.tight_time.min);
                    }
                }
            }
        }
        time
    }

    /// Make sure the time is not between segments.
    ///
    /// This is so that the playback doesn't get stuck between segments.
    fn snap_time_to_segments(&self, value: TimeReal) -> TimeReal {
        for segment in &self.segments {
            if value < segment.time.min {
                return segment.time.min;
            } else if value <= segment.time.max {
                return value;
            }
        }
        value
    }

    // Make sure playback time doesn't get stuck between non-continuos regions:
    pub fn snap_time_control(&self, ctx: &mut ViewerContext<'_>) {
        if ctx.rec_cfg.time_ctrl.play_state() != PlayState::Playing {
            return;
        }

        // Make sure time doesn't get stuck between non-continuos regions:
        if let Some(time) = ctx.rec_cfg.time_ctrl.time() {
            let time = self.snap_time_to_segments(time);
            ctx.rec_cfg.time_ctrl.set_time(time);
        } else if let Some(selection) = ctx.rec_cfg.time_ctrl.loop_selection() {
            let snapped_min = self.snap_time_to_segments(selection.min);
            let snapped_max = self.snap_time_to_segments(selection.max);

            let min_was_good = selection.min == snapped_min;
            let max_was_good = selection.max == snapped_max;

            if min_was_good || max_was_good {
                return;
            }

            // Keeping max works better when looping
            ctx.rec_cfg.time_ctrl.set_loop_selection(TimeRangeF::new(
                snapped_max - selection.length(),
                snapped_max,
            ));
        }
    }

    pub fn x_from_time(&self, needle_time: TimeReal) -> Option<f32> {
        let first_segment = self.segments.first()?;
        let mut last_x = *first_segment.x.start();
        let mut last_time = first_segment.time.min;

        if needle_time < last_time {
            // extrapolate:
            return Some(last_x - self.points_per_time * (last_time - needle_time).as_f32());
        }

        for segment in &self.segments {
            if needle_time < segment.time.min {
                let t = TimeRangeF::new(last_time, segment.time.min).inverse_lerp(needle_time);
                return Some(lerp(last_x..=*segment.x.start(), t));
            } else if needle_time <= segment.time.max {
                let t = segment.time.inverse_lerp(needle_time);
                return Some(lerp(segment.x.clone(), t));
            } else {
                last_x = *segment.x.end();
                last_time = segment.time.max;
            }
        }

        // extrapolate:
        Some(last_x + self.points_per_time * (needle_time - last_time).as_f32())
    }

    pub fn time_from_x(&self, needle_x: f32) -> Option<TimeReal> {
        let first_segment = self.segments.first()?;
        let mut last_x = *first_segment.x.start();
        let mut last_time = first_segment.time.min;

        if needle_x < last_x {
            // extrapolate:
            return Some(last_time + TimeReal::from((needle_x - last_x) / self.points_per_time));
        }

        for segment in &self.segments {
            if needle_x < *segment.x.start() {
                let t = remap(needle_x, last_x..=*segment.x.start(), 0.0..=1.0);
                return Some(TimeRangeF::new(last_time, segment.time.min).lerp(t));
            } else if needle_x <= *segment.x.end() {
                let t = remap(needle_x, segment.x.clone(), 0.0..=1.0);
                return Some(segment.time.lerp(t));
            } else {
                last_x = *segment.x.end();
                last_time = segment.time.max;
            }
        }

        // extrapolate:
        Some(last_time + TimeReal::from((needle_x - last_x) / self.points_per_time))
    }

    /// Pan the view, returning the new view.
    pub fn pan(&self, delta_x: f32) -> Option<TimeView> {
        Some(TimeView {
            min: self.time_from_x(*self.x_range.start() + delta_x)?,
            time_spanned: self.time_view.time_spanned,
        })
    }

    /// Zoom the view around the given x, returning the new view.
    pub fn zoom_at(&self, x: f32, zoom_factor: f32) -> Option<TimeView> {
        let mut min_x = *self.x_range.start();
        let max_x = *self.x_range.end();
        let t = remap(x, min_x..=max_x, 0.0..=1.0);

        let width = max_x - min_x;

        let new_width = width / zoom_factor;
        let width_delta = new_width - width;

        min_x -= t * width_delta;

        Some(TimeView {
            min: self.time_from_x(min_x)?,
            time_spanned: self.time_view.time_spanned / zoom_factor as f64,
        })
    }

    /// How many egui points for each time unit?
    pub fn points_per_time(&self) -> Option<f32> {
        for segment in &self.segments {
            let dx = *segment.x.end() - *segment.x.start();
            let dt = segment.time.length().as_f32();
            if dx > 0.0 && dt > 0.0 {
                return Some(dx / dt);
            }
        }
        None
    }
}

// ----------------------------------------------------------------------------

#[test]
fn test_time_ranges_ui() {
    let time_range_ui = TimeRangesUi::new(
        100.0..=1000.0,
        TimeView {
            min: TimeReal::from(0.5),
            time_spanned: 14.2,
        },
        &[
            TimeRange::new(TimeInt::from(0), TimeInt::from(0)),
            TimeRange::new(TimeInt::from(1), TimeInt::from(5)),
            TimeRange::new(TimeInt::from(10), TimeInt::from(100)),
        ],
    );

    let pixel_precision = 0.5;

    // Sanity check round-tripping:
    for segment in &time_range_ui.segments {
        assert_eq!(
            time_range_ui.time_from_x(*segment.x.start()).unwrap(),
            segment.time.min
        );
        assert_eq!(
            time_range_ui.time_from_x(*segment.x.end()).unwrap(),
            segment.time.max
        );

        if segment.time.is_empty() {
            let x = time_range_ui.x_from_time(segment.time.min).unwrap();
            let mid_x = lerp(segment.x.clone(), 0.5);
            assert!((mid_x - x).abs() < pixel_precision);
        } else {
            let min_x = time_range_ui.x_from_time(segment.time.min).unwrap();
            assert!((min_x - *segment.x.start()).abs() < pixel_precision);

            let max_x = time_range_ui.x_from_time(segment.time.max).unwrap();
            assert!((max_x - *segment.x.end()).abs() < pixel_precision);
        }
    }
}

#[test]
fn test_time_ranges_ui_2() {
    let time_range_ui = TimeRangesUi::new(
        0.0..=500.0,
        TimeView {
            min: TimeReal::from(0),
            time_spanned: 50.0,
        },
        &[
            TimeRange::new(TimeInt::from(10), TimeInt::from(20)),
            TimeRange::new(TimeInt::from(30), TimeInt::from(40)),
        ],
    );

    let pixel_precision = 0.5;

    for x_in in 0..=500 {
        let x_in = x_in as f32;
        let time = time_range_ui.time_from_x(x_in).unwrap();
        let x_out = time_range_ui.x_from_time(time).unwrap();

        assert!(
            (x_in - x_out).abs() < pixel_precision,
            "x_in: {x_in}, x_out: {x_out}, time: {time:?}, time_range_ui: {time_range_ui:#?}"
        );
    }

    for time_in in 0..=50 {
        let time_in = TimeReal::from(time_in as f64);
        let x = time_range_ui.x_from_time(time_in).unwrap();
        let time_out = time_range_ui.time_from_x(x).unwrap();

        assert!(
            (time_in - time_out).abs().as_f64() < 0.1,
            "time_in: {time_in:?}, time_out: {time_out:?}, x: {x}, time_range_ui: {time_range_ui:#?}"
        );
    }
}
