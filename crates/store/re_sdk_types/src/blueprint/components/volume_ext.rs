use super::Volume;

impl Default for Volume {
    #[inline]
    fn default() -> Self {
        Self(1.0.into())
    }
}
