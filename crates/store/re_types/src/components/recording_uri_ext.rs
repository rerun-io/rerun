use super::RecordingUri;

impl RecordingUri {
    /// Return the Recording URI contained in this component.
    pub fn uri(&self) -> &str {
        self.0.as_str()
    }
}
