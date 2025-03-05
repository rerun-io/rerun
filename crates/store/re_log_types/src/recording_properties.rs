use re_types_core::{archetypes, components};

use crate::Time;

#[derive(
    Clone, Debug, PartialOrd, Ord, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize,
)]
pub struct ApplicationId(String);

impl From<&str> for ApplicationId {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl From<String> for ApplicationId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<ApplicationId> for String {
    fn from(s: ApplicationId) -> Self {
        s.0
    }
}

impl ApplicationId {
    /// The default [`ApplicationId`] if the user hasn't set one.
    ///
    /// Currently: `"unknown_app_id"`.
    pub fn unknown() -> Self {
        Self("unknown_app_id".to_owned())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for ApplicationId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<ApplicationId> for components::ApplicationId {
    fn from(value: ApplicationId) -> Self {
        Self::from(value.0)
    }
}

impl From<components::ApplicationId> for ApplicationId {
    fn from(value: components::ApplicationId) -> Self {
        Self::from(value.0.as_str())
    }
}

impl re_byte_size::SizeBytes for ApplicationId {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }
}

/// The following are the recording properties that are relevant for the viewer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordingProperties {
    /// The user-chosen name of the application doing the logging.
    pub application_id: ApplicationId,

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
