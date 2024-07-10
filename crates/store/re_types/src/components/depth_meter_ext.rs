use super::DepthMeter;

impl std::fmt::Display for DepthMeter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.prec$}", self.0, prec = crate::DISPLAY_PRECISION)
    }
}

impl Default for DepthMeter {
    #[inline]
    fn default() -> Self {
        Self(1.0) // 1 unit == 1 meter.
    }
}
