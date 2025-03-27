use std::collections::BTreeMap;

use re_chunk::TimelineName;
use re_entity_db::{TimeCounts, TimesPerTimeline};
use re_log_types::{
    Duration, ResolvedTimeRange, ResolvedTimeRangeF, TimeInt, TimeReal, TimeType, Timeline,
};

use crate::NeedsRepaint;

/// The time range we are currently zoomed in on.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct TimeView {
    /// Where start of the range.
    pub min: TimeReal,

    /// How much time the full view covers.
    ///
    /// The unit is either nanoseconds or sequence numbers.
    ///
    /// If there is gaps in the data, the actual amount of viewed time might be less.
    pub time_spanned: f64,
}

impl From<ResolvedTimeRange> for TimeView {
    fn from(value: ResolvedTimeRange) -> Self {
        Self {
            min: value.min().into(),
            time_spanned: value.abs_length() as f64,
        }
    }
}

/// State per timeline.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
struct TimeState {
    /// The current time (play marker).
    time: TimeReal,

    /// Frames per second, when playing sequences (they are often video recordings).
    fps: f32,

    /// Selected time range, if any.
    #[serde(default)]
    loop_selection: Option<ResolvedTimeRangeF>,

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

// TODO(andreas): This should be a blueprint property and follow the usual rules of how we determine fallbacks.
#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq)]
enum ActiveTimeline {
    Auto(Timeline),
    UserEdited(Timeline),
}

impl std::ops::Deref for ActiveTimeline {
    type Target = Timeline;

    #[inline]
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Auto(t) | Self::UserEdited(t) => t,
        }
    }
}

#[derive(Clone, PartialEq, serde::Deserialize, serde::Serialize)]
struct TimeStateEntry {
    prev: TimeState,
    current: TimeState,
}

impl TimeStateEntry {
    fn new(time: impl Into<TimeReal>) -> Self {
        let state = TimeState::new(time);
        Self {
            prev: state,
            current: state,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq)]
struct LastFrame {
    timeline: Option<Timeline>,
    playing: bool,
}

impl Default for LastFrame {
    fn default() -> Self {
        Self {
            timeline: None,
            playing: true,
        }
    }
}

/// Controls the global view and progress of the time.
#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq)]
#[serde(default)]
pub struct TimeControl {
    last_frame: LastFrame,

    /// Name of the timeline (e.g. `log_time`).
    timeline: ActiveTimeline,

    states: BTreeMap<TimelineName, TimeStateEntry>,

    /// If true, we are either in [`PlayState::Playing`] or [`PlayState::Following`].
    playing: bool,

    /// If true, we are in "follow" mode (see [`PlayState::Following`]).
    /// Ignored when [`Self.playing`] is `false`.
    following: bool,

    speed: f32,

    looping: Looping,

    /// Range with special highlight.
    ///
    /// This is used during UI interactions. E.g. to show visual history range that's highlighted.
    #[serde(skip)]
    pub highlighted_range: Option<ResolvedTimeRange>,
}

impl Default for TimeControl {
    fn default() -> Self {
        Self {
            last_frame: Default::default(),
            timeline: ActiveTimeline::Auto(default_timeline([])),
            states: Default::default(),
            playing: true,
            following: true,
            speed: 1.0,
            looping: Looping::Off,
            highlighted_range: None,
        }
    }
}

pub struct TimeControlResponse {
    pub needs_repaint: NeedsRepaint,

    /// Set if play state changed.
    ///
    /// * `Some(true)` if playing changed to `true`
    /// * `Some(false)` if playing changed to `false`
    /// * `None` if playing did not change
    pub playing_change: Option<bool>,

    /// Set if timeline changed.
    ///
    /// Contains the timeline name and the current time.
    pub timeline_change: Option<(Timeline, TimeReal)>,

    /// Set if the time changed.
    pub time_change: Option<TimeReal>,
}

impl TimeControlResponse {
    fn no_repaint() -> Self {
        Self::new(NeedsRepaint::No)
    }

    fn new(needs_repaint: NeedsRepaint) -> Self {
        Self {
            needs_repaint,
            playing_change: None,
            timeline_change: None,
            time_change: None,
        }
    }
}

impl TimeControl {
    /// Move the time forward (if playing), and perhaps pause if we've reached the end.
    ///
    /// If `should_diff_state` is true, then the response also contains any changes in state
    /// between last frame and the current one.
    #[must_use]
    pub fn update(
        &mut self,
        times_per_timeline: &TimesPerTimeline,
        stable_dt: f32,
        more_data_is_coming: bool,
        should_diff_state: bool,
    ) -> TimeControlResponse {
        self.select_a_valid_timeline(times_per_timeline);

        let Some(full_range) = self.full_range(times_per_timeline) else {
            return TimeControlResponse::no_repaint(); // we have no data on this timeline yet, so bail
        };

        let needs_repaint = match self.play_state() {
            PlayState::Paused => {
                // It's possible that the playback is paused because e.g. it reached its end, but
                // then the user decides to switch timelines.
                // When they do so, it might be the case that they switch to a timeline they've
                // never interacted with before, in which case we don't even have a time state yet.
                self.states.entry(*self.timeline.name()).or_insert_with(|| {
                    TimeStateEntry::new(if self.following {
                        full_range.max()
                    } else {
                        full_range.min()
                    })
                });
                NeedsRepaint::No
            }
            PlayState::Playing => {
                let dt = stable_dt.min(0.1) * self.speed;

                let state = self
                    .states
                    .entry(*self.timeline.name())
                    .or_insert_with(|| TimeStateEntry::new(full_range.min()));

                if self.looping == Looping::Off && full_range.max() <= state.current.time {
                    // We've reached the end of the data
                    state.current.time = full_range.max().into();

                    if more_data_is_coming {
                        // then let's wait for it without pausing!
                        return TimeControlResponse::no_repaint(); // ui will wake up when more data arrives
                    } else {
                        self.pause();
                        return TimeControlResponse::no_repaint();
                    }
                }

                let loop_range = match self.looping {
                    Looping::Off => None,
                    Looping::Selection => state.current.loop_selection,
                    Looping::All => Some(full_range.into()),
                };

                if let Some(loop_range) = loop_range {
                    state.current.time = state.current.time.max(loop_range.min);
                }

                match self.timeline.typ() {
                    TimeType::Sequence => {
                        state.current.time += TimeReal::from(state.current.fps * dt);
                    }
                    TimeType::DurationNs | TimeType::TimestampNs => {
                        state.current.time += TimeReal::from(Duration::from_secs(dt));
                    }
                }

                if let Some(loop_range) = loop_range {
                    if loop_range.max < state.current.time {
                        state.current.time = loop_range.min; // loop!
                    }
                }

                NeedsRepaint::Yes
            }
            PlayState::Following => {
                // Set the time to the max:
                match self.states.entry(*self.timeline.name()) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(TimeStateEntry::new(full_range.max()));
                    }
                    std::collections::btree_map::Entry::Occupied(mut entry) => {
                        entry.get_mut().current.time = full_range.max().into();
                    }
                }
                NeedsRepaint::No // no need for request_repaint - we already repaint when new data arrives
            }
        };

        let mut response = TimeControlResponse::new(needs_repaint);

        if should_diff_state {
            self.diff_last_frame(&mut response);
        }

        response
    }

    /// Handle updating last frame state and trigger callbacks on changes.
    fn diff_last_frame(&mut self, response: &mut TimeControlResponse) {
        if self.last_frame.playing != self.playing {
            self.last_frame.playing = self.playing;

            if self.playing {
                response.playing_change = Some(true);
            } else {
                response.playing_change = Some(false);
            }
        }

        if self.last_frame.timeline != Some(*self.timeline) {
            self.last_frame.timeline = Some(*self.timeline);

            let time = self
                .time_for_timeline(*self.timeline.name())
                .unwrap_or(TimeReal::MIN);

            response.timeline_change = Some((*self.timeline, time));
        }

        if let Some(state) = self.states.get_mut(self.timeline.name()) {
            // TODO(jan): throttle?
            if state.prev.time != state.current.time {
                state.prev.time = state.current.time;

                response.time_change = Some(state.current.time);
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
                if let Some(timeline_stats) = times_per_timeline.get(self.timeline.name()) {
                    if let Some(state) = self.states.get_mut(self.timeline.name()) {
                        if max(&timeline_stats.per_time) <= state.current.time {
                            state.current.time = min(&timeline_stats.per_time).into();
                        }
                    } else {
                        self.states.insert(
                            *self.timeline.name(),
                            TimeStateEntry::new(min(&timeline_stats.per_time)),
                        );
                    }
                }
            }
            PlayState::Following => {
                self.playing = true;
                self.following = true;

                if let Some(timeline_stats) = times_per_timeline.get(self.timeline.name()) {
                    // Set the time to the max:
                    match self.states.entry(*self.timeline.name()) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(TimeStateEntry::new(max(&timeline_stats.per_time)));
                        }
                        std::collections::btree_map::Entry::Occupied(mut entry) => {
                            entry.get_mut().current.time = max(&timeline_stats.per_time).into();
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
        let Some(timeline_stats) = times_per_timeline.get(self.timeline().name()) else {
            return;
        };

        self.pause();

        if let Some(time) = self.time() {
            #[allow(clippy::collapsible_else_if)]
            let new_time = if let Some(loop_range) = self.active_loop_selection() {
                step_back_time_looped(time, &timeline_stats.per_time, &loop_range)
            } else {
                step_back_time(time, &timeline_stats.per_time).into()
            };
            self.set_time(new_time);
        }
    }

    pub fn step_time_fwd(&mut self, times_per_timeline: &TimesPerTimeline) {
        let Some(stats) = times_per_timeline.get(self.timeline().name()) else {
            return;
        };

        self.pause();

        if let Some(time) = self.time() {
            #[allow(clippy::collapsible_else_if)]
            let new_time = if let Some(loop_range) = self.active_loop_selection() {
                step_fwd_time_looped(time, &stats.per_time, &loop_range)
            } else {
                step_fwd_time(time, &stats.per_time).into()
            };
            self.set_time(new_time);
        }
    }

    pub fn restart(&mut self, times_per_timeline: &TimesPerTimeline) {
        if let Some(stats) = times_per_timeline.get(self.timeline.name()) {
            if let Some(state) = self.states.get_mut(self.timeline.name()) {
                state.current.time = min(&stats.per_time).into();
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
            if let Some(stats) = times_per_timeline.get(self.timeline.name()) {
                if let Some(state) = self.states.get_mut(self.timeline.name()) {
                    if max(&stats.per_time) <= state.current.time {
                        state.current.time = min(&stats.per_time).into();
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
        self.states
            .get(self.timeline().name())
            .map(|state| state.current.fps)
    }

    /// playback fps
    pub fn set_fps(&mut self, fps: f32) {
        if let Some(state) = self.states.get_mut(self.timeline.name()) {
            state.current.fps = fps;
        }
    }

    /// Make sure the selected timeline is a valid one
    pub fn select_a_valid_timeline(&mut self, times_per_timeline: &TimesPerTimeline) {
        fn is_timeline_valid(selected: &Timeline, times_per_timeline: &TimesPerTimeline) -> bool {
            for timeline in times_per_timeline.timelines() {
                if selected == timeline {
                    return true; // it's valid
                }
            }
            false
        }

        // If the timeline is auto refresh it every frame, otherwise only pick a new one if invalid.
        if matches!(self.timeline, ActiveTimeline::Auto(_))
            || !is_timeline_valid(self.timeline(), times_per_timeline)
        {
            self.timeline = ActiveTimeline::Auto(default_timeline(times_per_timeline.timelines()));
        }
    }

    /// The currently selected timeline
    #[inline]
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    /// The time type of the currently selected timeline
    pub fn time_type(&self) -> TimeType {
        self.timeline.typ()
    }

    pub fn set_timeline(&mut self, timeline: Timeline) {
        self.timeline = ActiveTimeline::UserEdited(timeline);
    }

    /// The current time.
    pub fn time(&self) -> Option<TimeReal> {
        self.states
            .get(self.timeline().name())
            .map(|state| state.current.time)
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
    pub fn current_query(&self) -> re_chunk_store::LatestAtQuery {
        re_chunk_store::LatestAtQuery::new(
            *self.timeline.name(),
            self.time().map_or(TimeInt::MAX, |t| t.floor()),
        )
    }

    /// The current loop range, iff selection looping is turned on.
    pub fn active_loop_selection(&self) -> Option<ResolvedTimeRangeF> {
        if self.looping == Looping::Selection {
            self.states
                .get(self.timeline().name())?
                .current
                .loop_selection
        } else {
            None
        }
    }

    /// The full range of times for the current timeline
    pub fn full_range(&self, times_per_timeline: &TimesPerTimeline) -> Option<ResolvedTimeRange> {
        times_per_timeline
            .get(self.timeline().name())
            .map(|stats| range(&stats.per_time))
    }

    /// The selected slice of time that is called the "loop selection".
    ///
    /// This can still return `Some` even if looping is currently off.
    pub fn loop_selection(&self) -> Option<ResolvedTimeRangeF> {
        self.states
            .get(self.timeline().name())?
            .current
            .loop_selection
    }

    /// Set the current loop selection without enabling looping.
    pub fn set_loop_selection(&mut self, selection: ResolvedTimeRangeF) {
        self.states
            .entry(*self.timeline.name())
            .or_insert_with(|| TimeStateEntry::new(selection.min))
            .current
            .loop_selection = Some(selection);
    }

    /// Remove the current loop selection.
    pub fn remove_loop_selection(&mut self) {
        if let Some(state) = self.states.get_mut(self.timeline.name()) {
            state.current.loop_selection = None;
        }
        if self.looping() == Looping::Selection {
            self.set_looping(Looping::Off);
        }
    }

    /// Is the current time in the selection range (if any), or at the current time mark?
    pub fn is_time_selected(&self, timeline: &TimelineName, needle: TimeInt) -> bool {
        if timeline != self.timeline().name() {
            return false;
        }

        if let Some(state) = self.states.get(self.timeline().name()) {
            state.current.time.floor() == needle
        } else {
            false
        }
    }

    pub fn set_timeline_and_time(&mut self, timeline: Timeline, time: impl Into<TimeReal>) {
        self.timeline = ActiveTimeline::UserEdited(timeline);
        self.set_time(time);
    }

    pub fn time_for_timeline(&self, timeline: TimelineName) -> Option<TimeReal> {
        self.states.get(&timeline).map(|state| state.current.time)
    }

    pub fn set_time_for_timeline(&mut self, timeline: Timeline, time: impl Into<TimeReal>) {
        let time = time.into();

        self.states
            .entry(*timeline.name())
            .or_insert_with(|| TimeStateEntry::new(time))
            .current
            .time = time;
    }

    pub fn set_time(&mut self, time: impl Into<TimeReal>) {
        let time = time.into();

        self.states
            .entry(*self.timeline.name())
            .or_insert_with(|| TimeStateEntry::new(time))
            .current
            .time = time;
    }

    /// The range of time we are currently zoomed in on.
    pub fn time_view(&self) -> Option<TimeView> {
        self.states
            .get(self.timeline().name())
            .and_then(|state| state.current.view)
    }

    /// The range of time we are currently zoomed in on.
    pub fn set_time_view(&mut self, view: TimeView) {
        self.states
            .entry(*self.timeline.name())
            .or_insert_with(|| TimeStateEntry::new(view.min))
            .current
            .view = Some(view);
    }

    /// The range of time we are currently zoomed in on.
    pub fn reset_time_view(&mut self) {
        if let Some(state) = self.states.get_mut(self.timeline.name()) {
            state.current.view = None;
        }
    }
}

fn min(values: &TimeCounts) -> TimeInt {
    *values.keys().next().unwrap_or(&TimeInt::MIN)
}

fn max(values: &TimeCounts) -> TimeInt {
    *values.keys().next_back().unwrap_or(&TimeInt::MIN)
}

fn range(values: &TimeCounts) -> ResolvedTimeRange {
    ResolvedTimeRange::new(min(values), max(values))
}

/// Pick the timeline that should be the default, prioritizing user-defined ones.
fn default_timeline<'a>(timelines: impl IntoIterator<Item = &'a Timeline>) -> Timeline {
    let mut found_log_tick = false;
    let mut found_log_time = false;

    for timeline in timelines {
        if timeline == &Timeline::log_tick() {
            found_log_tick = true;
        } else if timeline == &Timeline::log_time() {
            found_log_time = true;
        } else {
            return *timeline;
        }
    }

    if found_log_tick && !found_log_time {
        Timeline::log_tick()
    } else {
        Timeline::log_time()
    }
}

fn step_fwd_time(time: TimeReal, values: &TimeCounts) -> TimeInt {
    if let Some((next, _)) = values
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

fn step_back_time(time: TimeReal, values: &TimeCounts) -> TimeInt {
    if let Some((previous, _)) = values.range(..time.ceil()).next_back() {
        *previous
    } else {
        max(values)
    }
}

fn step_fwd_time_looped(
    time: TimeReal,
    values: &TimeCounts,
    loop_range: &ResolvedTimeRangeF,
) -> TimeReal {
    if time < loop_range.min || loop_range.max <= time {
        loop_range.min
    } else if let Some((next, _)) = values
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
    values: &TimeCounts,
    loop_range: &ResolvedTimeRangeF,
) -> TimeReal {
    if time <= loop_range.min || loop_range.max < time {
        loop_range.max
    } else if let Some((previous, _)) = values.range(loop_range.min.ceil()..time.ceil()).next_back()
    {
        TimeReal::from(*previous)
    } else {
        step_back_time(time, values).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_timeline() {
        let log_time = Timeline::log_time();
        let log_tick = Timeline::log_tick();
        let custom_timeline0 = Timeline::new("my_timeline0", TimeType::DurationNs);
        let custom_timeline1 = Timeline::new("my_timeline1", TimeType::DurationNs);

        assert_eq!(default_timeline([]), log_time);
        assert_eq!(default_timeline([&log_tick]), log_tick);
        assert_eq!(default_timeline([&log_time]), log_time);
        assert_eq!(default_timeline([&log_time, &log_tick]), log_time);
        assert_eq!(
            default_timeline([&log_time, &log_tick, &custom_timeline0]),
            custom_timeline0
        );
        assert_eq!(
            default_timeline([&custom_timeline0, &log_time, &log_tick]),
            custom_timeline0
        );
        assert_eq!(
            default_timeline([&log_time, &custom_timeline0, &log_tick]),
            custom_timeline0
        );
        assert_eq!(
            default_timeline([&custom_timeline0, &log_time]),
            custom_timeline0
        );
        assert_eq!(
            default_timeline([&custom_timeline0, &log_tick]),
            custom_timeline0
        );
        assert_eq!(
            default_timeline([&log_time, &custom_timeline0]),
            custom_timeline0
        );
        assert_eq!(
            default_timeline([&log_tick, &custom_timeline0]),
            custom_timeline0
        );

        assert_eq!(
            default_timeline([&custom_timeline0, &custom_timeline1]),
            custom_timeline0
        );
        assert_eq!(default_timeline([&custom_timeline0]), custom_timeline0);
    }
}
