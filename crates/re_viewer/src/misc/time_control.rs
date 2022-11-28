use std::collections::BTreeMap;

use egui::NumExt as _;

use re_data_store::{TimeQuery, TimesPerTimeline};
use re_log_types::*;

use super::{TimeRange, TimeRangeF, TimeReal};

/// The time range we are currently zoomed in on.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct TimeView {
    /// Where start of the the range.
    pub min: TimeReal,

    /// How much time the full view covers.
    ///
    /// The unit is either nanoseconds or sequence numbers.
    ///
    /// If there is gaps in the data, the actual amount of viewed time might be less.
    pub time_spanned: f64,
}

/// State per timeline.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
struct TimeState {
    /// The current time (play marker).
    time: TimeReal,

    /// Frames Per Second, when playing sequences (they are often video recordings).
    fps: f32,

    /// Selected time range, if any.
    #[serde(default)]
    selection: Option<TimeRangeF>,

    /// The time range we are currently zoomed in on.
    ///
    /// `None` means "everything", and is the default value.
    /// In this case, the view will expand while new data is added.
    /// Only when the user actually zooms or pans will this be set.
    #[serde(default)]
    view: Option<TimeView>,
}

impl TimeState {
    fn new(time: impl Into<TimeReal>) -> Self {
        Self {
            time: time.into(),
            fps: 30.0, // TODO(emilk): estimate based on data
            selection: Default::default(),
            view: None,
        }
    }
}

/// Controls the global view and progress of the time.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct TimeControl {
    /// Name of the timeline (e.g. "log_time").
    timeline: Timeline,

    states: BTreeMap<Timeline, TimeState>,

    playing: bool,
    looped: bool,
    speed: f32,

    #[serde(default)]
    pub loop_selection_active: bool,
}

impl Default for TimeControl {
    fn default() -> Self {
        Self {
            timeline: Default::default(),
            states: Default::default(),
            playing: true,
            looped: false,
            speed: 1.0,
            loop_selection_active: false,
        }
    }
}

impl TimeControl {
    /// Update the current time
    pub fn move_time(&mut self, egui_ctx: &egui::Context, times_per_timeline: &TimesPerTimeline) {
        self.select_a_valid_timeline(times_per_timeline);

        if !self.playing {
            return;
        }

        let Some(full_range) = self.full_range(times_per_timeline) else {
            return;
        };

        let state = self
            .states
            .entry(self.timeline)
            .or_insert_with(|| TimeState::new(full_range.min));

        let loop_range = if self.looped {
            if self.loop_selection_active {
                state.selection.unwrap_or_else(|| full_range.into())
            } else {
                full_range.into()
            }
        } else {
            full_range.into()
        };

        let dt = egui_ctx.input().stable_dt.at_most(0.1) * self.speed;

        // ----

        if self.looped {
            state.time = state.time.max(loop_range.min);
        }

        if state.time >= loop_range.max && !self.looped {
            // Don't pause or rewind, just stop moving time forward
            // until we receive more data!
            // This is important for "live view".
            return;
        }

        match self.timeline.typ() {
            TimeType::Sequence => {
                state.time += TimeReal::from(state.fps * dt);
            }
            TimeType::Time => state.time += TimeReal::from(Duration::from_secs(dt)),
        }
        egui_ctx.request_repaint(); // keep playing next frame

        if state.time > loop_range.max && self.looped {
            state.time = loop_range.min;
        }
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn play(&mut self, times_per_timeline: &TimesPerTimeline) {
        if self.playing {
            return;
        }

        // Start from beginning if we are at the end:
        if let Some(axis) = times_per_timeline.get(&self.timeline) {
            if let Some(state) = self.states.get_mut(&self.timeline) {
                if state.time >= max(axis) {
                    state.time = min(axis).into();
                }
            } else {
                self.states.insert(self.timeline, TimeState::new(min(axis)));
            }
        }
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// playback speed
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// playback speed
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    /// playback fps
    pub fn fps(&self) -> Option<f32> {
        self.states.get(&self.timeline).map(|state| state.fps)
    }

    /// playback fps
    pub fn set_fps(&mut self, fps: f32) {
        if let Some(state) = self.states.get_mut(&self.timeline) {
            state.fps = fps;
        }
    }

    /// looped playback enabled?
    pub fn looped(&self) -> bool {
        self.looped
    }

    /// looped playback enabled?
    pub fn set_looped(&mut self, looped: bool) {
        self.looped = looped;
    }

    /// Make sure the selected timeline is a valid one
    pub fn select_a_valid_timeline(&mut self, times_per_timeline: &TimesPerTimeline) {
        for timeline in times_per_timeline.timelines() {
            if &self.timeline == timeline {
                return; // it's valid
            }
        }
        if let Some(timeline) = default_time_line(times_per_timeline.timelines()) {
            self.timeline = *timeline;
        } else {
            self.timeline = Default::default();
        }
    }

    /// The currently selected timeline
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    /// The time type of the currently selected timeline
    pub fn time_type(&self) -> TimeType {
        self.timeline.typ()
    }

    pub fn set_timeline(&mut self, timeline: Timeline) {
        self.timeline = timeline;
    }

    /// The current time.
    pub fn time(&self) -> Option<TimeReal> {
        self.states.get(&self.timeline).map(|state| state.time)
    }

    /// The current time.
    pub fn time_int(&self) -> Option<TimeInt> {
        Some(self.time()?.floor())
    }

    /// The current filtered time.
    /// Returns a "point" range if we have no selection (normal play)
    pub fn time_range(&self) -> Option<TimeRangeF> {
        let state = self.states.get(&self.timeline)?;
        Some(TimeRangeF::point(state.time)) // TODO: remove function
    }

    /// The current loop range, iff looping is turned on
    pub fn loop_range(&self) -> Option<TimeRangeF> {
        if self.loop_selection_active {
            self.states.get(&self.timeline)?.selection
        } else {
            None
        }
    }

    /// The full range of times for the current timeline
    pub fn full_range(&self, times_per_timeline: &TimesPerTimeline) -> Option<TimeRange> {
        times_per_timeline.get(&self.timeline).map(range)
    }

    /// Is the current time in the selection range (if any), or at the current time mark?
    pub fn is_time_selected(&self, timeline: &Timeline, needle: TimeInt) -> bool {
        if timeline != &self.timeline {
            return false;
        }

        if let Some(state) = self.states.get(&self.timeline) {
            state.time.floor() == needle
        } else {
            false
        }
    }

    pub fn set_timeline_and_time(&mut self, timeline: Timeline, time: impl Into<TimeReal>) {
        self.timeline = timeline;
        self.set_time(time);
    }

    pub fn set_time(&mut self, time: impl Into<TimeReal>) {
        let time = time.into();

        self.states
            .entry(self.timeline)
            .or_insert_with(|| TimeState::new(time))
            .time = time;
    }

    /// The range of time we are currently zoomed in on.
    pub fn time_view(&self) -> Option<TimeView> {
        self.states.get(&self.timeline).and_then(|state| state.view)
    }

    /// The range of time we are currently zoomed in on.
    pub fn set_time_view(&mut self, view: TimeView) {
        self.states
            .entry(self.timeline)
            .or_insert_with(|| TimeState::new(view.min))
            .view = Some(view);
    }

    /// The range of time we are currently zoomed in on.
    pub fn reset_time_view(&mut self) {
        if let Some(state) = self.states.get_mut(&self.timeline) {
            state.view = None;
        }
    }

    pub fn time_selection(&self) -> Option<TimeRangeF> {
        self.states.get(&self.timeline)?.selection
    }

    pub fn set_time_selection(&mut self, selection: TimeRangeF) {
        self.states
            .entry(self.timeline)
            .or_insert_with(|| TimeState::new(selection.min))
            .selection = Some(selection);
    }

    pub fn time_query(&self) -> Option<TimeQuery<i64>> {
        Some(TimeQuery::LatestAt(self.time_int()?.as_i64()))
    }
}

fn min<T>(values: &BTreeMap<TimeInt, T>) -> TimeInt {
    *values.keys().next().unwrap()
}

fn max<T>(values: &BTreeMap<TimeInt, T>) -> TimeInt {
    *values.keys().rev().next().unwrap()
}

fn range<T>(values: &BTreeMap<TimeInt, T>) -> TimeRange {
    TimeRange::new(min(values), max(values))
}

/// Pick the timeline that should be the default, prioritizing user-defined ones.
fn default_time_line<'a>(timelines: impl Iterator<Item = &'a Timeline>) -> Option<&'a Timeline> {
    let mut log_time_timeline = None;

    for timeline in timelines {
        if timeline.name().as_str() == "log_time" {
            log_time_timeline = Some(timeline);
        } else {
            return Some(timeline); // user timeline - always prefer!
        }
    }

    log_time_timeline
}
