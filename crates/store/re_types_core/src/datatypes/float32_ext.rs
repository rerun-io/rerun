use super::Float32;

impl std::fmt::Display for Float32 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.prec$}", self.0, prec = crate::DISPLAY_PRECISION)
    }
}

impl std::ops::Deref for Float32 {
    type Target = f32;

    #[inline]
    fn deref(&self) -> &f32 {
        &self.0
    }
}

impl std::ops::DerefMut for Float32 {
    #[inline]
    fn deref_mut(&mut self) -> &mut f32 {
        &mut self.0
    }
}
