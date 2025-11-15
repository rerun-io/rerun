use std::collections::BTreeMap;

use nohash_hasher::IntMap;
use re_types::blueprint::{
    archetypes::TimePanelBlueprint,
    components::{LoopMode, PlayState},
};
use vec1::Vec1;

use re_chunk::{EntityPath, TimelineName};
use re_entity_db::{TimeHistogram, TimeHistogramPerTimeline};
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

    fn time(&self) -> Option<TimeInt>;

    fn set_timeline(&self, timeline: TimelineName);

    fn timeline(&self) -> Option<TimelineName>;

    /// Replaces the current timeline with the automatic one.
    fn clear_timeline(&self);

    /// Clears the blueprint time cursor, and will instead fall back
    /// to a default one, most likely the one saved in time control's
    /// per timeline state.
    fn clear_time(&self);

    fn set_playback_speed(&self, playback_speed: f64);
    fn playback_speed(&self) -> Option<f64>;

    fn set_fps(&self, fps: f64);
    fn fps(&self) -> Option<f64>;

    fn set_play_state(&self, play_state: PlayState);
    fn play_state(&self) -> Option<PlayState>;

    fn set_loop_mode(&self, loop_mode: LoopMode);
    fn loop_mode(&self) -> Option<LoopMode>;

    fn set_time_selection(&self, time_range: AbsoluteTimeRange);
    fn time_selection(&self) -> Option<AbsoluteTimeRange>;
    fn clear_time_selection(&self);
}

impl<T: BlueprintContext> TimeBlueprintExt for T {
    fn set_time(&self, time: impl Into<TimeInt>) {
        let time: TimeInt = time.into();
        self.save_static_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_time(),
            &re_types::blueprint::components::TimeInt(time.as_i64().into()),
        );
    }

    fn time(&self) -> Option<TimeInt> {
        let (_, time) = self
            .current_blueprint()
            .latest_at_component_quiet::<re_types::blueprint::components::TimeInt>(
                &time_panel_blueprint_entity_path(),
                self.blueprint_query(),
                TimePanelBlueprint::descriptor_time().component,
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

    fn timeline(&self) -> Option<TimelineName> {
        let (_, timeline) = self
            .current_blueprint()
            .latest_at_component_quiet::<re_types::blueprint::components::TimelineName>(
            &time_panel_blueprint_entity_path(),
            self.blueprint_query(),
            TimePanelBlueprint::descriptor_timeline().component,
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

    fn set_playback_speed(&self, playback_speed: f64) {
        self.save_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_playback_speed(),
            &re_types::blueprint::components::PlaybackSpeed(playback_speed.into()),
        );
    }

    fn playback_speed(&self) -> Option<f64> {
        let (_, playback_speed) = self
            .current_blueprint()
            .latest_at_component_quiet::<re_types::blueprint::components::PlaybackSpeed>(
            &time_panel_blueprint_entity_path(),
            self.blueprint_query(),
            TimePanelBlueprint::descriptor_playback_speed().component,
        )?;

        Some(**playback_speed)
    }

    fn set_fps(&self, fps: f64) {
        self.save_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_fps(),
            &re_types::blueprint::components::Fps(fps.into()),
        );
    }

    fn fps(&self) -> Option<f64> {
        let (_, fps) = self
            .current_blueprint()
            .latest_at_component_quiet::<re_types::blueprint::components::Fps>(
                &time_panel_blueprint_entity_path(),
                self.blueprint_query(),
                TimePanelBlueprint::descriptor_fps().component,
            )?;

        Some(**fps)
    }

    fn set_play_state(&self, play_state: PlayState) {
        self.save_static_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_play_state(),
            &play_state,
        );
    }

    fn play_state(&self) -> Option<PlayState> {
        let (_, play_state) = self
            .current_blueprint()
            .latest_at_component_quiet::<PlayState>(
                &time_panel_blueprint_entity_path(),
                self.blueprint_query(),
                TimePanelBlueprint::descriptor_play_state().component,
            )?;

        Some(play_state)
    }

    fn set_loop_mode(&self, loop_mode: LoopMode) {
        self.save_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_loop_mode(),
            &loop_mode,
        );
    }

    fn loop_mode(&self) -> Option<LoopMode> {
        let (_, loop_mode) = self
            .current_blueprint()
            .latest_at_component_quiet::<LoopMode>(
                &time_panel_blueprint_entity_path(),
                self.blueprint_query(),
                TimePanelBlueprint::descriptor_loop_mode().component,
            )?;

        Some(loop_mode)
    }

    fn set_time_selection(&self, time_range: AbsoluteTimeRange) {
        self.save_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_time_selection(),
            &re_types::blueprint::components::AbsoluteTimeRange(
                re_types::datatypes::AbsoluteTimeRange {
                    min: time_range.min.as_i64().into(),
                    max: time_range.max.as_i64().into(),
                },
            ),
        );
    }

    fn time_selection(&self) -> Option<AbsoluteTimeRange> {
        let (_, time_range) = self
            .current_blueprint()
            .latest_at_component_quiet::<re_types::blueprint::components::AbsoluteTimeRange>(
            &time_panel_blueprint_entity_path(),
            self.blueprint_query(),
            TimePanelBlueprint::descriptor_time_selection().component,
        )?;

        Some(AbsoluteTimeRange::new(time_range.min, time_range.max))
    }

    fn clear_time_selection(&self) {
        self.clear_static_blueprint_component(
            time_panel_blueprint_entity_path(),
            TimePanelBlueprint::descriptor_time_selection(),
        );
    }
}

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

impl From<AbsoluteTimeRange> for TimeView {
    fn from(value: AbsoluteTimeRange) -> Self {
        Self {
            min: value.min().into(),
            time_spanned: value.abs_length() as f64,
        }
    }
}

/// A command used to mutate `TimeControl`.
///
/// Can be sent using [`crate::SystemCommand::TimeControlCommands`].
#[derive(Debug)]
pub enum TimeControlCommand {
    HighlightRange(AbsoluteTimeRange),
    ClearHighlightedRange,

    /// Reset the active timeline to instead be automatically assigned.
    ResetActiveTimeline,
    SetActiveTimeline(TimelineName),

    /// Set the current looping state.
    SetLoopMode(LoopMode),
    SetPlayState(PlayState),
    Pause,
    TogglePlayPause,
    StepTimeBack,
    StepTimeForward,

    /// Restart the time cursor to the start.
    ///
    /// Stops any ongoing following.
    Restart,

    /// Set playback speed.
    SetSpeed(f32),

    /// Set playback fps.
    SetFps(f32),

    /// Set the current loop selection without enabling looping.
    SetLoopSelection(AbsoluteTimeRange),

    /// Remove the current loop selection.
    ///
    /// If the current loop mode is selection, turns off looping.
    RemoveLoopSelection,

    /// Sets the current time cursor.
    SetTime(TimeReal),

    /// Set the range of time we are currently zoomed in on.
    SetTimeView(TimeView),

    /// Reset the range of time we are currently zoomed in on.
    ///
    /// The view will instead fall back to the default which is
    /// showing all received data.
    ResetTimeView,

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
    AddValidTimeRange {
        timeline: Option<TimelineName>,
        time_range: AbsoluteTimeRange,
    },
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
///
/// Modifications to this can be done via sending [`TimeControlCommand`]s
/// which are handled at the end of frames.
///
/// The commands write both to this struct and to blueprints when
/// applicable.
#[derive(Clone, PartialEq)]
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

    loop_mode: LoopMode,

    /// Range with special highlight.
    ///
    /// This is used during UI interactions. E.g. to show visual history range that's highlighted.
    pub highlighted_range: Option<AbsoluteTimeRange>,
}

impl Default for TimeControl {
    fn default() -> Self {
        let empty_hist = TimeHistogramPerTimeline::default();
        let empty_timelines = std::collections::BTreeMap::new();
        Self {
            timeline: ActiveTimeline::Auto(default_timeline(&empty_hist, &empty_timelines)),
            states: Default::default(),
            valid_time_ranges: Default::default(),
            playing: true,
            following: true,
            speed: 1.0,
            loop_mode: LoopMode::Off,
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

        this.update_from_blueprint(blueprint_ctx, None, &std::collections::BTreeMap::new());

        this
    }

    /// Read from the time panel blueprint and update the state from that.
    ///
    /// If `time_histogram_per_timeline` is some this will also make sure we are on
    /// a valid timeline.
    pub fn update_from_blueprint(
        &mut self,
        blueprint_ctx: &impl BlueprintContext,
        time_histogram_per_timeline: Option<&TimeHistogramPerTimeline>,
        timelines: &std::collections::BTreeMap<TimelineName, Timeline>,
    ) {
        if let Some(timeline) = blueprint_ctx.timeline() {
            if matches!(self.timeline, ActiveTimeline::Auto(_))
                || timeline.as_str() != self.timeline().name().as_str()
            {
                self.timeline = ActiveTimeline::Pending(Timeline::new_sequence(timeline));
            }
        } else {
            self.timeline = ActiveTimeline::Auto(*self.timeline());
        }

        // Make sure we are on a valid timeline.
        if let Some(time_histogram_per_timeline) = time_histogram_per_timeline {
            self.select_valid_timeline(time_histogram_per_timeline, timelines);
        }

        if let Some(time) = blueprint_ctx.time() {
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
        else if let Some(time_histogram_per_timeline) = time_histogram_per_timeline
            && let Some(full_valid_range) = self.full_valid_range(time_histogram_per_timeline)
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

        if let Some(new_play_state) = blueprint_ctx.play_state()
            && new_play_state != self.play_state()
        {
            self.set_play_state(time_histogram_per_timeline, new_play_state, Some(blueprint_ctx));
        }

        if let Some(new_loop_mode) = blueprint_ctx.loop_mode() {
            self.loop_mode = new_loop_mode;

            if self.loop_mode != LoopMode::Off {
                if self.play_state() == PlayState::Following {
                    self.set_play_state(
                        time_histogram_per_timeline,
                        PlayState::Playing,
                        Some(blueprint_ctx),
                    );
                }

                // It makes no sense with looping and follow.
                self.following = false;
            }
        }

        if let Some(playback_speed) = blueprint_ctx.playback_speed() {
            self.speed = playback_speed as f32;
        }

        let play_state = self.play_state();

        // Update the last paused time if we are paused.
        if let Some(state) = self.states.get_mut(self.timeline.name()) {
            if let Some(fps) = blueprint_ctx.fps() {
                state.fps = fps as f32;
            }

            if let Some(new_time_selection) = blueprint_ctx.time_selection() {
                state.loop_selection = Some(new_time_selection.into());
            }

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
        time_histogram_per_timeline: &TimeHistogramPerTimeline,
        timelines: &std::collections::BTreeMap<TimelineName, Timeline>,
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
            self.update_from_blueprint(blueprint_ctx, Some(time_histogram_per_timeline), timelines);
        } else {
            self.select_valid_timeline(time_histogram_per_timeline, timelines);
        }

        let Some(full_valid_range) = self.full_valid_range(time_histogram_per_timeline) else {
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

                if self.loop_mode == LoopMode::Off && full_valid_range.max() <= state.time {
                    // We've reached the end of the data
                    self.update_time(blueprint_ctx, full_valid_range.max().into());

                    if more_data_is_coming {
                        // then let's wait for it without pausing!
                        return self.apply_state_diff_if_needed(
                            TimeControlResponse::no_repaint(), // ui will wake up when more data arrives
                            should_diff_state,
                            time_histogram_per_timeline,
                            timelines,
                            old_timeline,
                            old_playing,
                            old_state,
                        );
                    } else {
                        self.pause(blueprint_ctx);
                        return TimeControlResponse::no_repaint();
                    }
                }

                let mut new_time = state.time;

                let loop_range = match self.loop_mode {
                    LoopMode::Off => None,
                    LoopMode::Selection => state.loop_selection,
                    LoopMode::All => Some(full_valid_range.into()),
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

        self.apply_state_diff_if_needed(
            TimeControlResponse::new(needs_repaint),
            should_diff_state,
            time_histogram_per_timeline,
            timelines,
            old_timeline,
            old_playing,
            old_state,
        )
    }

    /// Apply state diff to response if needed.
    fn apply_state_diff_if_needed(
        &mut self,
        response: TimeControlResponse,
        should_diff_state: bool,
        time_histogram_per_timeline: &TimeHistogramPerTimeline,
        _timelines: &std::collections::BTreeMap<TimelineName, Timeline>,
        old_timeline: Timeline,
        old_playing: bool,
        old_state: Option<TimeState>,
    ) -> TimeControlResponse {
        let mut response = response;

        if should_diff_state
            && time_histogram_per_timeline
                .get(self.timeline.name())
                .is_some_and(|hist| !hist.is_empty())
        {
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

    pub fn loop_mode(&self) -> LoopMode {
        if self.play_state() == PlayState::Following {
            LoopMode::Off
        } else {
            self.loop_mode
        }
    }

    pub fn handle_time_commands(
        &mut self,
        blueprint_ctx: Option<&impl BlueprintContext>,
        time_histogram_per_timeline: &TimeHistogramPerTimeline,
        timelines: &std::collections::BTreeMap<TimelineName, Timeline>,
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

        for command in commands {
            let needs_repaint =
                self.handle_time_command(blueprint_ctx, time_histogram_per_timeline, timelines, command);

            if needs_repaint == NeedsRepaint::Yes {
                response.needs_repaint = NeedsRepaint::Yes;
            }
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
        time_histogram_per_timeline: &TimeHistogramPerTimeline,
        timelines: &std::collections::BTreeMap<TimelineName, Timeline>,
        command: &TimeControlCommand,
    ) -> NeedsRepaint {
        match command {
            // TODO(isse): Changing the highlighted range should technically cause a repaint. But this causes issues
            // because right now the selection panel wants to clear the range if it's some each frame, and maybe set
            // it again at later point.
            //
            // This is (right now) always caused by hovering on something, so the mouse movement will cause repaints
            // in all current cases.
            //
            // A better fix for this would be to collect all time commands before handling them, and for highlight
            // ranges only keep the last one. And requesting a repaint here again.
            TimeControlCommand::HighlightRange(range) => {
                self.highlighted_range = Some(*range);
                NeedsRepaint::No
            }
            TimeControlCommand::ClearHighlightedRange => {
                self.highlighted_range = None;
                NeedsRepaint::No
            }
            TimeControlCommand::ResetActiveTimeline => {
                if let Some(blueprint_ctx) = blueprint_ctx {
                    blueprint_ctx.clear_timeline();
                }
                self.timeline = ActiveTimeline::Auto(*self.timeline());

                NeedsRepaint::Yes
            }
            TimeControlCommand::SetActiveTimeline(timeline_name) => {
                if let Some(blueprint_ctx) = blueprint_ctx {
                    blueprint_ctx.set_timeline(*timeline_name);
                }

                if let Some(timeline) = timelines.get(timeline_name) {
                    self.timeline = ActiveTimeline::UserEdited(*timeline);
                } else {
                    self.timeline = ActiveTimeline::Pending(Timeline::new_sequence(*timeline_name));
                }

                if let Some(full_valid_range) = self.full_valid_range(time_histogram_per_timeline)
                    && !self.states.contains_key(timeline_name)
                {
                    self.states
                        .insert(*timeline_name, TimeState::new(full_valid_range.min));
                    if let Some(blueprint_ctx) = blueprint_ctx {
                        blueprint_ctx.set_time(full_valid_range.min);
                    }
                }

                NeedsRepaint::Yes
            }
            TimeControlCommand::SetLoopMode(loop_mode) => {
                if self.loop_mode != *loop_mode {
                    if let Some(blueprint_ctx) = blueprint_ctx {
                        blueprint_ctx.set_loop_mode(*loop_mode);
                    }
                    self.loop_mode = *loop_mode;
                    if self.loop_mode != LoopMode::Off {
                        if self.play_state() == PlayState::Following {
                            self.set_play_state(
                                time_histogram_per_timeline,
                                PlayState::Playing,
                                blueprint_ctx,
                            );
                        }

                        // It makes no sense with looping and follow.
                        self.following = false;
                    }

                    NeedsRepaint::Yes
                } else {
                    NeedsRepaint::No
                }
            }
            TimeControlCommand::SetPlayState(play_state) => {
                self.set_play_state(time_histogram_per_timeline, *play_state, blueprint_ctx);

                if self.following {
                    if let Some(blueprint_ctx) = blueprint_ctx {
                        blueprint_ctx.set_loop_mode(LoopMode::Off);
                    }
                    self.loop_mode = LoopMode::Off;
                }

                NeedsRepaint::Yes
            }
            TimeControlCommand::Pause => {
                if self.playing {
                    self.pause(blueprint_ctx);
                    NeedsRepaint::Yes
                } else {
                    NeedsRepaint::No
                }
            }

            TimeControlCommand::TogglePlayPause => {
                self.toggle_play_pause(time_histogram_per_timeline, timelines, blueprint_ctx);

                NeedsRepaint::Yes
            }
            TimeControlCommand::StepTimeBack => {
                self.step_time_back(time_histogram_per_timeline, blueprint_ctx);

                NeedsRepaint::Yes
            }
            TimeControlCommand::StepTimeForward => {
                self.step_time_fwd(time_histogram_per_timeline, blueprint_ctx);

                NeedsRepaint::Yes
            }
            TimeControlCommand::Restart => {
                if let Some(full_valid_range) = self.full_valid_range(time_histogram_per_timeline) {
                    self.following = false;

                    if let Some(blueprint_ctx) = blueprint_ctx {
                        blueprint_ctx.set_time(full_valid_range.min);
                    }

                    if let Some(state) = self.states.get_mut(self.timeline.name()) {
                        state.time = full_valid_range.min.into();
                    }

                    NeedsRepaint::Yes
                } else {
                    NeedsRepaint::No
                }
            }
            TimeControlCommand::SetSpeed(speed) => {
                if *speed != self.speed {
                    self.speed = *speed;

                    if let Some(blueprint_ctx) = blueprint_ctx {
                        blueprint_ctx.set_playback_speed(*speed as f64);
                    }

                    NeedsRepaint::Yes
                } else {
                    NeedsRepaint::No
                }
            }
            TimeControlCommand::SetFps(fps) => {
                if let Some(state) = self.states.get_mut(self.timeline.name())
                    && state.fps != *fps
                {
                    state.fps = *fps;

                    if let Some(blueprint_ctx) = blueprint_ctx {
                        blueprint_ctx.set_fps(*fps as f64);
                    }

                    NeedsRepaint::Yes
                } else {
                    NeedsRepaint::No
                }
            }
            TimeControlCommand::SetLoopSelection(time_range) => {
                if let Some(state) = self.states.get_mut(self.timeline.name()) {
                    if let Some(blueprint_ctx) = blueprint_ctx {
                        blueprint_ctx.set_time_selection(*time_range);
                    }

                    state.loop_selection = Some((*time_range).into());

                    NeedsRepaint::Yes
                } else {
                    NeedsRepaint::No
                }
            }
            TimeControlCommand::RemoveLoopSelection => {
                if let Some(state) = self.states.get_mut(self.timeline.name()) {
                    if let Some(blueprint_ctx) = blueprint_ctx {
                        blueprint_ctx.clear_time_selection();
                    }
                    state.loop_selection = None;

                    NeedsRepaint::Yes
                } else {
                    NeedsRepaint::No
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

                if update_blueprint {
                    NeedsRepaint::Yes
                } else {
                    NeedsRepaint::No
                }
            }
            TimeControlCommand::SetTimeView(time_view) => {
                if let Some(state) = self.states.get_mut(self.timeline.name()) {
                    state.view = Some(*time_view);

                    NeedsRepaint::Yes
                } else {
                    NeedsRepaint::No
                }
            }
            TimeControlCommand::ResetTimeView => {
                if let Some(state) = self.states.get_mut(self.timeline.name()) {
                    state.view = None;

                    NeedsRepaint::Yes
                } else {
                    NeedsRepaint::No
                }
            }
            TimeControlCommand::AddValidTimeRange {
                timeline,
                time_range,
            } => {
                self.mark_time_range_valid(*timeline, *time_range);
                NeedsRepaint::Yes
            }
        }
    }

    /// Updates the current play-state.
    ///
    /// If `blueprint_ctx` is specified this writes to the related
    /// blueprint.
    pub fn set_play_state(
        &mut self,
        time_histogram_per_timeline: &TimeHistogramPerTimeline,
        play_state: PlayState,
        blueprint_ctx: Option<&impl BlueprintContext>,
    ) {
        if let Some(blueprint_ctx) = blueprint_ctx
            && Some(play_state) != blueprint_ctx.play_state()
        {
            blueprint_ctx.set_play_state(play_state);
        }

        match play_state {
            PlayState::Paused => {
                self.playing = false;
            }
            PlayState::Playing => {
                self.playing = true;
                self.following = false;

                // Start from beginning if we are at the end:
                if let Some(hist) = time_histogram_per_timeline.get(self.timeline.name()) {
                    if let Some(state) = self.states.get_mut(self.timeline.name()) {
                        if max(hist) <= state.time {
                            let new_time = min(hist);
                            if let Some(blueprint_ctx) = blueprint_ctx {
                                blueprint_ctx.set_time(new_time);
                            }
                            state.time = new_time.into();
                        }
                    } else {
                        let new_time = min(hist);
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

                if let Some(hist) = time_histogram_per_timeline.get(self.timeline.name()) {
                    // Set the time to the max:
                    let new_time = max(hist);
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
        time_histogram_per_timeline: &TimeHistogramPerTimeline,
        blueprint_ctx: Option<&impl TimeBlueprintExt>,
    ) {
        let Some(hist) = time_histogram_per_timeline.get(self.timeline().name()) else {
            return;
        };

        self.pause(blueprint_ctx);

        if let Some(time) = self.time() {
            let new_time = if let Some(loop_range) = self.active_loop_selection() {
                step_back_time_looped(time, hist, &loop_range)
            } else {
                step_back_time(time, hist).into()
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
        time_histogram_per_timeline: &TimeHistogramPerTimeline,
        blueprint_ctx: Option<&impl TimeBlueprintExt>,
    ) {
        let Some(hist) = time_histogram_per_timeline.get(self.timeline().name()) else {
            return;
        };

        self.pause(blueprint_ctx);

        if let Some(time) = self.time() {
            let new_time = if let Some(loop_range) = self.active_loop_selection() {
                step_fwd_time_looped(time, hist, &loop_range)
            } else {
                step_fwd_time(time, hist).into()
            };
            if let Some(ctx) = blueprint_ctx {
                ctx.set_time(new_time.floor());
            }

            if let Some(state) = self.states.get_mut(self.timeline.name()) {
                state.time = new_time;
            }
        }
    }

    fn pause(&mut self, blueprint_ctx: Option<&impl BlueprintContext>) {
        self.playing = false;
        if let Some(blueprint_ctx) = blueprint_ctx {
            blueprint_ctx.set_play_state(PlayState::Paused);
        }
        if let Some(state) = self.states.get_mut(self.timeline.name()) {
            state.last_paused_time = Some(state.time);
        }
    }

    fn toggle_play_pause(
        &mut self,
        time_histogram_per_timeline: &TimeHistogramPerTimeline,
        _timelines: &std::collections::BTreeMap<TimelineName, Timeline>,
        blueprint_ctx: Option<&impl BlueprintContext>,
    ) {
        if self.playing {
            self.pause(blueprint_ctx);
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
            if let Some(hist) = time_histogram_per_timeline.get(self.timeline.name())
                && let Some(state) = self.states.get_mut(self.timeline.name())
                && max(hist) <= state.time
            {
                let new_time = min(hist);
                if let Some(blueprint_ctx) = blueprint_ctx {
                    blueprint_ctx.set_time(new_time);
                }
                state.time = new_time.into();
                self.playing = true;
                self.following = false;
                return;
            }

            if self.following {
                self.set_play_state(time_histogram_per_timeline, PlayState::Following, blueprint_ctx);
            } else {
                self.set_play_state(time_histogram_per_timeline, PlayState::Playing, blueprint_ctx);
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
    fn select_valid_timeline(
        &mut self,
        time_histogram_per_timeline: &TimeHistogramPerTimeline,
        timelines: &std::collections::BTreeMap<TimelineName, Timeline>,
    ) {
        fn is_timeline_valid(
            selected: &Timeline,
            time_histogram_per_timeline: &TimeHistogramPerTimeline,
        ) -> bool {
            time_histogram_per_timeline.has_timeline(selected.name())
        }

        let reset_timeline = match &self.timeline {
            // If the timeline is auto refresh it every frame.
            ActiveTimeline::Auto(_) => true,
            // If it's user edited, refresh it if it's invalid.
            ActiveTimeline::UserEdited(timeline) => {
                !is_timeline_valid(timeline, time_histogram_per_timeline)
            }
            // If it's pending never automatically refresh it.
            ActiveTimeline::Pending(timeline) => {
                // If the pending timeline is valid, it shouldn't be pending anymore.
                if let Some(timeline) = timelines.get(timeline.name()) {
                    self.timeline = ActiveTimeline::UserEdited(*timeline);
                }

                false
            }
        };

        if reset_timeline || matches!(self.timeline, ActiveTimeline::Auto(_)) {
            self.timeline =
                ActiveTimeline::Auto(default_timeline(time_histogram_per_timeline, timelines));
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
        if self.loop_mode == LoopMode::Selection {
            self.states.get(self.timeline().name())?.loop_selection
        } else {
            None
        }
    }

    /// The full range of times for the current timeline, skipping times outside of the valid data ranges
    /// at the start and end.
    fn full_valid_range(
        &self,
        time_histogram_per_timeline: &TimeHistogramPerTimeline,
    ) -> Option<AbsoluteTimeRange> {
        time_histogram_per_timeline.get(self.timeline().name()).map(|hist| {
            let data_range = range(hist);
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

    /// The range of time we are currently zoomed in on.
    pub fn time_view(&self) -> Option<TimeView> {
        self.states
            .get(self.timeline().name())
            .and_then(|state| state.view)
    }
}

fn min(hist: &TimeHistogram) -> TimeInt {
    hist.min_key()
        .map(TimeInt::new_temporal)
        .unwrap_or(TimeInt::MIN)
}

fn max(hist: &TimeHistogram) -> TimeInt {
    hist.max_key()
        .map(TimeInt::new_temporal)
        .unwrap_or(TimeInt::MIN)
}

fn range(hist: &TimeHistogram) -> AbsoluteTimeRange {
    AbsoluteTimeRange::new(min(hist), max(hist))
}

/// Pick the timeline that should be the default, by number of elements and prioritizing user-defined ones.
fn default_timeline(
    time_histogram_per_timeline: &TimeHistogramPerTimeline,
    timelines: &std::collections::BTreeMap<TimelineName, Timeline>,
) -> Timeline {
    re_tracing::profile_function!();

    // Helper function that acts as a tie-breaker.
    fn timeline_priority(timeline: &Timeline) -> u8 {
        match timeline {
            t if *t == Timeline::log_tick() => 0, // lowest priority
            t if *t == Timeline::log_time() => 1, // medium priority
            _ => 2,                               // user-defined, highest priority
        }
    }
    let most_events = time_histogram_per_timeline
        .iter()
        .filter_map(|(name, hist)| {
            timelines.get(name).map(|timeline| {
                (timeline, hist.total_count())
            })
        })
        .max_by(|(a_timeline, a_count), (b_timeline, b_count)| {
            a_count
                .cmp(b_count)
                .then_with(|| {
                    timeline_priority(a_timeline).cmp(&timeline_priority(b_timeline))
                })
        });

    if let Some((timeline, _)) = most_events {
        *timeline
    } else {
        Timeline::log_time()
    }
}

fn step_fwd_time(time: TimeReal, hist: &TimeHistogram) -> TimeInt {
    hist.next_key_after(time.floor().as_i64())
        .map(TimeInt::new_temporal)
        .unwrap_or_else(|| min(hist))
}

fn step_back_time(time: TimeReal, hist: &TimeHistogram) -> TimeInt {
    hist.prev_key_before(time.ceil().as_i64())
        .map(TimeInt::new_temporal)
        .unwrap_or_else(|| max(hist))
}

fn step_fwd_time_looped(
    time: TimeReal,
    hist: &TimeHistogram,
    loop_range: &AbsoluteTimeRangeF,
) -> TimeReal {
    if time < loop_range.min || loop_range.max <= time {
        loop_range.min
    } else if let Some(next) = hist
        .range(
            (
                std::ops::Bound::Excluded(time.floor().as_i64()),
                std::ops::Bound::Included(loop_range.max.floor().as_i64()),
            ),
            1,
        )
        .next()
        .map(|(r, _)| r.min)
    {
        TimeReal::from(TimeInt::new_temporal(next))
    } else {
        step_fwd_time(time, hist).into()
    }
}

fn step_back_time_looped(
    time: TimeReal,
    hist: &TimeHistogram,
    loop_range: &AbsoluteTimeRangeF,
) -> TimeReal {
    if time <= loop_range.min || loop_range.max < time {
        loop_range.max
    } else {
        // Collect all keys in the range and take the last one
        let mut prev_key = None;
        for (range, _) in hist.range(
            (
                std::ops::Bound::Included(loop_range.min.ceil().as_i64()),
                std::ops::Bound::Excluded(time.ceil().as_i64()),
            ),
            1,
        ) {
            prev_key = Some(range.max);
        }
        if let Some(prev) = prev_key {
            TimeReal::from(TimeInt::new_temporal(prev))
        } else {
            step_back_time(time, hist).into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data(timelines: &[Timeline], counts: &[u64]) -> (TimeHistogramPerTimeline, std::collections::BTreeMap<TimelineName, Timeline>) {
        let mut hist_per_timeline = TimeHistogramPerTimeline::default();
        let mut timeline_map = std::collections::BTreeMap::new();

        for (timeline, &count) in timelines.iter().zip(counts.iter()) {
            timeline_map.insert(*timeline.name(), *timeline);
            // Add count events at time 0 to simulate the count
            // Use the add method which takes (TimelineName, &[i64]) pairs
            let times: Vec<i64> = vec![0; count as usize];
            hist_per_timeline.add(&[(*timeline.name(), &times)], 1);
        }

        (hist_per_timeline, timeline_map)
    }

    #[test]
    fn test_default_timeline() {
        let log_time = Timeline::log_time();
        let log_tick = Timeline::log_tick();
        let custom_timeline0 = Timeline::new("my_timeline0", TimeType::DurationNs);
        let custom_timeline1 = Timeline::new("my_timeline1", TimeType::DurationNs);

        // Empty case
        let (empty_hist, empty_timelines) = create_test_data(&[], &[]);
        assert_eq!(default_timeline(&empty_hist, &empty_timelines), log_time);

        // Single timeline cases
        let (hist_tick, timelines_tick) = create_test_data(&[log_tick], &[42]);
        assert_eq!(default_timeline(&hist_tick, &timelines_tick), log_tick);

        let (hist_time, timelines_time) = create_test_data(&[log_time], &[42]);
        assert_eq!(default_timeline(&hist_time, &timelines_time), log_time);

        // Multiple timelines - log_time should win over log_tick when counts are equal
        let (hist_both, timelines_both) = create_test_data(&[log_time, log_tick], &[42, 42]);
        assert_eq!(default_timeline(&hist_both, &timelines_both), log_time);

        // Custom timeline should win over both log_time and log_tick when counts are equal
        let (hist_custom0, timelines_custom0) = create_test_data(&[log_time, log_tick, custom_timeline0], &[42, 42, 42]);
        assert_eq!(default_timeline(&hist_custom0, &timelines_custom0), custom_timeline0);

        // Order shouldn't matter
        let (hist_custom0_rev, timelines_custom0_rev) = create_test_data(&[custom_timeline0, log_time, log_tick], &[42, 42, 42]);
        assert_eq!(default_timeline(&hist_custom0_rev, &timelines_custom0_rev), custom_timeline0);

        let (hist_custom0_mid, timelines_custom0_mid) = create_test_data(&[log_time, custom_timeline0, log_tick], &[42, 42, 42]);
        assert_eq!(default_timeline(&hist_custom0_mid, &timelines_custom0_mid), custom_timeline0);

        // Custom timelines with different counts - higher count wins
        let (hist_custom1, timelines_custom1) = create_test_data(&[custom_timeline0, custom_timeline1], &[42, 43]);
        assert_eq!(default_timeline(&hist_custom1, &timelines_custom1), custom_timeline1);

        // Single custom timeline
        let (hist_single_custom, timelines_single_custom) = create_test_data(&[custom_timeline0], &[42]);
        assert_eq!(default_timeline(&hist_single_custom, &timelines_single_custom), custom_timeline0);
    }

    #[test]
    fn test_step_fwd_time() {
        use re_log_types::TimeReal;

        let mut hist = TimeHistogram::default();
        hist.increment(10, 1);
        hist.increment(20, 1);
        hist.increment(30, 1);

        // Step forward from before first key
        assert_eq!(step_fwd_time(TimeReal::from(5), &hist), TimeInt::new_temporal(10));

        // Step forward from middle
        assert_eq!(step_fwd_time(TimeReal::from(15), &hist), TimeInt::new_temporal(20));

        // Step forward from last key (wraps around)
        assert_eq!(step_fwd_time(TimeReal::from(30), &hist), TimeInt::new_temporal(10));

        // Step forward from after last key (wraps around)
        assert_eq!(step_fwd_time(TimeReal::from(35), &hist), TimeInt::new_temporal(10));

        // Empty histogram
        let empty_hist = TimeHistogram::default();
        assert_eq!(step_fwd_time(TimeReal::from(10), &empty_hist), TimeInt::MIN);
    }

    #[test]
    fn test_step_back_time() {
        use re_log_types::TimeReal;

        let mut hist = TimeHistogram::default();
        hist.increment(10, 1);
        hist.increment(20, 1);
        hist.increment(30, 1);

        // Step back from after last key
        assert_eq!(step_back_time(TimeReal::from(35), &hist), TimeInt::new_temporal(30));

        // Step back from middle
        assert_eq!(step_back_time(TimeReal::from(25), &hist), TimeInt::new_temporal(20));

        // Step back from first key (wraps around)
        assert_eq!(step_back_time(TimeReal::from(10), &hist), TimeInt::new_temporal(30));

        // Step back from before first key (wraps around)
        assert_eq!(step_back_time(TimeReal::from(5), &hist), TimeInt::new_temporal(30));

        // Empty histogram
        let empty_hist = TimeHistogram::default();
        assert_eq!(step_back_time(TimeReal::from(10), &empty_hist), TimeInt::MIN);
    }

    #[test]
    fn test_step_fwd_time_looped() {
        use re_log_types::{AbsoluteTimeRangeF, TimeReal};

        let mut hist = TimeHistogram::default();
        hist.increment(10, 1);
        hist.increment(20, 1);
        hist.increment(30, 1);

        let loop_range = AbsoluteTimeRangeF::new(15.0, 25.0);

        // Before loop range - should jump to start
        assert_eq!(step_fwd_time_looped(TimeReal::from(5), &hist, &loop_range), TimeReal::from(15.0));

        // In loop range - should step to next key
        assert_eq!(step_fwd_time_looped(TimeReal::from(15), &hist, &loop_range), TimeReal::from(20));

        // At end of loop range - should wrap to start
        assert_eq!(step_fwd_time_looped(TimeReal::from(25), &hist, &loop_range), TimeReal::from(15.0));

        // After loop range - should jump to start
        assert_eq!(step_fwd_time_looped(TimeReal::from(35), &hist, &loop_range), TimeReal::from(15.0));
    }

    #[test]
    fn test_step_back_time_looped() {
        use re_log_types::{AbsoluteTimeRangeF, TimeReal};

        let mut hist = TimeHistogram::default();
        hist.increment(10, 1);
        hist.increment(20, 1);
        hist.increment(30, 1);

        let loop_range = AbsoluteTimeRangeF::new(15.0, 25.0);

        // Before loop range - should jump to end
        assert_eq!(step_back_time_looped(TimeReal::from(5), &hist, &loop_range), TimeReal::from(25.0));

        // In loop range - should step to previous key
        assert_eq!(step_back_time_looped(TimeReal::from(25), &hist, &loop_range), TimeReal::from(20));

        // At start of loop range - should wrap to end
        assert_eq!(step_back_time_looped(TimeReal::from(15), &hist, &loop_range), TimeReal::from(25.0));

        // After loop range - should jump to end
        assert_eq!(step_back_time_looped(TimeReal::from(35), &hist, &loop_range), TimeReal::from(25.0));
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
