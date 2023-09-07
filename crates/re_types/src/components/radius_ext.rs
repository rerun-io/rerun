use super::Radius;

impl Radius {
    pub const ZERO: Self = Self::new(0.0);
    pub const ONE: Self = Self::new(1.0);

    #[inline]
    pub const fn new(r: f32) -> Self {
        Self(r)
    }
}

impl From<f32> for Radius {
    #[inline]
    fn from(r: f32) -> Self {
        Self::new(r)
    }
}

impl std::fmt::Display for Radius {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.prec$}", self.0, prec = crate::DISPLAY_PRECISION)
    }
}
