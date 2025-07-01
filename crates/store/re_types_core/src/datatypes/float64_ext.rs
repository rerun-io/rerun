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

impl From<half::f16> for Float64 {
    #[inline]
    fn from(value: half::f16) -> Self {
        Self(value.to_f64())
    }
}

impl From<f32> for Float64 {
    #[inline]
    fn from(value: f32) -> Self {
        Self(value as f64)
    }
}

impl From<i8> for Float64 {
    #[inline]
    fn from(value: i8) -> Self {
        Self(value as f64)
    }
}

impl From<i16> for Float64 {
    #[inline]
    fn from(value: i16) -> Self {
        Self(value as f64)
    }
}

impl From<i32> for Float64 {
    #[inline]
    fn from(value: i32) -> Self {
        Self(value as f64)
    }
}

impl From<u8> for Float64 {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value as f64)
    }
}

impl From<u16> for Float64 {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value as f64)
    }
}

impl From<u32> for Float64 {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value as f64)
    }
}
