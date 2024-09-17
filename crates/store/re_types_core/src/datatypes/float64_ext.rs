use super::Float64;

impl std::fmt::Display for Float64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prec = f.precision().unwrap_or(crate::DEFAULT_DISPLAY_DECIMALS);
        write!(f, "{:.prec$}", self.0)
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
