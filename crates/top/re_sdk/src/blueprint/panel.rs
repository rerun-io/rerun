//! Panel types for blueprint configuration.

use std::ops::RangeInclusive;

use re_log_types::{AbsoluteTimeRange, TimeInt};
use re_sdk_types::blueprint::archetypes::{PanelBlueprint, TimePanelBlueprint};
use re_sdk_types::blueprint::components::{
    Fps, LoopMode, PanelState, PlayState, PlaybackSpeed, TimelineName,
};
use re_sdk_types::datatypes::Float64;

use crate::RecordingStreamResult;

/// Blueprint panel configuration.
#[derive(Debug, Default)]
pub struct BlueprintPanel {
    state: Option<PanelState>,
}

impl BlueprintPanel {
    /// Create a new blueprint panel.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the panel state.
    pub fn with_state(mut self, state: impl Into<PanelState>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// Create a new blueprint panel with the given state.
    pub fn from_state(state: PanelState) -> Self {
        Self::new().with_state(state)
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
#[derive(Debug, Default)]
pub struct SelectionPanel {
    state: Option<PanelState>,
}

impl SelectionPanel {
    /// Create a new selection panel.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the panel state.
    pub fn with_state(mut self, state: impl Into<PanelState>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// Create a new selection panel with the given state.
    pub fn from_state(state: PanelState) -> Self {
        Self::new().with_state(state)
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
#[derive(Debug, Default)]
pub struct TimePanel {
    state: Option<PanelState>,
    timeline: Option<String>,
    playback_speed: Option<f64>,
    fps: Option<f64>,
    play_state: Option<PlayState>,
    loop_mode: Option<LoopMode>,
    time_selection: Option<AbsoluteTimeRange>,
}

impl TimePanel {
    /// Create a new time panel.
    pub fn new() -> Self {
        Self::default()
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
    pub fn with_time_selection(mut self, range: impl Into<RangeInclusive<TimeInt>>) -> Self {
        let range = range.into();
        self.time_selection = Some(AbsoluteTimeRange::new(*range.start(), *range.end()));
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
        if let Some(range) = self.time_selection {
            arch = arch.with_time_selection(re_sdk_types::datatypes::AbsoluteTimeRange {
                min: range.min.into(),
                max: range.max.into(),
            });
        }

        stream.log("time_panel", &arch)
    }
}
