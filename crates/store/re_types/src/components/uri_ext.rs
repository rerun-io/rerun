use super::Uri;

impl Uri {
    /// Return the URI contained in this component.
    pub fn uri(&self) -> &str {
        self.0.as_str()
    }
}
