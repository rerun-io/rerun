//! The UI for the recording panel.

#[cfg(feature = "testing")]
pub mod data;

#[cfg(not(feature = "testing"))]
mod data;
mod recording_panel_ui;

pub use recording_panel_ui::RecordingPanel;

/// Commands that need to be handled in the context of the recording panel UI.
///
/// These are triggered by the related `UiCommand` because the implementation depends on the order
/// in which recordings are displayed, which is the responsibility of the recording panel.
#[derive(Clone, Debug)]
pub enum RecordingPanelCommand {
    /// Switch to the next recording in the recording panel.
    SelectNextRecording,

    /// Switch to the previous recording in the recording panel.
    SelectPreviousRecording,
}
