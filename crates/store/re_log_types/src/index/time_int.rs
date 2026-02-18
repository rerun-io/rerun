use crate::{Duration, NonMinI64, TryFromIntError};

/// A 64-bit number describing either nanoseconds, sequence numbers or fully static data.
///
/// Must be matched with a [`crate::TimeType`] to know what.
///
/// Used both for time points and durations.
#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimeInt(Option<NonMinI64>);

static_assertions::assert_eq_size!(TimeInt, i64);
static_assertions::assert_eq_align!(TimeInt, i64);

impl re_byte_size::SizeBytes for TimeInt {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl std::fmt::Debug for TimeInt {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(NonMinI64::MIN) => f.debug_tuple("TimeInt::MIN").finish(),
            Some(NonMinI64::MAX) => f.debug_tuple("TimeInt::MAX").finish(),
            Some(t) => f.write_fmt(format_args!("TimeInt({})", re_format::format_int(t.get()))),
            None => f.debug_tuple("TimeInt::STATIC").finish(),
        }
    }
}

impl TimeInt {
    /// Special value used to represent static data.
    ///
    /// It is illegal to create a [`TimeInt`] with that value in a temporal context.
    ///
    /// SDK users cannot log data at that timestamp explicitly, the only way to do so is to use
    /// the static APIs.
    pub const STATIC: Self = Self(None);

    /// Value used to represent the minimal temporal value a [`TimeInt`] can hold.
    ///
    /// This is _not_ `i64::MIN`, as that is a special value reserved as a marker for static
    /// data (see [`Self::STATIC`]).
    pub const MIN: Self = Self(Some(NonMinI64::MIN));

    /// Value used to represent the maximum temporal value a [`TimeInt`] can hold.
    pub const MAX: Self = Self(Some(NonMinI64::MAX));

    pub const ZERO: Self = Self(Some(NonMinI64::ZERO));

    pub const ONE: Self = Self(Some(NonMinI64::ONE));

    #[inline]
    pub fn is_static(self) -> bool {
        self == Self::STATIC
    }

    /// Creates a new temporal [`TimeInt`].
    ///
    /// If `time` is `i64::MIN`, this will return [`TimeInt::MIN`].
    ///
    /// This can't return [`TimeInt::STATIC`], ever.
    #[inline]
    pub fn new_temporal(time: i64) -> Self {
        NonMinI64::new(time).map_or(Self::MIN, |t| Self(Some(t)))
    }

    /// For time timelines.
    #[inline]
    pub fn from_nanos(nanos: NonMinI64) -> Self {
        Self(Some(nanos))
    }

    /// For time timelines.
    #[inline]
    pub fn from_millis(millis: NonMinI64) -> Self {
        Self::new_temporal(millis.get().saturating_mul(1_000_000))
    }

    /// For time timelines.
    #[inline]
    pub fn from_secs(seconds: f64) -> Self {
        Self::new_temporal((seconds * 1e9).round() as _)
    }

    /// For sequence timelines.
    #[inline]
    pub fn from_sequence(sequence: NonMinI64) -> Self {
        Self(Some(sequence))
    }

    /// Clamp to valid non-static range.
    #[inline]
    pub fn saturated_temporal_i64(value: impl Into<i64>) -> Self {
        Self(Some(NonMinI64::saturating_from_i64(value)))
    }

    /// Clamp to valid non-static range.
    #[inline]
    pub fn saturated_temporal(value: impl TryInto<Self>) -> Self {
        value.try_into().unwrap_or(Self::MIN).max(Self::MIN)
    }

    /// Returns `i64::MIN` for [`Self::STATIC`].
    #[inline]
    pub const fn as_i64(self) -> i64 {
        match self.0 {
            Some(t) => t.get(),
            None => i64::MIN,
        }
    }

    /// Returns `f64::MIN` for [`Self::STATIC`].
    #[inline]
    pub const fn as_f64(self) -> f64 {
        match self.0 {
            Some(t) => t.get() as _,
            None => f64::MIN,
        }
    }

    /// Always returns [`Self::STATIC`] for [`Self::STATIC`].
    #[inline]
    #[must_use]
    pub fn inc(self) -> Self {
        match self.0 {
            Some(t) => Self::new_temporal(t.get().saturating_add(1)),
            None => self,
        }
    }

    /// Always returns [`Self::STATIC`] for [`Self::STATIC`].
    #[inline]
    #[must_use]
    pub fn dec(self) -> Self {
        match self.0 {
            Some(t) => Self::new_temporal(t.get().saturating_sub(1)),
            None => self,
        }
    }

    /// Calculates the midpoint (average) between `self` and `rhs`.
    ///
    /// If either is static (non-temporal), then this returns [`Self::STATIC`].
    #[inline]
    pub fn midpoint(&self, rhs: Self) -> Self {
        match (self.0, rhs.0) {
            (Some(lhs), Some(rhs)) => Self::from(lhs.midpoint(rhs)),
            _ => Self::STATIC,
        }
    }

    pub fn closest_multiple_of(&self, snap_interval: i64) -> Self {
        re_log::debug_assert!(1 <= snap_interval);
        match self.0 {
            Some(t) => {
                let v = t.get();
                let snapped = (v + snap_interval / 2).div_euclid(snap_interval) * snap_interval;
                Self::new_temporal(snapped)
            }
            None => Self::STATIC,
        }
    }

    pub fn saturating_sub(&self, arg: i64) -> Self {
        match self.0 {
            Some(t) => Self::new_temporal(t.get().saturating_sub(arg)),
            None => Self::STATIC,
        }
    }
}

impl TryFrom<i64> for TimeInt {
    type Error = TryFromIntError;

    #[inline]
    fn try_from(t: i64) -> Result<Self, Self::Error> {
        let Some(t) = NonMinI64::new(t) else {
            return Err(TryFromIntError);
        };
        Ok(Self(Some(t)))
    }
}

impl From<NonMinI64> for TimeInt {
    #[inline]
    fn from(seq: NonMinI64) -> Self {
        Self(Some(seq))
    }
}

impl From<TimeInt> for NonMinI64 {
    fn from(value: TimeInt) -> Self {
        match value.0 {
            Some(value) => value,
            None => Self::MIN,
        }
    }
}

// TODO(#9534): refactor this mess
// impl TryFrom<TimeInt> for NonMinI64 {
//     type Error = TryFromIntError;

//     #[inline]
//     fn try_from(t: TimeInt) -> Result<Self, Self::Error> {
//         Self::new(t.as_i64()).ok_or(TryFromIntError)
//     }
// }

impl From<TimeInt> for Duration {
    #[inline]
    fn from(int: TimeInt) -> Self {
        Self::from_nanos(int.as_i64())
    }
}

impl From<TimeInt> for re_types_core::datatypes::TimeInt {
    #[inline]
    fn from(time: TimeInt) -> Self {
        Self(time.as_i64())
    }
}

impl From<re_types_core::datatypes::TimeInt> for TimeInt {
    #[inline]
    fn from(time: re_types_core::datatypes::TimeInt) -> Self {
        Self::new_temporal(time.0)
    }
}

impl std::ops::Neg for TimeInt {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        match self.0 {
            Some(t) => Self(Some(-t)),
            None => self,
        }
    }
}

impl std::ops::Add for TimeInt {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        match (self.0, rhs.0) {
            // temporal + temporal = temporal
            (Some(lhs), Some(rhs)) => Self(Some(
                NonMinI64::new(lhs.get().saturating_add(rhs.get())).unwrap_or(NonMinI64::MIN),
            )),
            // static + anything = static
            _ => Self(None),
        }
    }
}

impl std::ops::Sub for TimeInt {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        match (self.0, rhs.0) {
            // temporal + temporal = temporal
            (Some(lhs), Some(rhs)) => Self(Some(
                NonMinI64::new(lhs.get().saturating_sub(rhs.get())).unwrap_or(NonMinI64::MIN),
            )),
            // static - anything = static
            _ => Self(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturated_temporal() {
        assert_eq!(TimeInt::saturated_temporal_i64(i64::MIN), TimeInt::MIN);
        assert_eq!(TimeInt::saturated_temporal_i64(i64::MIN + 1), TimeInt::MIN);
        assert_eq!(TimeInt::saturated_temporal_i64(i64::MAX), TimeInt::MAX);
        assert_eq!(
            TimeInt::saturated_temporal_i64(i64::MAX - 1),
            TimeInt::new_temporal(i64::MAX - 1)
        );
    }
}
