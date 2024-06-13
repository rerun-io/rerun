use super::Radius;

impl Radius {
    /// Zero radius.
    pub const ZERO: Self = Self(0.0);

    /// Unit radius.
    pub const ONE: Self = Self(1.0);
}

impl std::fmt::Display for Radius {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.prec$}", self.0, prec = crate::DISPLAY_PRECISION)
    }
}

impl Default for Radius {
    #[inline]
    fn default() -> Self {
        Self::ONE
    }
}
