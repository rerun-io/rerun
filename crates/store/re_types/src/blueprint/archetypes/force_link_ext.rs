use super::ForceLink;

impl Default for ForceLink {
    fn default() -> Self {
        Self {
            enabled: true.into(),
            distance: (30.).into(),
        }
    }
}
