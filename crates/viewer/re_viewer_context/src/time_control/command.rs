use re_chunk::TimelineName;
use re_log_types::{AbsoluteTimeRange, TimeReal, TimeType};
use re_sdk_types::blueprint::components::{LoopMode, PlayState};

use crate::NeedsRepaint;
use crate::blueprint_helpers::BlueprintContext;

use super::blueprint_ext::TimeBlueprintExt as _;
use super::{TimeControl, TimeControlDb, TimeControlResponse, TimeState, TimeView};

/// Direction for time movement commands.
#[derive(Debug, Clone, Copy)]
pub enum MoveDirection {
    Back,
    Forward,
}

/// Speed for time movement commands.
#[derive(Debug, Clone, Copy)]
pub enum MoveSpeed {
    Normal,
    Fast,
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
    Move {
        direction: MoveDirection,
        speed: MoveSpeed,
    },
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

impl TimeControl {
    pub fn handle_time_commands(
        &mut self,
        blueprint_ctx: Option<&impl BlueprintContext>,
        db: &dyn TimeControlDb,
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
            let needs_repaint = self.handle_time_command(blueprint_ctx, db, command);

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
        db: &dyn TimeControlDb,
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
                    .or_else(|| db.timelines().into_values().next())
                {
                    self.timeline = super::ActiveTimeline::Auto(timeline);
                }
                self.select_valid_timeline(db);

                NeedsRepaint::Yes
            }
            TimeControlCommand::SetActiveTimeline(timeline_name) => {
                if let Some(blueprint_ctx) = blueprint_ctx {
                    blueprint_ctx.set_timeline(*timeline_name);
                }

                if let Some(timeline) = db.timelines().get(timeline_name).copied() {
                    self.timeline = super::ActiveTimeline::UserEdited(timeline);
                } else {
                    self.timeline = super::ActiveTimeline::Pending(*timeline_name);
                }

                if let Some(state) = self.states.get(timeline_name) {
                    // Use the new timeline's cached time selection.
                    if let Some(blueprint_ctx) = blueprint_ctx {
                        match state.time_selection {
                            Some(selection) => blueprint_ctx.set_time_selection(selection.to_int()),
                            None => blueprint_ctx.clear_time_selection(),
                        }
                    }
                } else if let Some(full_range) = db.time_range_for(timeline_name) {
                    self.states
                        .insert(*timeline_name, TimeState::new(full_range.min));
                }

                NeedsRepaint::Yes
            }
            TimeControlCommand::SetLoopMode(loop_mode) => {
                if self.loop_mode == *loop_mode {
                    NeedsRepaint::No
                } else {
                    if let Some(blueprint_ctx) = blueprint_ctx {
                        blueprint_ctx.set_loop_mode(*loop_mode);
                    }
                    self.loop_mode = *loop_mode;
                    if self.loop_mode != LoopMode::Off {
                        if self.play_state() == PlayState::Following {
                            self.set_play_state(Some(db), PlayState::Playing, blueprint_ctx);
                        }

                        // It makes no sense with looping and follow.
                        self.following = false;
                    }

                    NeedsRepaint::Yes
                }
            }
            TimeControlCommand::SetPlayState(play_state) => {
                if self.play_state() == *play_state {
                    NeedsRepaint::No
                } else {
                    self.set_play_state(Some(db), *play_state, blueprint_ctx);

                    if self.following {
                        if let Some(blueprint_ctx) = blueprint_ctx {
                            blueprint_ctx.set_loop_mode(LoopMode::Off);
                        }
                        self.loop_mode = LoopMode::Off;
                    }

                    NeedsRepaint::Yes
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
                self.toggle_play_pause(db, blueprint_ctx);

                NeedsRepaint::Yes
            }
            TimeControlCommand::StepTimeBack => {
                self.step_time_back(db, blueprint_ctx);

                NeedsRepaint::Yes
            }
            TimeControlCommand::StepTimeForward => {
                self.step_time_fwd(db, blueprint_ctx);

                NeedsRepaint::Yes
            }
            TimeControlCommand::Move { direction, speed } => {
                self.move_time(db, blueprint_ctx, *direction, *speed);
                NeedsRepaint::Yes
            }
            TimeControlCommand::MoveBeginning => {
                if let Some(full_range) = db.time_range_for(self.timeline_name()) {
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
                if let Some(full_range) = db.time_range_for(self.timeline_name()) {
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
                if let Some(full_range) = db.time_range_for(self.timeline_name()) {
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
                if *speed == self.speed {
                    NeedsRepaint::No
                } else {
                    self.speed = *speed;

                    if let Some(blueprint_ctx) = blueprint_ctx {
                        blueprint_ctx.set_playback_speed(*speed as f64);
                    }

                    NeedsRepaint::Yes
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
                let state = self
                    .states
                    .entry(*self.timeline_name())
                    .or_insert_with(|| TimeState::new(*time));
                state.time = *time;

                self.exit_follow_mode(db, blueprint_ctx);
                self.start_buffering();

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

    fn step_time_back(
        &mut self,
        db: &dyn TimeControlDb,
        blueprint_ctx: Option<&impl BlueprintContext>,
    ) {
        re_tracing::profile_function!();
        self.pause(blueprint_ctx);
        self.step_time_back_no_pause(db);
    }

    fn step_time_fwd(
        &mut self,
        db: &dyn TimeControlDb,
        blueprint_ctx: Option<&impl BlueprintContext>,
    ) {
        re_tracing::profile_function!();
        self.pause(blueprint_ctx);
        self.step_time_fwd_no_pause(db);
    }

    fn step_time_back_no_pause(&mut self, db: &dyn TimeControlDb) {
        if let Some(time) = self.time() {
            let timeline = self.timeline_name();
            let prev = db.prev_time_on_timeline(timeline, time.ceil());

            let new_time = if let Some(loop_range) = self.active_loop_selection() {
                if let Some(prev) = prev
                    && TimeReal::from(prev) >= loop_range.min
                {
                    prev.into()
                } else {
                    // Wrap to end of loop
                    if let Some(prev_from_end) =
                        db.prev_time_on_timeline(timeline, loop_range.max.ceil())
                    {
                        prev_from_end.into()
                    } else {
                        loop_range.max
                    }
                }
            } else if let Some(prev) = prev {
                prev.into()
            } else {
                // Wrap to the end
                if let Some(range) = db.time_range_for(timeline) {
                    range.max.into()
                } else {
                    return;
                }
            };

            if let Some(state) = self.states.get_mut(self.timeline.name()) {
                state.time = new_time;
            }
        }
    }

    fn step_time_fwd_no_pause(&mut self, db: &dyn TimeControlDb) {
        if let Some(time) = self.time() {
            let timeline = self.timeline_name();
            let next = db.next_time_on_timeline(timeline, time.floor());

            let new_time = if let Some(loop_range) = self.active_loop_selection() {
                if let Some(next) = next
                    && TimeReal::from(next) <= loop_range.max
                {
                    next.into()
                } else {
                    // Wrap to start of loop
                    if let Some(next_from_start) =
                        db.next_time_on_timeline(timeline, loop_range.min.floor())
                    {
                        next_from_start.into()
                    } else {
                        loop_range.min
                    }
                }
            } else if let Some(next) = next {
                next.into()
            } else {
                // Wrap to the start
                if let Some(range) = db.time_range_for(timeline) {
                    range.min.into()
                } else {
                    return;
                }
            };

            if let Some(state) = self.states.get_mut(self.timeline.name()) {
                state.time = new_time;
            }
        }
    }

    /// Move time by arrow keys. Preserves play/pause state, but exits follow mode.
    fn move_time(
        &mut self,
        db: &dyn TimeControlDb,
        blueprint_ctx: Option<&impl BlueprintContext>,
        direction: MoveDirection,
        speed: MoveSpeed,
    ) {
        self.exit_follow_mode(db, blueprint_ctx);

        match self.time_type() {
            Some(TimeType::Sequence) => {
                let steps = match speed {
                    MoveSpeed::Normal => 1,
                    MoveSpeed::Fast => 10,
                };
                for _ in 0..steps {
                    match direction {
                        MoveDirection::Back => {
                            self.step_time_back_no_pause(db);
                        }
                        MoveDirection::Forward => {
                            self.step_time_fwd_no_pause(db);
                        }
                    }
                }
            }
            Some(TimeType::DurationNs | TimeType::TimestampNs) => {
                let seconds = match (direction, speed) {
                    (MoveDirection::Back, MoveSpeed::Normal) => -0.1,
                    (MoveDirection::Forward, MoveSpeed::Normal) => 0.1,
                    (MoveDirection::Back, MoveSpeed::Fast) => -1.0,
                    (MoveDirection::Forward, MoveSpeed::Fast) => 1.0,
                };
                self.move_by_seconds_temporal(db, seconds);
            }
            None => {}
        }
    }

    fn move_by_seconds_temporal(&mut self, db: &dyn TimeControlDb, seconds: f64) {
        if let Some(time) = self.time() {
            let mut new_time = time + TimeReal::from_secs(seconds);

            let range = self
                .time_selection()
                .or_else(|| db.time_range_for(self.timeline_name()).map(|r| r.into()));
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

    /// If following, switch to playing. Otherwise keep the current play state.
    fn exit_follow_mode(
        &mut self,
        db: &dyn TimeControlDb,
        blueprint_ctx: Option<&impl BlueprintContext>,
    ) {
        if self.following {
            self.set_play_state(Some(db), PlayState::Playing, blueprint_ctx);
        }
    }

    fn toggle_play_pause(
        &mut self,
        db: &dyn TimeControlDb,
        blueprint_ctx: Option<&impl BlueprintContext>,
    ) {
        if self.playing {
            self.pause(blueprint_ctx);
        } else {
            // Start from beginning if we are at the end:
            if let Some(range) = db.time_range_for(self.timeline_name())
                && let Some(state) = self.states.get_mut(self.timeline.name())
                && range.max <= state.time
            {
                state.time = range.min.into();
                self.playing = true;
                self.following = false;
                return;
            }

            self.set_play_state(Some(db), PlayState::Playing, blueprint_ctx);
        }
    }
}
