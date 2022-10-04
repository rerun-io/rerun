use fixed::{traits::LossyInto as _, FixedI128};

use re_log_types::TimeInt;

/// Either nanoseconds or sequence numbers.
///
/// Must be matched with a [`re_log_types::TimeType`] to know what.
///
/// Used both for time points and durations.
///
/// This is like [`TimeInt`] with added precision to be able to represent
/// time between sequences (and even between nanoseconds).
/// This is needed in the time panel to refer to time between sequence numbers,
/// e.g. for panning.
///
/// We use 64+64 bit fixed point representation in order to support
/// large numbers (nanos since unix epoch) with sub-integer precision.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub struct TimeReal(FixedI128<typenum::U64>);

impl TimeReal {
    #[inline]
    pub fn floor(&self) -> TimeInt {
        let int: i64 = self.0.floor().lossy_into();
        TimeInt::from(int)
    }

    #[inline]
    pub fn round(&self) -> TimeInt {
        let int: i64 = self.0.round().lossy_into();
        TimeInt::from(int)
    }

    #[inline]
    pub fn ceil(&self) -> TimeInt {
        let int: i64 = self.0.ceil().lossy_into();
        TimeInt::from(int)
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
        Self(self.0.abs())
    }
}

// ---------------

impl From<i64> for TimeReal {
    #[inline]
    fn from(integer: i64) -> Self {
        Self(integer.into())
    }
}

impl From<f32> for TimeReal {
    #[inline]
    fn from(value: f32) -> Self {
        Self(FixedI128::from_num(value))
    }
}

impl From<f64> for TimeReal {
    #[inline]
    fn from(value: f64) -> Self {
        Self(FixedI128::from_num(value))
    }
}

impl From<TimeInt> for TimeReal {
    #[inline]
    fn from(time_int: TimeInt) -> Self {
        Self::from(time_int.as_i64())
    }
}

impl From<re_log_types::Duration> for TimeReal {
    #[inline]
    fn from(duration: re_log_types::Duration) -> Self {
        Self::from(duration.as_nanos())
    }
}

impl From<re_log_types::Time> for TimeReal {
    #[inline]
    fn from(time: re_log_types::Time) -> Self {
        Self::from(time.nanos_since_epoch())
    }
}

impl From<TimeReal> for re_log_types::Time {
    #[inline]
    fn from(int: TimeReal) -> Self {
        Self::from_ns_since_epoch(int.round().as_i64())
    }
}

impl From<TimeReal> for re_log_types::Duration {
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
        Self(self.0.saturating_mul(FixedI128::from_num(rhs)))
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
        let mut sum = TimeReal::from(0);
        for item in iter {
            sum += item;
        }
        sum
    }
}

// ---------------

impl std::ops::Add<TimeInt> for TimeReal {
    type Output = TimeReal;

    #[inline]
    fn add(self, rhs: TimeInt) -> Self::Output {
        self + TimeReal::from(rhs)
    }
}

impl std::ops::Sub<TimeInt> for TimeReal {
    type Output = TimeReal;

    #[inline]
    fn sub(self, rhs: TimeInt) -> Self::Output {
        self - TimeReal::from(rhs)
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
}
