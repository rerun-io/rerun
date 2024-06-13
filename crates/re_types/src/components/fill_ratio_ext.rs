use super::FillRatio;

impl std::fmt::Display for FillRatio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.prec$}", self.0 .0, prec = crate::DISPLAY_PRECISION)
    }
}

impl Default for FillRatio {
    #[inline]
    fn default() -> Self {
        1.0.into()
    }
}
