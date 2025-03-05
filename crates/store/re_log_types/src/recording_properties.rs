use re_types_core::{archetypes, components};

use crate::Time;

/// The following are the recording properties that are relevant for the viewer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordingProperties {
    /// The user-chosen name of the application doing the logging.
    pub application_id: components::ApplicationId,

    /// When the recording started.
    ///
    /// Should be an absolute time, i.e. relative to Unix Epoch.
    pub recording_started: Time,

    /// An optional name for the recording.
    pub recording_name: Option<String>,
}

impl From<RecordingProperties> for archetypes::RecordingProperties {
    fn from(value: RecordingProperties) -> Self {
        let started = components::RecordingStartedTimestamp::from(
            value.recording_started.nanos_since_epoch(),
        );

        let s = Self::new([value.application_id], [started]);

        if let Some(name) = value.recording_name {
            s.with_name([name])
        } else {
            s
        }
    }
}
