use std::collections::{BTreeMap, BTreeSet};

use re_data_store::TimesPerTimeline;
use re_log_types::{Duration, TimeInt, TimeRange, TimeRangeF, TimeReal, TimeType, Timeline};

use crate::NeedsRepaint;

/// The time range we are currently zoomed in on.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub struct TimeView {
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
    loop_selection: Option<TimeRangeF>,

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
            loop_selection: Default::default(),
            view: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum Looping {
    /// Looping is off.
    Off,

    /// We are looping within the current loop selection.
    Selection,

    /// We are looping the entire recording.
    ///
    /// The loop selection is ignored.
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum PlayState {
    /// Time doesn't move
    Paused,

    /// Time move steadily
    Playing,

    /// Follow the latest available data
    Following,
}

/// Controls the global view and progress of the time.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TimeControl {
    /// Name of the timeline (e.g. "log_time").
    timeline: Timeline,

    states: BTreeMap<Timeline, TimeState>,

    /// If true, we are either in [`PlayState::Playing`] or [`PlayState::Following`].
    playing: bool,

    /// If true, we are in "follow" mode (see [`PlayState::Following`]).
    /// Ignored when [`Self.playing`] is `false`.
    following: bool,

    speed: f32,

    looping: Looping,
}

impl Default for TimeControl {
    fn default() -> Self {
        Self {
            timeline: Default::default(),
            states: Default::default(),
            playing: true,
            following: true,
            speed: 1.0,
            looping: Looping::Off,
        }
    }
}

impl TimeControl {
    /// Move the time forward (if playing), and perhaps pause if we've reached the end.
    #[must_use]
    pub fn update(
        &mut self,
        times_per_timeline: &TimesPerTimeline,
        stable_dt: f32,
        more_data_is_coming: bool,
    ) -> NeedsRepaint {
        self.select_a_valid_timeline(times_per_timeline);

        let Some(full_range) = self.full_range(times_per_timeline) else {
            return NeedsRepaint::No; // we have no data on this timeline yet, so bail
        };

        match self.play_state() {
            PlayState::Paused => {
                // It's possible that the playback is paused because e.g. it reached its end, but
                // then the user decides to switch timelines.
                // When they do so, it might be the case that they switch to a timeline they've
                // never interacted with before, in which case we don't even have a time state yet.
                self.states.entry(self.timeline).or_insert_with(|| {
                    TimeState::new(if self.following {
                        full_range.max
                    } else {
                        full_range.min
                    })
                });
                NeedsRepaint::No
            }
            PlayState::Playing => {
                let dt = stable_dt.min(0.1) * self.speed;

                let state = self
                    .states
                    .entry(self.timeline)
                    .or_insert_with(|| TimeState::new(full_range.min));

                if self.looping == Looping::Off && full_range.max <= state.time {
                    // We've reached the end of the data
                    state.time = full_range.max.into();

                    if more_data_is_coming {
                        // then let's wait for it without pausing!
                        return NeedsRepaint::No; // ui will wake up when more data arrives
                    } else {
                        self.pause();
                        return NeedsRepaint::No;
                    }
                }

                let loop_range = match self.looping {
                    Looping::Off => None,
                    Looping::Selection => state.loop_selection,
                    Looping::All => Some(full_range.into()),
                };

                if let Some(loop_range) = loop_range {
                    state.time = state.time.max(loop_range.min);
                }

                match self.timeline.typ() {
                    TimeType::Sequence => {
                        state.time += TimeReal::from(state.fps * dt);
                    }
                    TimeType::Time => state.time += TimeReal::from(Duration::from_secs(dt)),
                }

                if let Some(loop_range) = loop_range {
                    if loop_range.max < state.time {
                        state.time = loop_range.min; // loop!
                    }
                }

                NeedsRepaint::Yes
            }
            PlayState::Following => {
                // Set the time to the max:
                match self.states.entry(self.timeline) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(TimeState::new(full_range.max));
                    }
                    std::collections::btree_map::Entry::Occupied(mut entry) => {
                        entry.get_mut().time = full_range.max.into();
                    }
                }
                NeedsRepaint::No // no need for request_repaint - we already repaint when new data arrives
            }
        }
    }

    pub fn play_state(&self) -> PlayState {
        if self.playing {
            if self.following {
                PlayState::Following
            } else {
                PlayState::Playing
            }
        } else {
            PlayState::Paused
        }
    }

    pub fn looping(&self) -> Looping {
        if self.play_state() == PlayState::Following {
            Looping::Off
        } else {
            self.looping
        }
    }

    pub fn set_looping(&mut self, looping: Looping) {
        self.looping = looping;
        if self.looping != Looping::Off {
            // It makes no sense with looping and follow.
            self.following = false;
        }
    }

    pub fn set_play_state(&mut self, times_per_timeline: &TimesPerTimeline, play_state: PlayState) {
        match play_state {
            PlayState::Paused => {
                self.playing = false;
            }
            PlayState::Playing => {
                self.playing = true;
                self.following = false;

                // Start from beginning if we are at the end:
                if let Some(time_points) = times_per_timeline.get(&self.timeline) {
                    if let Some(state) = self.states.get_mut(&self.timeline) {
                        if max(time_points) <= state.time {
                            state.time = min(time_points).into();
                        }
                    } else {
                        self.states
                            .insert(self.timeline, TimeState::new(min(time_points)));
                    }
                }
            }
            PlayState::Following => {
                self.playing = true;
                self.following = true;

                if let Some(time_points) = times_per_timeline.get(&self.timeline) {
                    // Set the time to the max:
                    match self.states.entry(self.timeline) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(TimeState::new(max(time_points)));
                        }
                        std::collections::btree_map::Entry::Occupied(mut entry) => {
                            entry.get_mut().time = max(time_points).into();
                        }
                    }
                }
            }
        }
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn step_time_back(&mut self, times_per_timeline: &TimesPerTimeline) {
        let Some(time_values) = times_per_timeline.get(self.timeline()) else { return; };

        self.pause();

        if let Some(time) = self.time() {
            #[allow(clippy::collapsible_else_if)]
            let new_time = if let Some(loop_range) = self.active_loop_selection() {
                step_back_time_looped(time, time_values, &loop_range)
            } else {
                step_back_time(time, time_values).into()
            };
            self.set_time(new_time);
        }
    }

    pub fn step_time_fwd(&mut self, times_per_timeline: &TimesPerTimeline) {
        let Some(time_values) = times_per_timeline.get(self.timeline()) else { return; };

        self.pause();

        if let Some(time) = self.time() {
            #[allow(clippy::collapsible_else_if)]
            let new_time = if let Some(loop_range) = self.active_loop_selection() {
                step_fwd_time_looped(time, time_values, &loop_range)
            } else {
                step_fwd_time(time, time_values).into()
            };
            self.set_time(new_time);
        }
    }

    pub fn restart(&mut self, times_per_timeline: &TimesPerTimeline) {
        if let Some(time_points) = times_per_timeline.get(&self.timeline) {
            if let Some(state) = self.states.get_mut(&self.timeline) {
                state.time = min(time_points).into();
                self.following = false;
            }
        }
    }

    pub fn toggle_play_pause(&mut self, times_per_timeline: &TimesPerTimeline) {
        #[allow(clippy::collapsible_else_if)]
        if self.playing {
            self.pause();
        } else {
            // If we are in follow-mode (but paused), what should toggling play/pause do?
            //
            // There are two cases to consider:
            // * We are looking at a file
            // * We are following a stream
            //
            // If we are watching a stream, it makes sense to keep following:
            // you paused to look at something, now you're done, so keep following.
            //
            // If you are watching a file: if the file has finished loading, then
            // it can still make sense to go to the end of it.
            // But if you're already at the end, then staying at "follow" makes little sense,
            // as repeated toggling will just go between paused and follow at the latest data.
            // This is made worse by Follow being our default mode (even for files).
            //
            // As of writing (2023-02) we don't know if we are watching a file or a stream
            // (after all, files are also streamed).
            //
            // So we use a heuristic:
            // If we are at the end of the file and unpause, we always start from
            // the beginning in play mode.

            // Start from beginning if we are at the end:
            if let Some(time_points) = times_per_timeline.get(&self.timeline) {
                if let Some(state) = self.states.get_mut(&self.timeline) {
                    if max(time_points) <= state.time {
                        state.time = min(time_points).into();
                        self.playing = true;
                        self.following = false;
                        return;
                    }
                }
            }

            if self.following {
                self.set_play_state(times_per_timeline, PlayState::Following);
            } else {
                self.set_play_state(times_per_timeline, PlayState::Playing);
            }
        }
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

    /// Make sure the selected timeline is a valid one
    pub fn select_a_valid_timeline(&mut self, times_per_timeline: &TimesPerTimeline) {
        for timeline in times_per_timeline.timelines() {
            if &self.timeline == timeline {
                return; // it's valid
            }
        }
        if let Some(timeline) = default_timeline(times_per_timeline.timelines()) {
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
        self.time().map(|t| t.floor())
    }

    /// The current time.
    pub fn time_i64(&self) -> Option<i64> {
        self.time().map(|t| t.floor().as_i64())
    }

    /// Query for latest value at the currently selected time on the currently selected timeline.
    pub fn current_query(&self) -> re_arrow_store::LatestAtQuery {
        re_arrow_store::LatestAtQuery::new(
            self.timeline,
            self.time().map_or(TimeInt::MAX, |t| t.floor()),
        )
    }

    /// The current loop range, iff selection looping is turned on.
    pub fn active_loop_selection(&self) -> Option<TimeRangeF> {
        if self.looping == Looping::Selection {
            self.states.get(&self.timeline)?.loop_selection
        } else {
            None
        }
    }

    /// The full range of times for the current timeline
    pub fn full_range(&self, times_per_timeline: &TimesPerTimeline) -> Option<TimeRange> {
        times_per_timeline.get(&self.timeline).map(range)
    }

    /// The selected slice of time that is called the "loop selection".
    ///
    /// This can still return `Some` even if looping is currently off.
    pub fn loop_selection(&self) -> Option<TimeRangeF> {
        self.states.get(&self.timeline)?.loop_selection
    }

    /// Set the current loop selection without enabling looping.
    pub fn set_loop_selection(&mut self, selection: TimeRangeF) {
        self.states
            .entry(self.timeline)
            .or_insert_with(|| TimeState::new(selection.min))
            .loop_selection = Some(selection);
    }

    /// Remove the current loop selection.
    pub fn remove_loop_selection(&mut self) {
        if let Some(state) = self.states.get_mut(&self.timeline) {
            state.loop_selection = None;
        }
        if self.looping() == Looping::Selection {
            self.set_looping(Looping::Off);
        }
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
}

fn min(values: &BTreeSet<TimeInt>) -> TimeInt {
    *values.iter().next().unwrap_or(&TimeInt::BEGINNING)
}

fn max(values: &BTreeSet<TimeInt>) -> TimeInt {
    *values.iter().rev().next().unwrap_or(&TimeInt::BEGINNING)
}

fn range(values: &BTreeSet<TimeInt>) -> TimeRange {
    TimeRange::new(min(values), max(values))
}

/// Pick the timeline that should be the default, prioritizing user-defined ones.
fn default_timeline<'a>(timelines: impl Iterator<Item = &'a Timeline>) -> Option<&'a Timeline> {
    let mut log_time_timeline = None;

    for timeline in timelines {
        if timeline == &Timeline::log_time() {
            log_time_timeline = Some(timeline);
        } else if timeline != &Timeline::log_tick() {
            return Some(timeline); // user timeline - always prefer!
        }
    }

    log_time_timeline
}

fn step_fwd_time(time: TimeReal, values: &BTreeSet<TimeInt>) -> TimeInt {
    if let Some(next) = values
        .range((
            std::ops::Bound::Excluded(time.floor()),
            std::ops::Bound::Unbounded,
        ))
        .next()
    {
        *next
    } else {
        min(values)
    }
}

fn step_back_time(time: TimeReal, values: &BTreeSet<TimeInt>) -> TimeInt {
    if let Some(previous) = values.range(..time.ceil()).rev().next() {
        *previous
    } else {
        max(values)
    }
}

fn step_fwd_time_looped(
    time: TimeReal,
    values: &BTreeSet<TimeInt>,
    loop_range: &TimeRangeF,
) -> TimeReal {
    if time < loop_range.min || loop_range.max <= time {
        loop_range.min
    } else if let Some(next) = values
        .range((
            std::ops::Bound::Excluded(time.floor()),
            std::ops::Bound::Included(loop_range.max.floor()),
        ))
        .next()
    {
        TimeReal::from(*next)
    } else {
        step_fwd_time(time, values).into()
    }
}

fn step_back_time_looped(
    time: TimeReal,
    values: &BTreeSet<TimeInt>,
    loop_range: &TimeRangeF,
) -> TimeReal {
    if time <= loop_range.min || loop_range.max < time {
        loop_range.max
    } else if let Some(previous) = values
        .range(loop_range.min.ceil()..time.ceil())
        .rev()
        .next()
    {
        TimeReal::from(*previous)
    } else {
        step_back_time(time, values).into()
    }
}
