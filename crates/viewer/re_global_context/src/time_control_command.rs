use re_chunk::TimelineName;
use re_log_types::{AbsoluteTimeRange, TimeReal};

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
    SetLooping(Looping),
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
