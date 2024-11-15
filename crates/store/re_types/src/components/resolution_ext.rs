use super::Resolution;

impl Default for Resolution {
    #[inline]
    fn default() -> Self {
        // Picking anything specific seems more arbitrary than just 0.
        [0.0, 0.0].into()
    }
}
