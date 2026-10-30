use super::TimelineFraction;

impl Default for TimelineFraction {
    #[inline]
    fn default() -> Self {
        Self(0.5.into())
    }
}
