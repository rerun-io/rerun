use super::Resolution;

impl Default for Resolution {
    fn default() -> Self {
        // Picking anything specific seems more arbitrary than just 0.
        [0.0, 0.0].into()
    }
}
