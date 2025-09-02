//! The UI for the recording panel.

#[cfg(feature = "testing")]
pub mod data;

#[cfg(not(feature = "testing"))]
mod data;
mod recording_panel_ui;

pub use self::recording_panel_ui::recordings_panel_ui;
