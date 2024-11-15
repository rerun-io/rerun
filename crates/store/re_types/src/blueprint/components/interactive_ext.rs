use super::Interactive;

impl Default for Interactive {
    #[inline]
    fn default() -> Self {
        Self(true.into())
    }
}
