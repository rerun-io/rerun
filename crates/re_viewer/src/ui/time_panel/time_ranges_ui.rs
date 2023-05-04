//! egui uses `f32` precision for all screen-space ui coordinates,
//! which makes sense, because that is plenty of precision for things on screen.
//!
//! In this file we need to use `f64` precision because when zoomed in, the screen-space
//! time-ranges of are way outside the screen, leading to precision issues with `f32`.

use std::ops::RangeInclusive;

use egui::{lerp, remap, NumExt};
use itertools::Itertools as _;

use re_log_types::{TimeInt, TimeRange, TimeRangeF, TimeReal};
use re_viewer_context::{PlayState, TimeView, ViewerContext};

/// The ideal gap between time segments.
///
/// This is later shrunk via [`GAP_EXPANSION_FRACTION`].
const MAX_GAP: f64 = 40.0;

/// How much of the gap use up to expand segments visually to either side?
///
/// Should be strictly less than half, or we will get overlapping segments.
const GAP_EXPANSION_FRACTION: f64 = 1.0 / 4.0;

/// Sze of the gap between time segments.
pub fn gap_width(x_range: &RangeInclusive<f32>, segments: &[TimeRange]) -> f64 {
    let num_gaps = segments.len().saturating_sub(1);
    if num_gaps == 0 {
        // gap width doesn't matter when there are no gaps
        MAX_GAP
    } else {
        // shrink gaps if there are a lot of them
        let width = *x_range.end() - *x_range.start();
        (width as f64 / (num_gaps as f64)).at_most(MAX_GAP)
    }
}

#[derive(Debug)]
pub struct Segment {
    /// The range on the x-axis in the ui, in screen coordinates.
    ///
    /// Matches [`Self::time`] (linear transform).
    ///
    /// Uses `f64` because the ends of this range can be way outside the screen
    /// when we are very zoomed in.
    pub x: RangeInclusive<f64>,

    /// Matches [`Self::x`] (linear transform).
    pub time: TimeRangeF,

    /// Does NOT match any of the above. Instead this is a tight bound.
    pub tight_time: TimeRange,
}

/// Represents a compressed view of time.
///
/// It does so by breaking up the timeline in linear [`Segment`]s.
///
/// Recreated each frame.
#[derive(Debug)]
pub struct TimeRangesUi {
    /// The total UI x-range we are viewing.
    x_range: RangeInclusive<f64>,

    /// The range of time we are viewing.
    time_view: TimeView,

    /// The linear segments.
    ///
    /// Before the first and after the last we extrapolate.
    /// Between the segments we interpolate.
    pub segments: Vec<Segment>,

    /// x distance per time unit inside the segments,
    /// and before/after the last segment.
    /// Between segments time moves faster.
    pub points_per_time: f64,
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
    pub fn new(
        x_range: RangeInclusive<f32>,
        time_view: TimeView,
        time_ranges: &[TimeRange],
    ) -> Self {
        crate::profile_function!();

        debug_assert!(x_range.start() < x_range.end());

        //        <------- time_view ------>
        //        <-------- x_range ------->
        //        |                        |
        //    [segment] [long segment]
        //             ^ gap

        let gap_width_in_ui = gap_width(&x_range, time_ranges);
        let x_range = (*x_range.start() as f64)..=(*x_range.end() as f64);
        let width_in_ui = *x_range.end() - *x_range.start();
        let points_per_time = width_in_ui / time_view.time_spanned;
        let points_per_time = if points_per_time > 0.0 && points_per_time.is_finite() {
            points_per_time
        } else {
            1.0
        };

        // We expand each segment slightly, shrinking the gaps.
        // This is so that when a user drags the time to the start or end of a segment,
        // and they overshoot, they don't immediately go into the non-linear realm between segments.
        // When we expand we must take care not to expand so much that the gaps cover _negative_ time!
        let shortest_time_gap =
            time_ranges
                .iter()
                .tuple_windows()
                .fold(f64::INFINITY, |shortest, (a, b)| {
                    debug_assert!(a.max < b.min, "Overlapping time ranges: {a:?}, {b:?}");
                    let time_gap = b.min - a.max;
                    time_gap.as_f64().min(shortest)
                });

        let expansion_in_time = TimeReal::from(
            (GAP_EXPANSION_FRACTION * gap_width_in_ui / points_per_time)
                .at_most(shortest_time_gap * GAP_EXPANSION_FRACTION),
        );
        let expansion_in_ui = points_per_time * expansion_in_time.as_f64();

        let mut left = 0.0; // we will translate things left/right later to align x_range with time_view
        let segments = time_ranges
            .iter()
            .map(|&tight_time_range| {
                let range_width = tight_time_range.abs_length() as f64 * points_per_time;
                let right = left + range_width;
                let x_range = left..=right;
                left = right + gap_width_in_ui;

                // expand each span outwards a bit to make selection of outer data points easier.
                // Also gives zero-width segments some width!
                let x_range =
                    (*x_range.start() - expansion_in_ui)..=(*x_range.end() + expansion_in_ui);

                let time_range = TimeRangeF::new(
                    tight_time_range.min - expansion_in_time,
                    tight_time_range.max + expansion_in_time,
                );

                Segment {
                    x: x_range,
                    time: time_range,
                    tight_time: tight_time_range,
                }
            })
            .collect();

        let mut slf = Self {
            x_range: x_range.clone(),
            time_view,
            segments,
            points_per_time,
        };

        if let Some(time_start_x) = slf.x_from_time(time_view.min) {
            // Now move things left/right to align `x_range` and `time_view`:
            let x_translate = *x_range.start() - time_start_x;
            for segment in &mut slf.segments {
                segment.x = (*segment.x.start() + x_translate)..=(*segment.x.end() + x_translate);
            }
        }

        #[cfg(debug_assertions)]
        for (a, b) in slf.segments.iter().tuple_windows() {
            debug_assert!(
                a.x.end() < b.x.start(),
                "Overlapping x in segments: {a:#?}, {b:#?}"
            );
            debug_assert!(
                a.tight_time.max < b.tight_time.min,
                "Overlapping time in segments: {a:#?}, {b:#?}"
            );
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
            // real times, so interpolating between them always produces valid times
            // (we want users to have a smooth experience dragging the time handle anywhere else).
            // By disallowing times between BEGINNING and the first real segment,
            // we also disallow users dragging the time to be between -∞ and the
            // real beginning of their data. That further highlights the specialness of -∞.
            if first.tight_time.contains(TimeInt::BEGINNING) {
                if let Some(second) = self.segments.get(1) {
                    let half_way =
                        TimeRangeF::new(TimeInt::BEGINNING, second.tight_time.min).lerp(0.5);

                    if time < half_way {
                        time = TimeReal::from(TimeInt::BEGINNING);
                    } else if time < second.tight_time.min {
                        time = second.tight_time.min.into();
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

    // Make sure playback time doesn't get stuck between non-continuous regions:
    pub fn snap_time_control(&self, ctx: &mut ViewerContext<'_>) {
        if ctx.rec_cfg.time_ctrl.play_state() != PlayState::Playing {
            return;
        }

        // Make sure time doesn't get stuck between non-continuous regions:
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

    pub fn x_from_time_f32(&self, needle_time: TimeReal) -> Option<f32> {
        self.x_from_time(needle_time).map(|x| x as f32)
    }

    pub fn x_from_time(&self, needle_time: TimeReal) -> Option<f64> {
        let first_segment = self.segments.first()?;
        let mut last_x = *first_segment.x.start();
        let mut last_time = first_segment.time.min;

        if needle_time < last_time {
            // extrapolate:
            return Some(last_x - self.points_per_time * (last_time - needle_time).as_f64());
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
        Some(last_x + self.points_per_time * (needle_time - last_time).as_f64())
    }

    pub fn time_from_x_f32(&self, needle_x: f32) -> Option<TimeReal> {
        self.time_from_x_f64(needle_x as f64)
    }

    pub fn time_from_x_f64(&self, needle_x: f64) -> Option<TimeReal> {
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

    pub fn time_range_from_x_range(&self, x_range: RangeInclusive<f32>) -> TimeRange {
        let (min_x, max_x) = (*x_range.start(), *x_range.end());
        TimeRange {
            min: self
                .time_from_x_f32(min_x)
                .map_or(TimeInt::MIN, |tf| tf.floor()),

            max: self
                .time_from_x_f32(max_x)
                .map_or(TimeInt::MAX, |tf| tf.ceil()),
        }
    }

    /// Pan the view, returning the new view.
    pub fn pan(&self, delta_x: f32) -> Option<TimeView> {
        Some(TimeView {
            min: self.time_from_x_f64(*self.x_range.start() + delta_x as f64)?,
            time_spanned: self.time_view.time_spanned,
        })
    }

    /// Zoom the view around the given x, returning the new view.
    pub fn zoom_at(&self, x: f32, zoom_factor: f32) -> Option<TimeView> {
        let x = x as f64;
        let zoom_factor = zoom_factor as f64;

        let mut min_x = *self.x_range.start();
        let max_x = *self.x_range.end();
        let t = remap(x, min_x..=max_x, 0.0..=1.0);

        let width = max_x - min_x;

        let new_width = width / zoom_factor;
        let width_delta = new_width - width;

        min_x -= t * width_delta;

        Some(TimeView {
            min: self.time_from_x_f64(min_x)?,
            time_spanned: self.time_view.time_spanned / zoom_factor,
        })
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
            time_range_ui.time_from_x_f64(*segment.x.start()).unwrap(),
            segment.time.min
        );
        assert_eq!(
            time_range_ui.time_from_x_f64(*segment.x.end()).unwrap(),
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
        let x_in = x_in as f64;
        let time = time_range_ui.time_from_x_f64(x_in).unwrap();
        let x_out = time_range_ui.x_from_time(time).unwrap();

        assert!(
            (x_in - x_out).abs() < pixel_precision,
            "x_in: {x_in}, x_out: {x_out}, time: {time:?}, time_range_ui: {time_range_ui:#?}"
        );
    }

    for time_in in 0..=50 {
        let time_in = TimeReal::from(time_in as f64);
        let x = time_range_ui.x_from_time(time_in).unwrap();
        let time_out = time_range_ui.time_from_x_f64(x).unwrap();

        assert!(
            (time_in - time_out).abs().as_f64() < 0.1,
            "time_in: {time_in:?}, time_out: {time_out:?}, x: {x}, time_range_ui: {time_range_ui:#?}"
        );
    }
}
