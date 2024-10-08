use super::TimeInt;

impl TimeInt {
    // matches `re_log_types::TimeInt::MIN`
    pub const MIN: Self = Self(i64::MIN + 1);
    pub const MAX: Self = Self(i64::MAX);
}

impl std::ops::Add for TimeInt {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl std::ops::Sub for TimeInt {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}
