use std::collections::BTreeMap;

use nohash_hasher::IntMap;
use re_global_context::time_control_command::{Looping, PlayState, TimeControlCommand, TimeView};
use re_types::blueprint::archetypes::TimePanelBlueprint;
use vec1::Vec1;

use re_chunk::{EntityPath, TimelineName};
use re_entity_db::{TimeCounts, TimelineStats, TimesPerTimeline};
use re_log_types::{
    AbsoluteTimeRange, AbsoluteTimeRangeF, Duration, TimeCell, TimeInt, TimeReal, TimeType,
    Timeline,
};

use crate::{NeedsRepaint, blueprint_helpers::BlueprintContext};

pub const TIME_PANEL_PATH: &str = "time_panel";

pub fn time_panel_blueprint_entity_path() -> EntityPath {
    TIME_PANEL_PATH.into()
}

/// Helper trait to write time panel related blueprint components.
trait TimeBlueprintExt {
    fn set_time(&self, time: impl Into<TimeInt>);

    fn get_time(&self) -> Option<TimeInt>;

    fn set_timeline(&self, timeline: TimelineName);

    fn get_timeline(&self) -> Option<TimelineName>;

    /// Replaces the current timeline with the automatic one.
    fn clear_timeline(&self);

    /// Clears the blueprint time cursor, and will instead fall back
    /// to a default one, most likely the one saved in time control's
    /// per timeline state.
    fn clear_time(&self);
}

impl<T: BlueprintContext> TimeBlueprintExt for T {
    fn set_time(&self, time: impl Into<TimeInt>) {
        let time: TimeInt = time.into();
        self.save_static_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_time(),
            &re_types::blueprint::components::TimeCell(time.as_i64().into()),
        );
    }

    fn get_time(&self) -> Option<TimeInt> {
        let (_, time) = self
            .current_blueprint()
            .latest_at_component_quiet::<re_types::blueprint::components::TimeCell>(
                &time_panel_blueprint_entity_path(),
                self.blueprint_query(),
                &TimePanelBlueprint::descriptor_time(),
            )?;

        Some(TimeInt::saturated_temporal_i64(time.0.0))
    }

    fn set_timeline(&self, timeline: TimelineName) {
        self.save_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_timeline(),
            &re_types::blueprint::components::TimelineName::from(timeline.as_str()),
        );
        self.clear_time();
    }

    fn get_timeline(&self) -> Option<TimelineName> {
        let (_, timeline) = self
            .current_blueprint()
            .latest_at_component_quiet::<re_types::blueprint::components::TimelineName>(
            &time_panel_blueprint_entity_path(),
            self.blueprint_query(),
            &TimePanelBlueprint::descriptor_timeline(),
        )?;

        Some(TimelineName::new(timeline.as_str()))
    }

    fn clear_timeline(&self) {
        self.clear_blueprint_component(
            time_panel_blueprint_entity_path(),
            TimePanelBlueprint::descriptor_timeline(),
        );
    }

    fn clear_time(&self) {
        self.clear_static_blueprint_component(
            time_panel_blueprint_entity_path(),
            TimePanelBlueprint::descriptor_time(),
        );
    }
}

/// State per timeline.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
struct TimeState {
    /// The current time (play marker).
    time: TimeReal,

    /// The last time this timeline was paused at.
    ///
    /// Used for the web url.
    #[serde(skip)]
    last_paused_time: Option<TimeReal>,

    /// Frames per second, when playing sequences (they are often video recordings).
    fps: f32,

    /// Selected time range, if any.
    #[serde(default)]
    loop_selection: Option<AbsoluteTimeRangeF>,

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
            last_paused_time: None,
            fps: 30.0, // TODO(emilk): estimate based on data
            loop_selection: Default::default(),
            view: None,
        }
    }
}

// TODO(andreas): This should be a blueprint property and follow the usual rules of how we determine fallbacks.
#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq)]
enum ActiveTimeline {
    Auto(Timeline),
    UserEdited(Timeline),
    Pending(Timeline),
}

impl std::ops::Deref for ActiveTimeline {
    type Target = Timeline;

    #[inline]
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Auto(t) | Self::UserEdited(t) | Self::Pending(t) => t,
        }
    }
}

/// Controls the global view and progress of the time.
#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq)]
#[serde(default)]
pub struct TimeControl {
    /// Name of the timeline (e.g. `log_time`).
    timeline: ActiveTimeline,

    states: BTreeMap<TimelineName, TimeState>,

    /// Valid time ranges for each timeline.
    ///
    /// If a timeline is not in the map, all it's ranges are considered to be valid.
    valid_time_ranges: IntMap<TimelineName, Vec1<AbsoluteTimeRange>>,

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
    pub highlighted_range: Option<AbsoluteTimeRange>,
}

impl Default for TimeControl {
    fn default() -> Self {
        Self {
            timeline: ActiveTimeline::Auto(default_timeline([])),
            states: Default::default(),
            valid_time_ranges: Default::default(),
            playing: true,
            following: true,
            speed: 1.0,
            looping: Looping::Off,
            highlighted_range: None,
        }
    }
}

#[must_use]
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
    pub fn from_blueprint(blueprint_ctx: &impl BlueprintContext) -> Self {
        let mut this = Self::default();

        this.update_from_blueprint(blueprint_ctx, None);

        this
    }

    /// Read from the time panel blueprint and update the state from that.
    ///
    /// If `times_per_timeline` is some this will also make sure we are on
    /// a valid timeline.
    pub fn update_from_blueprint(
        &mut self,
        blueprint_ctx: &impl BlueprintContext,
        times_per_timeline: Option<&TimesPerTimeline>,
    ) {
        if let Some(timeline) = blueprint_ctx.get_timeline() {
            if matches!(self.timeline, ActiveTimeline::Auto(_))
                || timeline.as_str() != self.timeline().name().as_str()
            {
                self.timeline = ActiveTimeline::Pending(Timeline::new_sequence(timeline));
            }
        } else {
            self.timeline = ActiveTimeline::Auto(*self.timeline());
        }

        // Make sure we are on a valid timeline.
        if let Some(times_per_timeline) = times_per_timeline {
            self.select_valid_timeline(times_per_timeline);
        }

        if let Some(time) = blueprint_ctx.get_time() {
            if self.time_int() != Some(time) {
                self.states
                    .entry(*self.timeline().name())
                    .or_insert_with(|| TimeState::new(time))
                    .time = time.into();
            }
        }
        // If the blueprint time wasn't set, but the current state's time was, we likely just switched timelines, so restore that timeline's time.
        else if let Some(state) = self.states.get(self.timeline().name()) {
            blueprint_ctx.set_time(state.time.floor());
        }
        // If we can't restore that timeline's state, we are on a new timeline.
        //
        // Then insert that new state at the start. Or end if we're following.
        else if let Some(times_per_timeline) = times_per_timeline
            && let Some(full_valid_range) = self.full_valid_range(times_per_timeline)
        {
            self.states.insert(
                *self.timeline.name(),
                TimeState::new(if self.following {
                    full_valid_range.max
                } else {
                    full_valid_range.min
                }),
            );
        }

        let play_state = self.play_state();

        // Update the last paused time if we are paused.
        if let Some(state) = self.states.get_mut(self.timeline.name()) {
            match play_state {
                PlayState::Paused => {
                    state.last_paused_time = Some(state.time);
                }
                PlayState::Playing | PlayState::Following => {}
            }
        }
    }

    /// Sets the current time.
    ///
    /// If `blueprint_ctx` is some, this will also update the time stored in
    /// the blueprint if `time_int` has changed.
    fn update_time(&mut self, blueprint_ctx: Option<&impl BlueprintContext>, time: TimeReal) {
        let time_int = time.floor();
        if self.time_int() != Some(time_int)
            && let Some(blueprint_ctx) = blueprint_ctx
        {
            blueprint_ctx.set_time(time_int);
        }

        self.states
            .entry(*self.timeline.name())
            .or_insert_with(|| TimeState::new(time))
            .time = time;
    }

    /// Create [`TimeControlCommand`]s to move the time forward (if playing), and perhaps pause if
    /// we've reached the end.
    pub fn update(
        &mut self,
        times_per_timeline: &TimesPerTimeline,
        stable_dt: f32,
        more_data_is_coming: bool,
        should_diff_state: bool,
        blueprint_ctx: Option<&impl BlueprintContext>,
    ) -> TimeControlResponse {
        let (old_playing, old_timeline, old_state) = (
            self.playing,
            *self.timeline(),
            self.states.get(self.timeline.name()).copied(),
        );

        if let Some(blueprint_ctx) = blueprint_ctx {
            self.update_from_blueprint(blueprint_ctx, Some(times_per_timeline));
        } else {
            self.select_valid_timeline(times_per_timeline);
        }

        let Some(full_valid_range) = self.full_valid_range(times_per_timeline) else {
            return TimeControlResponse::no_repaint(); // we have no data on this timeline yet, so bail
        };

        let needs_repaint = match self.play_state() {
            PlayState::Paused => {
                // It's possible that the playback is paused because e.g. it reached its end, but
                // then the user decides to switch timelines.
                // When they do so, it might be the case that they switch to a timeline they've
                // never interacted with before, in which case we don't even have a time state yet.
                let state = self.states.entry(*self.timeline.name()).or_insert_with(|| {
                    TimeState::new(if self.following {
                        full_valid_range.max()
                    } else {
                        full_valid_range.min()
                    })
                });

                state.last_paused_time = Some(state.time);
                NeedsRepaint::No
            }
            PlayState::Playing => {
                let dt = stable_dt.min(0.1) * self.speed;

                let state = self
                    .states
                    .entry(*self.timeline.name())
                    .or_insert_with(|| TimeState::new(full_valid_range.min()));

                if self.looping == Looping::Off && full_valid_range.max() <= state.time {
                    // We've reached the end of the data
                    self.update_time(blueprint_ctx, full_valid_range.max().into());

                    if more_data_is_coming {
                        // then let's wait for it without pausing!
                        return TimeControlResponse::no_repaint(); // ui will wake up when more data arrives
                    } else {
                        self.pause();
                        return TimeControlResponse::no_repaint();
                    }
                }

                let mut new_time = state.time;

                let loop_range = match self.looping {
                    Looping::Off => None,
                    Looping::Selection => state.loop_selection,
                    Looping::All => Some(full_valid_range.into()),
                };

                match self.timeline.typ() {
                    TimeType::Sequence => {
                        new_time += TimeReal::from(state.fps * dt);
                    }
                    TimeType::DurationNs | TimeType::TimestampNs => {
                        new_time += TimeReal::from(Duration::from_secs(dt));
                    }
                }

                if let Some(loop_range) = loop_range
                    && loop_range.max < new_time
                {
                    new_time = loop_range.min; // loop!
                }

                // Confine cursor to valid ranges.
                {
                    let valid_ranges = self
                        .valid_time_ranges
                        .get(self.timeline.name())
                        .cloned()
                        .unwrap_or_else(|| Vec1::new(AbsoluteTimeRange::EVERYTHING));

                    // The valid range index that the time cursor is either contained in or just behind.
                    let next_valid_range_idx =
                        valid_ranges.partition_point(|range| range.max() < new_time);
                    let clamp_range = valid_ranges
                        .get(next_valid_range_idx)
                        .unwrap_or_else(|| valid_ranges.last());
                    new_time = new_time.clamp(clamp_range.min().into(), clamp_range.max().into());
                }

                self.update_time(blueprint_ctx, new_time);

                NeedsRepaint::Yes
            }
            PlayState::Following => {
                // Set the time to the max:
                self.update_time(blueprint_ctx, full_valid_range.max().into());

                NeedsRepaint::No // no need for request_repaint - we already repaint when new data arrives
            }
        };

        let mut response = TimeControlResponse::new(needs_repaint);

        // Only diff if the caller asked for it, _and_ we have some times on the timeline.
        let should_diff_state = should_diff_state
            && times_per_timeline
                .get(self.timeline.name())
                .is_some_and(|stats| !stats.per_time.is_empty());
        if should_diff_state {
            self.diff_with(&mut response, old_timeline, old_playing, old_state);
        }

        response
    }

    /// Handle updating last frame state and trigger callbacks on changes.
    fn diff_with(
        &mut self,
        response: &mut TimeControlResponse,
        old_timeline: Timeline,
        old_playing: bool,
        old_state: Option<TimeState>,
    ) {
        if old_playing != self.playing {
            response.playing_change = Some(self.playing);
        }

        if old_timeline != *self.timeline {
            let time = self
                .time_for_timeline(*self.timeline.name())
                .unwrap_or(TimeReal::MIN);

            response.timeline_change = Some((*self.timeline, time));
        }

        if let Some(state) = self.states.get_mut(self.timeline.name()) {
            // TODO(jan): throttle?
            if old_state.is_none_or(|old_state| old_state.time != state.time) {
                response.time_change = Some(state.time);
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

    pub fn handle_time_commands(
        &mut self,
        blueprint_ctx: Option<&impl BlueprintContext>,
        times_per_timeline: &TimesPerTimeline,
        commands: &[TimeControlCommand],
    ) -> TimeControlResponse {
        let mut response = TimeControlResponse {
            needs_repaint: NeedsRepaint::No,
            playing_change: None,
            timeline_change: None,
            time_change: None,
        };

        let (old_playing, old_timeline, old_state) = (
            self.playing,
            *self.timeline(),
            self.states.get(self.timeline.name()).copied(),
        );

        let mut redraw = false;

        for command in commands {
            redraw |= self.handle_time_command(blueprint_ctx, times_per_timeline, command);
        }

        if redraw {
            response.needs_repaint = NeedsRepaint::Yes;
        }

        self.diff_with(&mut response, old_timeline, old_playing, old_state);

        response
    }

    /// Applies a time command with respect to the current timeline.
    ///
    /// If `blueprint_ctx` is some, this also writes to that blueprint
    /// for applicable commands.
    ///
    /// Returns if the command should cause a repaint.
    fn handle_time_command(
        &mut self,
        blueprint_ctx: Option<&impl BlueprintContext>,
        times_per_timeline: &TimesPerTimeline,
        command: &TimeControlCommand,
    ) -> bool {
        match command {
            TimeControlCommand::HighlightRange(range) => {
                self.highlighted_range = Some(*range);

                true
            }
            TimeControlCommand::ClearHighlighedRange => {
                self.highlighted_range = None;

                true
            }
            TimeControlCommand::ResetActiveTimeline => {
                if let Some(blueprint_ctx) = blueprint_ctx {
                    blueprint_ctx.clear_timeline();
                }
                self.timeline = ActiveTimeline::Auto(*self.timeline());

                true
            }
            TimeControlCommand::SetActiveTimeline(timeline_name) => {
                if let Some(blueprint_ctx) = blueprint_ctx {
                    blueprint_ctx.set_timeline(*timeline_name);
                }

                if let Some(stats) = times_per_timeline.get(timeline_name) {
                    self.timeline = ActiveTimeline::UserEdited(stats.timeline);
                } else {
                    self.timeline = ActiveTimeline::Pending(Timeline::new_sequence(*timeline_name));
                }

                if let Some(full_valid_range) = self.full_valid_range(times_per_timeline) {
                    self.states
                        .entry(*timeline_name)
                        .or_insert_with(|| TimeState::new(full_valid_range.min));
                }

                true
            }
            TimeControlCommand::SetLooping(looping) => {
                self.looping = *looping;
                if self.looping != Looping::Off {
                    // It makes no sense with looping and follow.
                    self.following = false;
                }

                true
            }
            TimeControlCommand::SetPlayState(play_state) => {
                self.set_play_state(times_per_timeline, *play_state, blueprint_ctx);

                true
            }
            TimeControlCommand::Pause => {
                let redraw = self.playing;

                self.pause();

                redraw
            }
            TimeControlCommand::TogglePlayPause => {
                self.toggle_play_pause(times_per_timeline, blueprint_ctx);

                true
            }
            TimeControlCommand::StepTimeBack => {
                self.step_time_back(times_per_timeline, blueprint_ctx);

                true
            }
            TimeControlCommand::StepTimeForward => {
                self.step_time_fwd(times_per_timeline, blueprint_ctx);

                true
            }
            TimeControlCommand::Restart => {
                if let Some(full_valid_range) = self.full_valid_range(times_per_timeline) {
                    self.following = false;

                    if let Some(blueprint_ctx) = blueprint_ctx {
                        blueprint_ctx.set_time(full_valid_range.min);
                    }

                    if let Some(state) = self.states.get_mut(self.timeline.name()) {
                        state.time = full_valid_range.min.into();
                    }

                    true
                } else {
                    false
                }
            }
            TimeControlCommand::SetSpeed(speed) => {
                let redraw = *speed != self.speed;

                self.speed = *speed;

                redraw
            }
            TimeControlCommand::SetFps(fps) => {
                if let Some(state) = self.states.get_mut(self.timeline.name()) {
                    let redraw = state.fps != *fps;

                    state.fps = *fps;

                    redraw
                } else {
                    false
                }
            }
            TimeControlCommand::SetLoopSelection(time_range) => {
                if let Some(state) = self.states.get_mut(self.timeline.name()) {
                    state.loop_selection = Some((*time_range).into());

                    true
                } else {
                    false
                }
            }
            TimeControlCommand::RemoveLoopSelection => {
                if let Some(state) = self.states.get_mut(self.timeline.name()) {
                    state.loop_selection = None;

                    true
                } else {
                    false
                }
            }
            TimeControlCommand::SetTime(time) => {
                let time_int = time.floor();
                let update_blueprint = self.time_int() != Some(time_int);
                if let Some(blueprint_ctx) = blueprint_ctx
                    && update_blueprint
                {
                    blueprint_ctx.set_time(time_int);
                }
                self.states
                    .entry(*self.timeline.name())
                    .or_insert_with(|| TimeState::new(*time))
                    .time = *time;

                update_blueprint
            }
            TimeControlCommand::SetTimeView(time_view) => {
                if let Some(state) = self.states.get_mut(self.timeline.name()) {
                    state.view = Some(*time_view);

                    true
                } else {
                    false
                }
            }
            TimeControlCommand::ResetTimeView => {
                if let Some(state) = self.states.get_mut(self.timeline.name()) {
                    state.view = None;

                    true
                } else {
                    false
                }
            }
            TimeControlCommand::AddValidTimeRange {
                timeline,
                time_range,
            } => {
                self.mark_time_range_valid(*timeline, *time_range);
                true
            }
        }
    }

    /// Updates the current play-state.
    ///
    /// If `blueprint_ctx` is specified this writes to the related
    /// blueprint.
    pub fn set_play_state(
        &mut self,
        times_per_timeline: &TimesPerTimeline,
        play_state: PlayState,
        blueprint_ctx: Option<&impl BlueprintContext>,
    ) {
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
                        if max(&timeline_stats.per_time) <= state.time {
                            let new_time = min(&timeline_stats.per_time);
                            if let Some(blueprint_ctx) = blueprint_ctx {
                                blueprint_ctx.set_time(new_time);
                            }
                            state.time = new_time.into();
                        }
                    } else {
                        let new_time = min(&timeline_stats.per_time);
                        if let Some(blueprint_ctx) = blueprint_ctx {
                            blueprint_ctx.set_time(new_time);
                        }
                        self.states
                            .insert(*self.timeline.name(), TimeState::new(new_time));
                    }
                }
            }
            PlayState::Following => {
                self.playing = true;
                self.following = true;

                if let Some(timeline_stats) = times_per_timeline.get(self.timeline.name()) {
                    // Set the time to the max:
                    let new_time = max(&timeline_stats.per_time);
                    if let Some(blueprint_ctx) = blueprint_ctx {
                        blueprint_ctx.set_time(new_time);
                    }
                    self.states
                        .entry(*self.timeline.name())
                        .or_insert_with(|| TimeState::new(new_time))
                        .time = new_time.into();
                }
            }
        }
    }

    fn step_time_back(
        &mut self,
        times_per_timeline: &TimesPerTimeline,
        blueprint_ctx: Option<&impl TimeBlueprintExt>,
    ) {
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
            if let Some(ctx) = blueprint_ctx {
                ctx.set_time(new_time.floor());
            }

            if let Some(state) = self.states.get_mut(self.timeline.name()) {
                state.time = new_time;
            }
        }
    }

    fn step_time_fwd(
        &mut self,
        times_per_timeline: &TimesPerTimeline,
        blueprint_ctx: Option<&impl TimeBlueprintExt>,
    ) {
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
            if let Some(ctx) = blueprint_ctx {
                ctx.set_time(new_time.floor());
            }

            if let Some(state) = self.states.get_mut(self.timeline.name()) {
                state.time = new_time;
            }
        }
    }

    fn pause(&mut self) {
        self.playing = false;
        if let Some(state) = self.states.get_mut(self.timeline.name()) {
            state.last_paused_time = Some(state.time);
        }
    }

    fn toggle_play_pause(
        &mut self,
        times_per_timeline: &TimesPerTimeline,
        blueprint_ctx: Option<&impl BlueprintContext>,
    ) {
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
            if let Some(stats) = times_per_timeline.get(self.timeline.name())
                && let Some(state) = self.states.get_mut(self.timeline.name())
                && max(&stats.per_time) <= state.time
            {
                let new_time = min(&stats.per_time);
                if let Some(blueprint_ctx) = blueprint_ctx {
                    blueprint_ctx.set_time(new_time);
                }
                state.time = new_time.into();
                self.playing = true;
                self.following = false;
                return;
            }

            if self.following {
                self.set_play_state(times_per_timeline, PlayState::Following, blueprint_ctx);
            } else {
                self.set_play_state(times_per_timeline, PlayState::Playing, blueprint_ctx);
            }
        }
    }

    /// playback speed
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// playback fps
    pub fn fps(&self) -> Option<f32> {
        self.states
            .get(self.timeline().name())
            .map(|state| state.fps)
    }

    /// Make sure the selected timeline is a valid one
    fn select_valid_timeline(&mut self, times_per_timeline: &TimesPerTimeline) {
        fn is_timeline_valid(selected: &Timeline, times_per_timeline: &TimesPerTimeline) -> bool {
            for timeline in times_per_timeline.timelines() {
                if selected == timeline {
                    return true; // it's valid
                }
            }
            false
        }

        let reset_timeline = match &self.timeline {
            // If the timeline is auto refresh it every frame.
            ActiveTimeline::Auto(_) => true,
            // If it's user edited, refresh it if it's invalid.
            ActiveTimeline::UserEdited(timeline) => {
                !is_timeline_valid(timeline, times_per_timeline)
            }
            // If it's pending never automatically refresh it.
            ActiveTimeline::Pending(timeline) => {
                // If the pending timeline is valid, it shouldn't be pending anymore.
                if let Some(timeline) = times_per_timeline
                    .timelines()
                    .find(|t| t.name() == timeline.name())
                {
                    self.timeline = ActiveTimeline::UserEdited(*timeline);
                }

                false
            }
        };

        if reset_timeline || matches!(self.timeline, ActiveTimeline::Auto(_)) {
            self.timeline =
                ActiveTimeline::Auto(default_timeline(times_per_timeline.timelines_with_stats()));
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

    /// Mark up a time range as valid.
    ///
    /// Everything outside can still be navigated to, but will be considered potentially lacking some data and therefore "invalid".
    /// Visually, it is outside of the normal time range and shown greyed out.
    ///
    /// If timeline is `None`, this signals that all timelines are considered to be valid entirely.
    //
    // TODO(andreas): The source of truth for this should probably in recording properties as it is just that,
    // a property of the data!
    // However, as of writing it's hard to inject _additional_ properties upon recording loading.
    // For an attempt of modelling this as a serialized recordign property see `andreas/valid-ranges-rec-props` branch.
    fn mark_time_range_valid(
        &mut self,
        timeline: Option<TimelineName>,
        time_range: AbsoluteTimeRange,
    ) {
        if let Some(timeline) = timeline {
            match self.valid_time_ranges.entry(timeline) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(Vec1::new(time_range));
                }
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    let ranges = entry.get_mut();

                    // TODO(andreas): Could do this more efficiently by using binary search to insert and then merging more intelligently.
                    // But we don't expect a lot of ranges, so let's keep it simple.
                    ranges.push(time_range);
                    ranges.sort_by_key(|r| r.min);

                    // Remove overlapping ranges by merging them
                    let mut merged = Vec1::new(*ranges.first());
                    for range in ranges.iter().skip(1) {
                        let last = merged.last_mut();
                        if last.max >= range.min {
                            // Extend existing range instead of adding a new one.
                            *last = AbsoluteTimeRange::new(last.min, last.max.max(range.max));
                        } else {
                            merged.push(*range);
                        }
                    }
                    *ranges = merged;
                }
            }
        } else {
            self.valid_time_ranges.clear();
        }
    }

    /// Returns the valid time ranges for a given timeline.
    ///
    /// Ranges are guaranteed to be non-overlapping and sorted by their min.
    ///
    /// If everything is valid, returns a single `AbsoluteTimeRange::EVERYTHING)`.
    ///
    /// See also [`Self.mark_time_range_valid`].
    pub fn valid_time_ranges_for(&self, timeline: TimelineName) -> Vec1<AbsoluteTimeRange> {
        self.valid_time_ranges
            .get(&timeline)
            .cloned()
            .unwrap_or_else(|| Vec1::new(AbsoluteTimeRange::EVERYTHING))
    }

    /// The maximum extent of the valid time ranges into the past and future.
    ///
    /// There may be gaps in validity of this range.
    ///
    /// See also [`Self.mark_time_range_valid`].
    pub fn max_valid_range_for(&self, timeline: TimelineName) -> AbsoluteTimeRange {
        self.valid_time_ranges
            .get(&timeline)
            .map(|ranges| AbsoluteTimeRange::new(ranges.first().min, ranges.last().max))
            .unwrap_or(AbsoluteTimeRange::EVERYTHING)
    }

    /// The current time.
    pub fn time(&self) -> Option<TimeReal> {
        self.states
            .get(self.timeline().name())
            .map(|state| state.time)
    }

    pub fn last_paused_time(&self) -> Option<TimeReal> {
        if matches!(self.play_state(), PlayState::Paused) {
            self.time()
        } else {
            self.states
                .get(self.timeline().name())
                .and_then(|state| state.last_paused_time)
        }
    }

    /// The current time & timeline.
    pub fn time_cell(&self) -> Option<TimeCell> {
        self.time()
            .map(|t| TimeCell::new(self.timeline().typ(), t.floor().as_i64()))
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

    /// The current loop range, if selection looping is turned on.
    pub fn active_loop_selection(&self) -> Option<AbsoluteTimeRangeF> {
        if self.looping == Looping::Selection {
            self.states.get(self.timeline().name())?.loop_selection
        } else {
            None
        }
    }

    /// The full range of times for the current timeline, skipping times outside of the valid data ranges
    /// at the start and end.
    fn full_valid_range(&self, times_per_timeline: &TimesPerTimeline) -> Option<AbsoluteTimeRange> {
        times_per_timeline.get(self.timeline().name()).map(|stats| {
            let data_range = range(&stats.per_time);
            let max_valid_range_for = self.max_valid_range_for(*self.timeline().name());
            AbsoluteTimeRange::new(
                data_range.min.max(max_valid_range_for.min),
                data_range.max.min(max_valid_range_for.max),
            )
        })
    }

    /// The selected slice of time that is called the "loop selection".
    ///
    /// This can still return `Some` even if looping is currently off.
    pub fn loop_selection(&self) -> Option<AbsoluteTimeRangeF> {
        self.states.get(self.timeline().name())?.loop_selection
    }

    // /// Set the current loop selection without enabling looping.
    // pub fn set_loop_selection(&mut self, selection: AbsoluteTimeRangeF) {
    //     self.states
    //         .entry(*self.timeline.name())
    //         .or_insert_with(|| TimeStateEntry::new(selection.min))
    //         .current
    //         .loop_selection = Some(selection);
    // }

    // /// Remove the current loop selection.
    // pub fn remove_loop_selection(&mut self) {
    //     if let Some(state) = self.states.get_mut(self.timeline.name()) {
    //         state.current.loop_selection = None;
    //     }
    //     if self.looping() == Looping::Selection {
    //         self.set_looping(Looping::Off);
    //     }
    // }

    /// Is the current time in the selection range (if any), or at the current time mark?
    pub fn is_time_selected(&self, timeline: &TimelineName, needle: TimeInt) -> bool {
        if timeline != self.timeline().name() {
            return false;
        }

        if let Some(state) = self.states.get(self.timeline().name()) {
            state.time.floor() == needle
        } else {
            false
        }
    }

    /// Is the active timeline pending?
    pub fn is_pending(&self) -> bool {
        matches!(self.timeline, ActiveTimeline::Pending(_))
    }

    pub fn time_for_timeline(&self, timeline: TimelineName) -> Option<TimeReal> {
        self.states.get(&timeline).map(|state| state.time)
    }

    // /// Set the time.
    // ///
    // /// This does not affect the time stored in blueprints.
    // fn set_time(&mut self, time: TimeReal) {
    //     self.states
    //         .entry(*self.timeline.name())
    //         .or_insert_with(|| TimeStateEntry::new(time))
    //         .current
    //         .time = time;
    // }

    /// The range of time we are currently zoomed in on.
    pub fn time_view(&self) -> Option<TimeView> {
        self.states
            .get(self.timeline().name())
            .and_then(|state| state.view)
    }

    // /// The range of time we are currently zoomed in on.
    // pub fn set_time_view(&mut self, view: TimeView) {
    //     self.states
    //         .entry(*self.timeline.name())
    //         .or_insert_with(|| TimeStateEntry::new(view.min))
    //         .current
    //         .view = Some(view);
    // }

    // /// The range of time we are currently zoomed in on.
    // pub fn reset_time_view(&mut self) {
    //     if let Some(state) = self.states.get_mut(self.timeline.name()) {
    //         state.current.view = None;
    //     }
    // }
}

fn min(values: &TimeCounts) -> TimeInt {
    *values.keys().next().unwrap_or(&TimeInt::MIN)
}

fn max(values: &TimeCounts) -> TimeInt {
    *values.keys().next_back().unwrap_or(&TimeInt::MIN)
}

fn range(values: &TimeCounts) -> AbsoluteTimeRange {
    AbsoluteTimeRange::new(min(values), max(values))
}

/// Pick the timeline that should be the default, by number of elements and prioritizing user-defined ones.
fn default_timeline<'a>(timelines: impl IntoIterator<Item = &'a TimelineStats>) -> Timeline {
    re_tracing::profile_function!();

    // Helper function that acts as a tie-breaker.
    fn timeline_priority(timeline: &Timeline) -> u8 {
        match timeline {
            t if *t == Timeline::log_tick() => 0, // lowest priority
            t if *t == Timeline::log_time() => 1, // medium priority
            _ => 2,                               // user-defined, highest priority
        }
    }
    let most_events = timelines.into_iter().max_by(|a, b| {
        a.num_events()
            .cmp(&b.num_events())
            .then_with(|| timeline_priority(&a.timeline).cmp(&timeline_priority(&b.timeline)))
    });

    if let Some(most_events) = most_events {
        most_events.timeline
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
    loop_range: &AbsoluteTimeRangeF,
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
    loop_range: &AbsoluteTimeRangeF,
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

    fn with_events(timeline: Timeline, num: u64) -> TimelineStats {
        TimelineStats {
            timeline,
            // Dummy `TimeInt` because were only interested in the counts.
            per_time: std::iter::once((TimeInt::ZERO, num)).collect(),
            total_count: num,
        }
    }

    #[test]
    fn test_default_timeline() {
        let log_time = with_events(Timeline::log_time(), 42);
        let log_tick = with_events(Timeline::log_tick(), 42);
        let custom_timeline0 = with_events(Timeline::new("my_timeline0", TimeType::DurationNs), 42);
        let custom_timeline1 = with_events(Timeline::new("my_timeline1", TimeType::DurationNs), 43);

        assert_eq!(default_timeline([]), log_time.timeline);
        assert_eq!(default_timeline([&log_tick]), log_tick.timeline);
        assert_eq!(default_timeline([&log_time]), log_time.timeline);
        assert_eq!(default_timeline([&log_time, &log_tick]), log_time.timeline);
        assert_eq!(
            default_timeline([&log_time, &log_tick, &custom_timeline0]),
            custom_timeline0.timeline
        );
        assert_eq!(
            default_timeline([&custom_timeline0, &log_time, &log_tick]),
            custom_timeline0.timeline
        );
        assert_eq!(
            default_timeline([&log_time, &custom_timeline0, &log_tick]),
            custom_timeline0.timeline
        );
        assert_eq!(
            default_timeline([&custom_timeline0, &log_time]),
            custom_timeline0.timeline
        );
        assert_eq!(
            default_timeline([&custom_timeline0, &log_tick]),
            custom_timeline0.timeline
        );
        assert_eq!(
            default_timeline([&log_time, &custom_timeline0]),
            custom_timeline0.timeline
        );
        assert_eq!(
            default_timeline([&log_tick, &custom_timeline0]),
            custom_timeline0.timeline
        );

        assert_eq!(
            default_timeline([&custom_timeline0, &custom_timeline1]),
            custom_timeline1.timeline
        );
        assert_eq!(
            default_timeline([&custom_timeline0]),
            custom_timeline0.timeline
        );
    }

    #[test]
    fn test_valid_ranges() {
        let mut time_control = TimeControl::default();
        let timeline = TimelineName::new("test");

        // Test default behavior - everything should be valid
        assert_eq!(
            time_control.valid_time_ranges_for(timeline),
            vec1::vec1![AbsoluteTimeRange::EVERYTHING]
        );

        // Test adding a single range
        let range1 = AbsoluteTimeRange::new(TimeInt::new_temporal(100), TimeInt::new_temporal(200));
        time_control.mark_time_range_valid(Some(timeline), range1);
        assert_eq!(
            time_control.valid_time_ranges_for(timeline),
            vec1::vec1![range1]
        );

        // Test adding a non-overlapping range (should remain separate)
        let range2 = AbsoluteTimeRange::new(TimeInt::new_temporal(300), TimeInt::new_temporal(400));
        time_control.mark_time_range_valid(Some(timeline), range2);
        assert_eq!(
            time_control.valid_time_ranges_for(timeline),
            vec1::vec1![range1, range2]
        );

        // Test adding a range extending an existing range (should merge)
        let range3 = AbsoluteTimeRange::new(TimeInt::new_temporal(150), TimeInt::new_temporal(250));
        let range1_extended = AbsoluteTimeRange::new(range1.min, range3.max);
        time_control.mark_time_range_valid(Some(timeline), range3);
        assert_eq!(
            time_control.valid_time_ranges_for(timeline),
            vec1::vec1![range1_extended, range2]
        );

        // Test adding a range that connects two existing ranges
        let range4 = AbsoluteTimeRange::new(TimeInt::new_temporal(150), TimeInt::new_temporal(300));
        let new_combined_range = AbsoluteTimeRange::new(range1_extended.min, range2.max);
        time_control.mark_time_range_valid(Some(timeline), range4);
        assert_eq!(
            time_control.valid_time_ranges_for(timeline),
            vec1::vec1![new_combined_range]
        );

        // Clear everything. back to default behavior.
        time_control.mark_time_range_valid(None, AbsoluteTimeRange::EVERYTHING);
        assert_eq!(
            time_control.valid_time_ranges_for(timeline),
            vec1::vec1![AbsoluteTimeRange::EVERYTHING]
        );
    }
}
