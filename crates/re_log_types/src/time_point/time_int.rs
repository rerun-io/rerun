use crate::time::{Duration, Time};

/// A 64-bit number describing either nanoseconds OR sequence numbers.
///
/// Must be matched with a [`crate::TimeType`] to know what.
///
/// Used both for time points and durations.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimeInt(pub(crate) i64);

impl re_types_core::SizeBytes for TimeInt {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

impl TimeInt {
    /// Special value used to represent static data in the time panel.
    ///
    /// The reason we don't use i64::MIN is because in the time panel we need
    /// to be able to pan to before the [`TimeInt::BEGINNING`], and so we need
    /// a bit of leeway.
    //
    // TODO(#5264): remove this once the timeless
    #[doc(hidden)]
    pub const STATIC_TIME_PANEL: Self = Self(i64::MIN / 2);

    // TODO(#4832): `TimeInt::BEGINNING` vs. `TimeInt::MIN` vs. `Option<TimeInt>`…
    pub const MIN: Self = Self(i64::MIN);
    pub const MAX: Self = Self(i64::MAX);

    /// For time timelines.
    #[inline]
    pub fn from_nanos(nanos: i64) -> Self {
        Self(nanos)
    }

    /// For time timelines.
    #[inline]
    pub fn from_milliseconds(millis: i64) -> Self {
        Self::from_nanos(millis * 1_000_000)
    }

    /// For time timelines.
    #[inline]
    pub fn from_seconds(seconds: i64) -> Self {
        Self::from_nanos(seconds.saturating_mul(1_000_000_000))
    }

    /// For sequence timelines.
    #[inline]
    pub fn from_sequence(sequence: i64) -> Self {
        Self(sequence)
    }

    #[inline]
    pub fn as_i64(&self) -> i64 {
        self.0
    }

    #[inline]
    pub fn as_f32(&self) -> f32 {
        self.0 as _
    }

    #[inline]
    pub fn as_f64(&self) -> f64 {
        self.0 as _
    }

    #[inline]
    pub fn abs(&self) -> Self {
        Self(self.0.saturating_abs())
    }
}

impl From<i64> for TimeInt {
    #[inline]
    fn from(seq: i64) -> Self {
        Self(seq)
    }
}

impl From<Duration> for TimeInt {
    #[inline]
    fn from(duration: Duration) -> Self {
        Self(duration.as_nanos())
    }
}

impl From<Time> for TimeInt {
    #[inline]
    fn from(time: Time) -> Self {
        Self(time.nanos_since_epoch())
    }
}

impl From<TimeInt> for Time {
    #[inline]
    fn from(int: TimeInt) -> Self {
        Self::from_ns_since_epoch(int.as_i64())
    }
}

impl From<TimeInt> for Duration {
    #[inline]
    fn from(int: TimeInt) -> Self {
        Self::from_nanos(int.as_i64())
    }
}

impl std::ops::Neg for TimeInt {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self(self.0.saturating_neg())
    }
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

impl std::ops::AddAssign for TimeInt {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl std::ops::SubAssign for TimeInt {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl std::iter::Sum for TimeInt {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut sum = TimeInt(0);
        for item in iter {
            sum += item;
        }
        sum
    }
}
