//! Panel types for blueprint configuration.

use re_sdk_types::blueprint::archetypes::{PanelBlueprint, TimePanelBlueprint};
use re_sdk_types::blueprint::components::{
    AbsoluteTimeRange, Fps, LoopMode, PanelState, PlayState, PlaybackSpeed, TimelineName,
};
use re_sdk_types::datatypes::Float64;

use crate::RecordingStreamResult;

/// Blueprint panel configuration.
#[derive(Debug)]
pub struct BlueprintPanel {
    state: Option<PanelState>,
}

impl BlueprintPanel {
    /// Create a new blueprint panel with the given state.
    pub fn new(state: impl Into<PanelState>) -> Self {
        Self {
            state: Some(state.into()),
        }
    }

    /// Set the panel state.
    pub fn with_state(mut self, state: impl Into<PanelState>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// Log this panel to the blueprint stream.
    pub(crate) fn log_to_stream(
        &self,
        stream: &crate::RecordingStream,
    ) -> RecordingStreamResult<()> {
        let mut arch = PanelBlueprint::new();
        if let Some(state) = self.state {
            arch = arch.with_state(state);
        }
        stream.log("blueprint_panel", &arch)
    }
}

/// Selection panel configuration.
#[derive(Debug)]
pub struct SelectionPanel {
    state: Option<PanelState>,
}

impl SelectionPanel {
    /// Create a new selection panel with the given state.
    pub fn new(state: impl Into<PanelState>) -> Self {
        Self {
            state: Some(state.into()),
        }
    }

    /// Set the panel state.
    pub fn with_state(mut self, state: impl Into<PanelState>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// Log this panel to the blueprint stream.
    pub(crate) fn log_to_stream(
        &self,
        stream: &crate::RecordingStream,
    ) -> RecordingStreamResult<()> {
        let mut arch = PanelBlueprint::new();
        if let Some(state) = self.state {
            arch = arch.with_state(state);
        }
        stream.log("selection_panel", &arch)
    }
}

/// Time panel configuration.
#[derive(Debug)]
pub struct TimePanel {
    state: Option<PanelState>,
    timeline: Option<String>,
    playback_speed: Option<f64>,
    fps: Option<f64>,
    play_state: Option<PlayState>,
    loop_mode: Option<LoopMode>,
    time_selection: Option<(i64, i64)>,
}

impl TimePanel {
    /// Create a new time panel.
    pub fn new() -> Self {
        Self {
            state: None,
            timeline: None,
            playback_speed: None,
            fps: None,
            play_state: None,
            loop_mode: None,
            time_selection: None,
        }
    }

    /// Set the panel state.
    pub fn with_state(mut self, state: impl Into<PanelState>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// Set the timeline name.
    pub fn with_timeline(mut self, timeline: impl Into<String>) -> Self {
        self.timeline = Some(timeline.into());
        self
    }

    /// Set the playback speed multiplier.
    pub fn with_playback_speed(mut self, speed: f64) -> Self {
        self.playback_speed = Some(speed);
        self
    }

    /// Set the frames per second (only applicable for sequence timelines).
    pub fn with_fps(mut self, fps: f64) -> Self {
        self.fps = Some(fps);
        self
    }

    /// Set the play state (paused, playing, or following).
    pub fn with_play_state(mut self, play_state: impl Into<PlayState>) -> Self {
        self.play_state = Some(play_state.into());
        self
    }

    /// Set the loop mode.
    pub fn with_loop_mode(mut self, loop_mode: impl Into<LoopMode>) -> Self {
        self.loop_mode = Some(loop_mode.into());
        self
    }

    /// Set the time selection range.
    pub fn with_time_selection(mut self, start: i64, end: i64) -> Self {
        self.time_selection = Some((start, end));
        self
    }

    /// Log this panel to the blueprint stream.
    pub(crate) fn log_to_stream(
        &self,
        stream: &crate::RecordingStream,
    ) -> RecordingStreamResult<()> {
        let mut arch = TimePanelBlueprint::new();

        if let Some(state) = self.state {
            arch = arch.with_state(state);
        }
        if let Some(ref timeline) = self.timeline {
            arch = arch.with_timeline(TimelineName(timeline.clone().into()));
        }
        if let Some(speed) = self.playback_speed {
            arch = arch.with_playback_speed(PlaybackSpeed(Float64(speed)));
        }
        if let Some(fps) = self.fps {
            arch = arch.with_fps(Fps(Float64(fps)));
        }
        if let Some(play_state) = self.play_state {
            arch = arch.with_play_state(play_state);
        }
        if let Some(loop_mode) = self.loop_mode {
            arch = arch.with_loop_mode(loop_mode);
        }
        if let Some((start, end)) = self.time_selection {
            arch = arch.with_time_selection(AbsoluteTimeRange(
                re_sdk_types::datatypes::AbsoluteTimeRange {
                    min: start.into(),
                    max: end.into(),
                },
            ));
        }

        stream.log("time_panel", &arch)
    }
}

impl Default for TimePanel {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions to convert string literals to enum values
impl BlueprintPanel {
    /// Create a blueprint panel from a state string ("hidden", "collapsed", "expanded").
    pub fn from_state(state: &str) -> Self {
        Self::new(parse_panel_state(state))
    }
}

impl SelectionPanel {
    /// Create a selection panel from a state string ("hidden", "collapsed", "expanded").
    pub fn from_state(state: &str) -> Self {
        Self::new(parse_panel_state(state))
    }
}

impl TimePanel {
    /// Set the panel state from a string ("hidden", "collapsed", "expanded").
    pub fn with_state_str(mut self, state: &str) -> Self {
        self.state = Some(parse_panel_state(state));
        self
    }

    /// Set the play state from a string ("paused", "playing", "following").
    pub fn with_play_state_str(mut self, play_state: &str) -> Self {
        self.play_state = Some(parse_play_state(play_state));
        self
    }

    /// Set the loop mode from a string ("off", "selection", "all").
    pub fn with_loop_mode_str(mut self, loop_mode: &str) -> Self {
        self.loop_mode = Some(parse_loop_mode(loop_mode));
        self
    }
}

fn parse_panel_state(s: &str) -> PanelState {
    match s {
        "hidden" => PanelState::Hidden,
        "collapsed" => PanelState::Collapsed,
        "expanded" => PanelState::Expanded,
        _ => PanelState::Expanded, // Default to expanded for unknown values
    }
}

fn parse_play_state(s: &str) -> PlayState {
    match s {
        "paused" => PlayState::Paused,
        "playing" => PlayState::Playing,
        "following" => PlayState::Following,
        _ => PlayState::Playing, // Default
    }
}

fn parse_loop_mode(s: &str) -> LoopMode {
    match s {
        "off" => LoopMode::Off,
        "selection" => LoopMode::Selection,
        "all" => LoopMode::All,
        _ => LoopMode::Off, // Default
    }
}
