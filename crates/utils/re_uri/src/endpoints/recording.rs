use crate::{Origin, TimeRange};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordingEndpoint {
    pub origin: Origin,
    pub recording_id: String,
    pub time_range: Option<TimeRange>,
}

impl std::fmt::Display for RecordingEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/recording/{}", self.origin, self.recording_id)?;
        if let Some(time_range) = &self.time_range {
            write!(f, "?time_range={time_range}")?;
        }
        Ok(())
    }
}

impl RecordingEndpoint {
    pub fn new(origin: Origin, recording_id: String, time_range: Option<TimeRange>) -> Self {
        Self {
            origin,
            recording_id,
            time_range,
        }
    }
}
