use super::Scalar;

impl std::fmt::Display for Scalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.prec$}", self.0, prec = crate::DISPLAY_PRECISION)
    }
}
