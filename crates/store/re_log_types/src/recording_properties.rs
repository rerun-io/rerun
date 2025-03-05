use re_types_core::components;

use crate::Time;

/// The following are the recording properties that are relevant for the viewer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordingProperties {
    /// The user-chosen name of the application doing the logging.
    pub application_name: components::ApplicationId,

    /// When the recording started.
    ///
    /// Should be an absolute time, i.e. relative to Unix Epoch.
    pub recording_started: Time,

    /// An optional name for the recording.
    pub recording_name: Option<String>,
}
