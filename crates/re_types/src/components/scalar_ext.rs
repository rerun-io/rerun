use super::Scalar;

impl Scalar {
    #[inline]
    pub const fn new(r: f64) -> Self {
        Self(r)
    }
}

impl From<f64> for Scalar {
    #[inline]
    fn from(r: f64) -> Self {
        Self::new(r)
    }
}

impl std::fmt::Display for Scalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.prec$}", self.0, prec = crate::DISPLAY_PRECISION)
    }
}
