use super::TimeInt;

impl TimeInt {
    pub const MIN: Self = Self(i64::MIN);
    pub const MAX: Self = Self(i64::MAX);
}
