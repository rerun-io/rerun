use super::Radius;

impl Radius {
    pub const ZERO: Self = Self(0.0);
    pub const ONE: Self = Self(1.0);
}

impl std::fmt::Display for Radius {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.prec$}", self.0, prec = crate::DISPLAY_PRECISION)
    }
}
