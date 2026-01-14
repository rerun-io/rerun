use std::collections::BTreeMap;

use re_chunk::{EntityPath, TimelineName};
use re_entity_db::{TimeHistogram, TimeHistogramPerTimeline};
use re_log_types::{
    AbsoluteTimeRange, AbsoluteTimeRangeF, Duration, TimeCell, TimeInt, TimeReal, TimeType,
    Timeline,
};
use re_sdk_types::blueprint::archetypes::TimePanelBlueprint;
use re_sdk_types::blueprint::components::{LoopMode, PlayState};

use crate::NeedsRepaint;
use crate::blueprint_helpers::BlueprintContext;

pub const TIME_PANEL_PATH: &str = "time_panel";

pub fn time_panel_blueprint_entity_path() -> EntityPath {
    TIME_PANEL_PATH.into()
}

/// Helper trait to write time panel related blueprint components.
trait TimeBlueprintExt {
    fn set_timeline(&self, timeline: TimelineName);

    fn timeline(&self) -> Option<TimelineName>;

    /// Replaces the current timeline with the automatic one.
    fn clear_timeline(&self);

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
    fn set_timeline(&self, timeline: TimelineName) {
        self.save_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_timeline(),
            &re_sdk_types::blueprint::components::TimelineName::from(timeline.as_str()),
        );
    }

    fn timeline(&self) -> Option<TimelineName> {
        let (_, timeline) = self
            .current_blueprint()
            .latest_at_component_quiet::<re_sdk_types::blueprint::components::TimelineName>(
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

    fn set_playback_speed(&self, playback_speed: f64) {
        self.save_blueprint_component(
            time_panel_blueprint_entity_path(),
            &TimePanelBlueprint::descriptor_playback_speed(),
            &re_sdk_types::blueprint::components::PlaybackSpeed(playback_speed.into()),
        );
    }

    fn playback_speed(&self) -> Option<f64> {
        let (_, playback_speed) = self
            .current_blueprint()
            .latest_at_component_quiet::<re_sdk_types::blueprint::components::PlaybackSpeed>(
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
            &re_sdk_types::blueprint::components::Fps(fps.into()),
        );
    }

    fn fps(&self) -> Option<f64> {
        let (_, fps) = self
            .current_blueprint()
            .latest_at_component_quiet::<re_sdk_types::blueprint::components::Fps>(
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
            &re_sdk_types::blueprint::components::AbsoluteTimeRange(
                re_sdk_types::datatypes::AbsoluteTimeRange {
                    min: time_range.min.as_i64().into(),
                    max: time_range.max.as_i64().into(),
                },
            ),
        );
    }

    fn time_selection(&self) -> Option<AbsoluteTimeRange> {
        let (_, time_range) = self
            .current_blueprint()
            .latest_at_component_quiet::<re_sdk_types::blueprint::components::AbsoluteTimeRange>(
            &time_panel_blueprint_entity_path(),
            self.blueprint_query(),
            TimePanelBlueprint::descriptor_time_selection().component,
        )?;

        Some(AbsoluteTimeRange::new(time_range.min, time_range.max))
    }

    fn clear_time_selection(&self) {
        self.clear_blueprint_component(
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
    MoveBySeconds(f64),
    MoveBeginning,
    MoveEnd,

    /// Restart the time cursor to the start.
    ///
    /// Stops any ongoing following.
    Restart,

    /// Set playback speed.
    SetSpeed(f32),

    /// Set playback fps.
    SetFps(f32),

    /// Set the current time selection without enabling looping.
    SetTimeSelection(AbsoluteTimeRange),

    /// Remove the current time selection.
    ///
    /// If the current loop mode is selection, turns off looping.
    RemoveTimeSelection,

    /// Sets the current time cursor.
    SetTime(TimeReal),

    /// Set the range of time we are currently zoomed in on.
    SetTimeView(TimeView),

    /// Reset the range of time we are currently zoomed in on.
    ///
    /// The view will instead fall back to the default which is
    /// showing all received data.
    ResetTimeView,
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
    time_selection: Option<AbsoluteTimeRangeF>,

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
            time_selection: Default::default(),
            view: None,
        }
    }
}

// TODO(andreas): This should be a blueprint property and follow the usual rules of how we determine fallbacks.
#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq, Debug)]
enum ActiveTimeline {
    Auto(Timeline),
    UserEdited(Timeline),
    Pending(TimelineName),
}

impl ActiveTimeline {
    pub fn name(&self) -> &TimelineName {
        match self {
            Self::Auto(timeline) | Self::UserEdited(timeline) => timeline.name(),
            Self::Pending(timeline_name) => timeline_name,
        }
    }

    pub fn timeline(&self) -> Option<&Timeline> {
        match self {
            Self::Auto(timeline) | Self::UserEdited(timeline) => Some(timeline),
            Self::Pending(_) => None,
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

    /// If true, we are either in [`PlayState::Playing`] or [`PlayState::Following`].
    playing: bool,

    /// If true, we are in "follow" mode (see [`PlayState::Following`]).
    /// Ignored when [`Self::playing`] is `false`.
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
        Self {
            timeline: ActiveTimeline::Auto(default_timeline([])),
            states: Default::default(),
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

        this.update_from_blueprint(blueprint_ctx, None);

        this
    }

    /// Read from the time panel blueprint and update the state from that.
    ///
    /// If `timeline_histograms` is some this will also make sure we are on
    /// a valid timeline.
    pub fn update_from_blueprint(
        &mut self,
        blueprint_ctx: &impl BlueprintContext,
        timeline_histograms: Option<&TimeHistogramPerTimeline>,
    ) {
        if let Some(timeline) = blueprint_ctx.timeline() {
            if matches!(self.timeline, ActiveTimeline::Auto(_))
                || timeline.as_str() != self.timeline_name().as_str()
            {
                self.timeline = ActiveTimeline::Pending(timeline);
            }
        } else if let Some(timeline) = self.timeline() {
            self.timeline = ActiveTimeline::Auto(*timeline);
        }

        let old_timeline = *self.timeline_name();
        // Make sure we are on a valid timeline.
        if let Some(timeline_histograms) = timeline_histograms {
            self.select_valid_timeline(timeline_histograms);
        }
        // If we are on a new timeline insert that new state at the start. Or end if we're following.
        else if let Some(timeline_histograms) = timeline_histograms
            && let Some(full_range) = self.full_range(timeline_histograms)
        {
            self.states.insert(
                *self.timeline_name(),
                TimeState::new(if self.following {
                    full_range.max
                } else {
                    full_range.min
                }),
            );
        }

        if let Some(new_play_state) = blueprint_ctx.play_state()
            && new_play_state != self.play_state()
        {
            self.set_play_state(timeline_histograms, new_play_state, Some(blueprint_ctx));
        }

        if let Some(new_loop_mode) = blueprint_ctx.loop_mode() {
            self.loop_mode = new_loop_mode;

            if self.loop_mode != LoopMode::Off {
                if self.play_state() == PlayState::Following {
                    self.set_play_state(
                        timeline_histograms,
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
        let timeline = *self.timeline_name();
        if let Some(state) = self.states.get_mut(&timeline) {
            if let Some(fps) = blueprint_ctx.fps() {
                state.fps = fps as f32;
            }

            let bp_loop_section = blueprint_ctx.time_selection();
            // If we've switched timeline, use the new timeline's cached time selection.
            if old_timeline != timeline {
                match state.time_selection {
                    Some(selection) => blueprint_ctx.set_time_selection(selection.to_int()),
                    None => {
                        blueprint_ctx.clear_time_selection();
                    }
                }
            } else {
                state.time_selection = bp_loop_section.map(|r| r.into());
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
    fn update_time(&mut self, time: TimeReal) {
        self.states
            .entry(*self.timeline_name())
            .or_insert_with(|| TimeState::new(time))
            .time = time;
    }

    /// Create [`TimeControlCommand`]s to move the time forward (if playing), and perhaps pause if
    /// we've reached the end.
    #[expect(clippy::fn_params_excessive_bools)] // TODO(emilk): remove bool parameters
    pub fn update(
        &mut self,
        timeline_histograms: &TimeHistogramPerTimeline,
        stable_dt: f32,
        more_data_is_coming: bool,
        should_diff_state: bool,
        blueprint_ctx: Option<&impl BlueprintContext>,
    ) -> TimeControlResponse {
        let (old_playing, old_timeline, old_state) = (
            self.playing,
            self.timeline().copied(),
            self.states.get(self.timeline_name()).copied(),
        );

        if let Some(blueprint_ctx) = blueprint_ctx {
            self.update_from_blueprint(blueprint_ctx, Some(timeline_histograms));
        } else {
            self.select_valid_timeline(timeline_histograms);
        }

        let Some(full_range) = self.full_range(timeline_histograms) else {
            return TimeControlResponse::no_repaint(); // we have no data on this timeline yet, so bail
        };

        let needs_repaint = match self.play_state() {
            PlayState::Paused => {
                // It's possible that the playback is paused because e.g. it reached its end, but
                // then the user decides to switch timelines.
                // When they do so, it might be the case that they switch to a timeline they've
                // never interacted with before, in which case we don't even have a time state yet.
                let state = self.states.entry(*self.timeline_name()).or_insert_with(|| {
                    TimeState::new(if self.following {
                        full_range.max()
                    } else {
                        full_range.min()
                    })
                });

                state.last_paused_time = Some(state.time);
                NeedsRepaint::No
            }
            PlayState::Playing => {
                let dt = stable_dt.min(0.1) * self.speed;

                let state = self
                    .states
                    .entry(*self.timeline_name())
                    .or_insert_with(|| TimeState::new(full_range.min()));

                if self.loop_mode == LoopMode::Off && full_range.max() <= state.time {
                    // We've reached the end of the data
                    self.update_time(full_range.max().into());

                    if more_data_is_coming {
                        // then let's wait for it without pausing!
                        return self.apply_state_diff_if_needed(
                            TimeControlResponse::no_repaint(), // ui will wake up when more data arrives
                            should_diff_state,
                            timeline_histograms,
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
                    LoopMode::Selection => state.time_selection,
                    LoopMode::All => Some(full_range.into()),
                };

                match self.timeline.timeline().map(|t| t.typ()) {
                    Some(TimeType::Sequence) => {
                        new_time += TimeReal::from(state.fps * dt);
                    }
                    Some(TimeType::DurationNs | TimeType::TimestampNs) => {
                        new_time += TimeReal::from(Duration::from_secs(dt));
                    }
                    None => {}
                }

                if let Some(loop_range) = loop_range
                    && loop_range.max < new_time
                {
                    new_time = loop_range.min; // loop!
                }

                self.update_time(new_time);

                NeedsRepaint::Yes
            }
            PlayState::Following => {
                // Set the time to the max:
                self.update_time(full_range.max().into());

                NeedsRepaint::No // no need for request_repaint - we already repaint when new data arrives
            }
        };

        self.apply_state_diff_if_needed(
            TimeControlResponse::new(needs_repaint),
            should_diff_state,
            timeline_histograms,
            old_timeline,
            old_playing,
            old_state,
        )
    }

    /// Apply state diff to response if needed.
    #[expect(clippy::fn_params_excessive_bools)] // TODO(emilk): remove bool parameters
    fn apply_state_diff_if_needed(
        &mut self,
        response: TimeControlResponse,
        should_diff_state: bool,
        timeline_histograms: &TimeHistogramPerTimeline,
        old_timeline: Option<Timeline>,
        old_playing: bool,
        old_state: Option<TimeState>,
    ) -> TimeControlResponse {
        let mut response = response;

        if should_diff_state
            && timeline_histograms
                .get(self.timeline_name())
                .is_some_and(|stats| !stats.is_empty())
        {
            self.diff_with(&mut response, old_timeline, old_playing, old_state);
        }

        response
    }

    /// Handle updating last frame state and trigger callbacks on changes.
    fn diff_with(
        &mut self,
        response: &mut TimeControlResponse,
        old_timeline: Option<Timeline>,
        old_playing: bool,
        old_state: Option<TimeState>,
    ) {
        if old_playing != self.playing {
            response.playing_change = Some(self.playing);
        }

        if old_timeline != self.timeline().copied() {
            let time = self
                .time_for_timeline(*self.timeline_name())
                .unwrap_or(TimeReal::MIN);

            response.timeline_change = self.timeline().map(|t| (*t, time));
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
        timeline_histograms: &TimeHistogramPerTimeline,
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
            self.timeline().copied(),
            self.states.get(self.timeline_name()).copied(),
        );

        for command in commands {
            let needs_repaint =
                self.handle_time_command(blueprint_ctx, timeline_histograms, command);

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
        timeline_histograms: &TimeHistogramPerTimeline,
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
                if let Some(timeline) = self
                    .timeline()
                    .copied()
                    .or_else(|| timeline_histograms.timelines().next())
                {
                    self.timeline = ActiveTimeline::Auto(timeline);
                }
                self.select_valid_timeline(timeline_histograms);

                NeedsRepaint::Yes
            }
            TimeControlCommand::SetActiveTimeline(timeline_name) => {
                if let Some(blueprint_ctx) = blueprint_ctx {
                    blueprint_ctx.set_timeline(*timeline_name);
                }

                if let Some(stats) = timeline_histograms.get(timeline_name) {
                    self.timeline = ActiveTimeline::UserEdited(stats.timeline());
                } else {
                    self.timeline = ActiveTimeline::Pending(*timeline_name);
                }

                if let Some(state) = self.states.get(timeline_name) {
                    // Use the new timeline's cached time selection.
                    if let Some(blueprint_ctx) = blueprint_ctx {
                        match state.time_selection {
                            Some(selection) => blueprint_ctx.set_time_selection(selection.to_int()),
                            None => blueprint_ctx.clear_time_selection(),
                        }
                    }
                } else if let Some(full_range) = self.full_range(timeline_histograms) {
                    self.states
                        .insert(*timeline_name, TimeState::new(full_range.min));
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
                                Some(timeline_histograms),
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
                if self.play_state() != *play_state {
                    self.set_play_state(Some(timeline_histograms), *play_state, blueprint_ctx);

                    if self.following {
                        if let Some(blueprint_ctx) = blueprint_ctx {
                            blueprint_ctx.set_loop_mode(LoopMode::Off);
                        }
                        self.loop_mode = LoopMode::Off;
                    }

                    NeedsRepaint::Yes
                } else {
                    NeedsRepaint::No
                }
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
                self.toggle_play_pause(timeline_histograms, blueprint_ctx);

                NeedsRepaint::Yes
            }
            TimeControlCommand::StepTimeBack => {
                self.step_time_back(timeline_histograms, blueprint_ctx);

                NeedsRepaint::Yes
            }
            TimeControlCommand::StepTimeForward => {
                self.step_time_fwd(timeline_histograms, blueprint_ctx);

                NeedsRepaint::Yes
            }
            TimeControlCommand::MoveBySeconds(seconds) => {
                self.move_by_seconds(timeline_histograms, *seconds);

                NeedsRepaint::Yes
            }
            TimeControlCommand::MoveBeginning => {
                if let Some(full_range) = self.full_range(timeline_histograms) {
                    self.states
                        .entry(*self.timeline_name())
                        .or_insert_with(|| TimeState::new(full_range.min))
                        .time = full_range.min.into();

                    NeedsRepaint::Yes
                } else {
                    NeedsRepaint::No
                }
            }
            TimeControlCommand::MoveEnd => {
                if let Some(full_range) = self.full_range(timeline_histograms) {
                    self.states
                        .entry(*self.timeline_name())
                        .or_insert_with(|| TimeState::new(full_range.max))
                        .time = full_range.max.into();
                    NeedsRepaint::Yes
                } else {
                    NeedsRepaint::No
                }
            }
            TimeControlCommand::Restart => {
                if let Some(full_range) = self.full_range(timeline_histograms) {
                    self.following = false;

                    if let Some(state) = self.states.get_mut(self.timeline.name()) {
                        state.time = full_range.min.into();
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
            TimeControlCommand::SetTimeSelection(time_range) => {
                if let Some(blueprint_ctx) = blueprint_ctx {
                    blueprint_ctx.set_time_selection(*time_range);
                }

                let state = self
                    .states
                    .entry(*self.timeline_name())
                    .or_insert_with(|| TimeState::new(time_range.min));

                let repaint = state.time_selection.map(|r| r.to_int()) != Some(*time_range);

                state.time_selection = Some((*time_range).into());

                if repaint {
                    NeedsRepaint::Yes
                } else {
                    NeedsRepaint::No
                }
            }
            TimeControlCommand::RemoveTimeSelection => {
                if let Some(state) = self.states.get_mut(self.timeline.name()) {
                    if let Some(blueprint_ctx) = blueprint_ctx {
                        blueprint_ctx.clear_time_selection();
                    }
                    state.time_selection = None;
                    if self.loop_mode == LoopMode::Selection {
                        self.loop_mode = LoopMode::Off;

                        if let Some(blueprint_ctx) = blueprint_ctx {
                            blueprint_ctx.set_loop_mode(self.loop_mode);
                        }
                    }

                    NeedsRepaint::Yes
                } else {
                    NeedsRepaint::No
                }
            }
            TimeControlCommand::SetTime(time) => {
                let time_int = time.floor();
                let repaint = self.time_int() != Some(time_int);
                self.states
                    .entry(*self.timeline_name())
                    .or_insert_with(|| TimeState::new(*time))
                    .time = *time;

                if repaint {
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
        }
    }

    /// Updates the current play-state.
    ///
    /// If `blueprint_ctx` is specified this writes to the related
    /// blueprint.
    pub fn set_play_state(
        &mut self,
        timeline_histograms: Option<&TimeHistogramPerTimeline>,
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
                if let Some(timeline_histograms) = timeline_histograms
                    && let Some(histogram) = timeline_histograms.get(self.timeline_name())
                {
                    if let Some(state) = self.states.get_mut(self.timeline.name()) {
                        if histogram.max() <= state.time {
                            let new_time = histogram.min();
                            state.time = new_time.into();
                        }
                    } else {
                        let new_time = histogram.min();
                        self.states
                            .insert(*self.timeline_name(), TimeState::new(new_time));
                    }
                }
            }
            PlayState::Following => {
                self.playing = true;
                self.following = true;

                if let Some(timeline_histograms) = timeline_histograms
                    && let Some(histogram) = timeline_histograms.get(self.timeline_name())
                {
                    // Set the time to the max:
                    let new_time = histogram.max();
                    self.states
                        .entry(*self.timeline_name())
                        .or_insert_with(|| TimeState::new(new_time))
                        .time = new_time.into();
                }
            }
        }
    }

    fn step_time_back(
        &mut self,
        timeline_histograms: &TimeHistogramPerTimeline,
        blueprint_ctx: Option<&impl BlueprintContext>,
    ) {
        let Some(histogram) = timeline_histograms.get(self.timeline.name()) else {
            return;
        };

        self.pause(blueprint_ctx);

        if let Some(time) = self.time() {
            let new_time = if let Some(loop_range) = self.active_loop_selection() {
                histogram.step_back_time_looped(time, &loop_range)
            } else {
                histogram.step_back_time(time).into()
            };

            if let Some(state) = self.states.get_mut(self.timeline.name()) {
                state.time = new_time;
            }
        }
    }

    fn step_time_fwd(
        &mut self,
        timeline_histograms: &TimeHistogramPerTimeline,
        blueprint_ctx: Option<&impl BlueprintContext>,
    ) {
        let Some(stats) = timeline_histograms.get(self.timeline_name()) else {
            return;
        };

        self.pause(blueprint_ctx);

        if let Some(time) = self.time() {
            let new_time = if let Some(loop_range) = self.active_loop_selection() {
                stats.step_fwd_time_looped(time, &loop_range)
            } else {
                stats.step_fwd_time(time).into()
            };

            if let Some(state) = self.states.get_mut(self.timeline.name()) {
                state.time = new_time;
            }
        }
    }

    fn move_by_seconds(&mut self, timeline_histograms: &TimeHistogramPerTimeline, seconds: f64) {
        if let Some(time) = self.time() {
            let mut new_time = match self.time_type() {
                Some(TimeType::Sequence) => time + TimeReal::from(seconds as i64),
                Some(TimeType::DurationNs | TimeType::TimestampNs) => {
                    time + TimeReal::from_secs(seconds)
                }
                None => return,
            };

            let range = self
                .time_selection()
                .or_else(|| self.full_range(timeline_histograms).map(|r| r.into()));
            if let Some(range) = range {
                if time == range.min && new_time < range.min {
                    // jump right to the end
                    new_time = range.max;
                } else if new_time < range.min {
                    // we are right at the end, wrap to the start
                    new_time = range.min;
                } else if time == range.max && new_time > range.max {
                    // jump right to the start
                    new_time = range.min;
                } else if new_time > range.max {
                    // we are right at the start, wrap to the end
                    new_time = range.max;
                }
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
        timeline_histograms: &TimeHistogramPerTimeline,
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
            if let Some(stats) = timeline_histograms.get(self.timeline_name())
                && let Some(state) = self.states.get_mut(self.timeline.name())
                && stats.max() <= state.time
            {
                let new_time = stats.min();
                state.time = new_time.into();
                self.playing = true;
                self.following = false;
                return;
            }

            if self.following {
                self.set_play_state(
                    Some(timeline_histograms),
                    PlayState::Following,
                    blueprint_ctx,
                );
            } else {
                self.set_play_state(Some(timeline_histograms), PlayState::Playing, blueprint_ctx);
            }
        }
    }

    /// playback speed
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// playback fps
    pub fn fps(&self) -> Option<f32> {
        self.states.get(self.timeline_name()).map(|state| state.fps)
    }

    /// Make sure the selected timeline is a valid one
    fn select_valid_timeline(&mut self, timeline_histograms: &TimeHistogramPerTimeline) {
        fn is_timeline_valid(
            selected: &Timeline,
            timeline_histograms: &TimeHistogramPerTimeline,
        ) -> bool {
            for timeline in timeline_histograms.timelines() {
                if selected == &timeline {
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
                !is_timeline_valid(timeline, timeline_histograms)
            }
            // If it's pending never automatically refresh it.
            ActiveTimeline::Pending(timeline) => {
                // If the pending timeline is valid, it shouldn't be pending anymore.
                if let Some(timeline) = timeline_histograms
                    .timelines()
                    .find(|t| t.name() == timeline)
                {
                    self.timeline = ActiveTimeline::UserEdited(timeline);
                }

                false
            }
        };

        if reset_timeline || matches!(self.timeline, ActiveTimeline::Auto(_)) {
            self.timeline =
                ActiveTimeline::Auto(default_timeline(timeline_histograms.histograms()));
        }
    }

    /// The currently selected timeline
    #[inline]
    pub fn timeline(&self) -> Option<&Timeline> {
        self.timeline.timeline()
    }

    pub fn timeline_name(&self) -> &TimelineName {
        self.timeline.name()
    }

    /// The time type of the currently selected timeline
    pub fn time_type(&self) -> Option<TimeType> {
        self.timeline().map(|t| t.typ())
    }

    /// The current time.
    pub fn time(&self) -> Option<TimeReal> {
        self.states
            .get(self.timeline_name())
            .map(|state| state.time)
    }

    pub fn last_paused_time(&self) -> Option<TimeReal> {
        if matches!(self.play_state(), PlayState::Paused) {
            self.time()
        } else {
            self.states
                .get(self.timeline_name())
                .and_then(|state| state.last_paused_time)
        }
    }

    /// The current time & timeline.
    pub fn time_cell(&self) -> Option<TimeCell> {
        let t = self.time()?;
        Some(TimeCell::new(self.time_type()?, t.floor().as_i64()))
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
            *self.timeline_name(),
            self.time().map_or(TimeInt::MAX, |t| t.floor()),
        )
    }

    /// The current loop range, if selection looping is turned on.
    pub fn active_loop_selection(&self) -> Option<AbsoluteTimeRangeF> {
        if self.loop_mode == LoopMode::Selection {
            self.states.get(self.timeline_name())?.time_selection
        } else {
            None
        }
    }

    /// The full range of times for the current timeline, skipping times outside of the valid data ranges
    /// at the start and end.
    fn full_range(
        &self,
        timeline_histograms: &TimeHistogramPerTimeline,
    ) -> Option<AbsoluteTimeRange> {
        timeline_histograms
            .get(self.timeline_name())
            .map(|stats| stats.full_range())
    }

    /// The selected slice of time that is called the "loop selection".
    ///
    /// This can still return `Some` even if looping is currently off.
    pub fn time_selection(&self) -> Option<AbsoluteTimeRangeF> {
        self.states.get(self.timeline_name())?.time_selection
    }

    /// Is the current time in the selection range (if any), or at the current time mark?
    pub fn is_time_selected(&self, timeline: &TimelineName, needle: TimeInt) -> bool {
        if timeline != self.timeline_name() {
            return false;
        }

        if let Some(state) = self.states.get(self.timeline_name()) {
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
            .get(self.timeline_name())
            .and_then(|state| state.view)
    }
}

/// Pick the timeline that should be the default, by number of elements and prioritizing user-defined ones.
fn default_timeline<'a>(timelines: impl IntoIterator<Item = &'a TimeHistogram>) -> Timeline {
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
        a.num_rows()
            .cmp(&b.num_rows())
            .then_with(|| timeline_priority(&a.timeline()).cmp(&timeline_priority(&b.timeline())))
    });

    if let Some(most_events) = most_events {
        most_events.timeline()
    } else {
        Timeline::log_time()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_events(timeline: Timeline, num: u64) -> TimeHistogram {
        let mut stats = TimeHistogram::new(timeline);
        stats.insert(TimeInt::ZERO, num);
        stats
    }

    #[test]
    fn test_default_timeline() {
        let log_time = with_events(Timeline::log_time(), 42);
        let log_tick = with_events(Timeline::log_tick(), 42);
        let custom_timeline0 = with_events(Timeline::new("my_timeline0", TimeType::DurationNs), 42);
        let custom_timeline1 = with_events(Timeline::new("my_timeline1", TimeType::DurationNs), 43);

        assert_eq!(default_timeline([]), log_time.timeline());
        assert_eq!(default_timeline([&log_tick]), log_tick.timeline());
        assert_eq!(default_timeline([&log_time]), log_time.timeline());
        assert_eq!(
            default_timeline([&log_time, &log_tick]),
            log_time.timeline()
        );
        assert_eq!(
            default_timeline([&log_time, &log_tick, &custom_timeline0]),
            custom_timeline0.timeline()
        );
        assert_eq!(
            default_timeline([&custom_timeline0, &log_time, &log_tick]),
            custom_timeline0.timeline()
        );
        assert_eq!(
            default_timeline([&log_time, &custom_timeline0, &log_tick]),
            custom_timeline0.timeline()
        );
        assert_eq!(
            default_timeline([&custom_timeline0, &log_time]),
            custom_timeline0.timeline()
        );
        assert_eq!(
            default_timeline([&custom_timeline0, &log_tick]),
            custom_timeline0.timeline()
        );
        assert_eq!(
            default_timeline([&log_time, &custom_timeline0]),
            custom_timeline0.timeline()
        );
        assert_eq!(
            default_timeline([&log_tick, &custom_timeline0]),
            custom_timeline0.timeline()
        );

        assert_eq!(
            default_timeline([&custom_timeline0, &custom_timeline1]),
            custom_timeline1.timeline()
        );
        assert_eq!(
            default_timeline([&custom_timeline0]),
            custom_timeline0.timeline()
        );
    }
}
