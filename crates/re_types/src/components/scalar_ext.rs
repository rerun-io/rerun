use super::Scalar;

impl std::fmt::Display for Scalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Default for Scalar {
    #[inline]
    fn default() -> Self {
        Self(0.0)
    }
}
