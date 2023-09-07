use super::DepthMeter;

impl DepthMeter {
    #[inline]
    pub const fn new(r: f32) -> Self {
        Self(r)
    }
}

impl From<f32> for DepthMeter {
    #[inline]
    fn from(r: f32) -> Self {
        Self::new(r)
    }
}

impl std::fmt::Display for DepthMeter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.prec$}", self.0, prec = crate::DISPLAY_PRECISION)
    }
}
