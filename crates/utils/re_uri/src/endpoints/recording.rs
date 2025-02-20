use crate::Origin;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordingEndpoint {
    pub origin: Origin,
    pub recording_id: String,
}

impl std::fmt::Display for RecordingEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/recording/{}", self.origin, self.recording_id)
    }
}

impl RecordingEndpoint {
    pub fn new(origin: Origin, recording_id: String) -> Self {
        Self {
            origin,
            recording_id,
        }
    }
}
