use fixed::FixedI128;
use fixed::traits::LossyInto as _;

use super::NonMinI64;
use crate::TimeInt;

/// Either nanoseconds or sequence numbers.
///
/// Must be matched with a [`crate::TimeType`] to know what.
///
/// Used both for time points and durations.
///
/// This is like [`TimeInt`] with added precision to be able to represent
/// time between sequences (and even between nanoseconds).
/// This is needed in the time panel to refer to time between sequence numbers,
/// e.g. for smooth panning.
///
/// We use 64+64 bit fixed point representation in order to support
/// large numbers (nanos since unix epoch) with sub-integer precision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimeReal(FixedI128<typenum::U64>);

impl TimeReal {
    pub const MIN: Self = Self(FixedI128::MIN);
    pub const ZERO: Self = Self(FixedI128::ZERO);
    pub const MAX: Self = Self(FixedI128::MAX);

    #[inline]
    pub fn floor(&self) -> TimeInt {
        let int: i64 = self.0.saturating_floor().lossy_into();
        TimeInt::new_temporal(int)
    }

    #[inline]
    pub fn round(&self) -> TimeInt {
        let int: i64 = self.0.saturating_round().lossy_into();
        TimeInt::new_temporal(int)
    }

    #[inline]
    pub fn ceil(&self) -> TimeInt {
        let int: i64 = self.0.saturating_ceil().lossy_into();
        TimeInt::new_temporal(int)
    }

    #[inline]
    pub fn as_f32(&self) -> f32 {
        self.0.lossy_into()
    }

    #[inline]
    pub fn as_f64(self) -> f64 {
        self.0.lossy_into()
    }

    #[inline]
    pub fn abs(self) -> Self {
        Self(self.0.saturating_abs())
    }

    #[inline]
    pub fn from_secs(v: f64) -> Self {
        Self::from(v * 1_000_000_000f64)
    }

    #[inline]
    pub fn as_secs_f64(self) -> f64 {
        self.as_f64() / 1_000_000_000f64
    }

    /// Returns the value half-way to `other`.
    pub fn midpoint(self, other: Self) -> Self {
        Self((self.0 + other.0) / FixedI128::from_num(2))
    }
}

// ---------------

impl From<i64> for TimeReal {
    #[inline]
    fn from(integer: i64) -> Self {
        Self(integer.into())
    }
}

impl From<NonMinI64> for TimeReal {
    #[inline]
    fn from(integer: NonMinI64) -> Self {
        Self(integer.get().into())
    }
}

impl From<f32> for TimeReal {
    /// Saturating cast
    #[inline]
    fn from(value: f32) -> Self {
        debug_assert!(!value.is_nan());
        if value.is_nan() {
            re_log::warn_once!("NaN time detected");
            Self(0.into())
        } else if let Some(num) = FixedI128::checked_from_num(value) {
            Self(num)
        } else if value < 0.0 {
            Self::MIN
        } else {
            Self::MAX
        }
    }
}

impl From<f64> for TimeReal {
    /// Saturating cast
    #[inline]
    fn from(value: f64) -> Self {
        debug_assert!(!value.is_nan());
        if value.is_nan() {
            re_log::warn_once!("NaN time detected");
            Self(0.into())
        } else if let Some(num) = FixedI128::checked_from_num(value) {
            Self(num)
        } else if value < 0.0 {
            Self::MIN
        } else {
            Self::MAX
        }
    }
}

impl From<TimeInt> for TimeReal {
    #[inline]
    fn from(time_int: TimeInt) -> Self {
        Self::from(time_int.as_i64())
    }
}

impl From<crate::Duration> for TimeReal {
    #[inline]
    fn from(duration: crate::Duration) -> Self {
        Self::from(duration.as_nanos())
    }
}

impl From<TimeReal> for crate::Duration {
    #[inline]
    fn from(int: TimeReal) -> Self {
        Self::from_nanos(int.round().as_i64())
    }
}

// ---------------

impl std::ops::Neg for TimeReal {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self(self.0.saturating_neg())
    }
}

impl std::ops::Add for TimeReal {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl std::ops::Sub for TimeReal {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl std::ops::Mul<f64> for TimeReal {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f64) -> Self::Output {
        Self(self.0.saturating_mul(Self::from(rhs).0))
    }
}

impl std::ops::AddAssign for TimeReal {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.0 = self.0.saturating_add(rhs.0);
    }
}

impl std::ops::SubAssign for TimeReal {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 = self.0.saturating_sub(rhs.0);
    }
}

impl std::iter::Sum for TimeReal {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut sum = Self::from(0);
        for item in iter {
            sum += item;
        }
        sum
    }
}

// ---------------

impl std::ops::Add<TimeInt> for TimeReal {
    type Output = Self;

    #[inline]
    fn add(self, rhs: TimeInt) -> Self::Output {
        self + Self::from(rhs)
    }
}

impl std::ops::Sub<TimeInt> for TimeReal {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: TimeInt) -> Self::Output {
        self - Self::from(rhs)
    }
}

impl std::ops::Add<TimeReal> for TimeInt {
    type Output = TimeReal;

    #[inline]
    fn add(self, rhs: TimeReal) -> Self::Output {
        TimeReal::from(self) + rhs
    }
}

impl std::ops::Sub<TimeReal> for TimeInt {
    type Output = TimeReal;

    #[inline]
    fn sub(self, rhs: TimeReal) -> Self::Output {
        TimeReal::from(self) - rhs
    }
}

// ---------------

impl PartialEq<TimeInt> for TimeReal {
    #[inline]
    fn eq(&self, other: &TimeInt) -> bool {
        self.0 == other.as_i64()
    }
}

impl PartialEq<TimeReal> for TimeInt {
    #[inline]
    fn eq(&self, other: &TimeReal) -> bool {
        self.as_i64() == other.0
    }
}

impl PartialOrd<TimeInt> for TimeReal {
    #[inline]
    fn partial_cmp(&self, other: &TimeInt) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.as_i64())
    }
}

impl PartialOrd<TimeReal> for TimeInt {
    #[inline]
    fn partial_cmp(&self, other: &TimeReal) -> Option<std::cmp::Ordering> {
        self.as_i64().partial_cmp(&other.0)
    }
}

// ---------------

#[test]
fn test_time_value_f() {
    type T = TimeReal;

    let nice_floats = [-1.75, -0.25, 0.0, 0.25, 1.0, 1.75];
    for &f in &nice_floats {
        assert_eq!(T::from(f).as_f64(), f);
        assert_eq!(-T::from(f), T::from(-f));
        assert_eq!(T::from(f).abs(), T::from(f.abs()));

        for &g in &nice_floats {
            assert_eq!(T::from(f) + T::from(g), T::from(f + g));
            assert_eq!(T::from(f) - T::from(g), T::from(f - g));
        }
    }

    assert_eq!(TimeReal::from(f32::NEG_INFINITY), TimeReal::MIN);
    assert_eq!(TimeReal::from(f32::MIN), TimeReal::MIN);
    assert_eq!(TimeReal::from(f32::INFINITY), TimeReal::MAX);
    assert_eq!(TimeReal::from(f32::MAX), TimeReal::MAX);
}
