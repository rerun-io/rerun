use super::Float64;

impl std::fmt::Display for Float64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.prec$}", self.0, prec = crate::DISPLAY_PRECISION)
    }
}

impl std::ops::Deref for Float64 {
    type Target = f64;

    #[inline]
    fn deref(&self) -> &f64 {
        &self.0
    }
}

impl std::ops::DerefMut for Float64 {
    #[inline]
    fn deref_mut(&mut self) -> &mut f64 {
        &mut self.0
    }
}
