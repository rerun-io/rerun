use crate::{time::Time, Duration, NonMinI64, TryFromIntError};

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
}

impl std::fmt::Debug for TimeInt {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(NonMinI64::MIN) => f
                .debug_tuple("TimeInt::MIN")
                .field(&NonMinI64::MIN)
                .finish(),
            Some(NonMinI64::MAX) => f
                .debug_tuple("TimeInt::MAX")
                .field(&NonMinI64::MAX)
                .finish(),
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
    /// the timeless APIs.
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
    pub fn from_milliseconds(millis: NonMinI64) -> Self {
        Self::new_temporal(millis.get().saturating_mul(1_000_000))
    }

    /// For time timelines.
    #[inline]
    pub fn from_seconds(seconds: NonMinI64) -> Self {
        Self::new_temporal(seconds.get().saturating_mul(1_000_000_000))
    }

    /// For sequence timelines.
    #[inline]
    pub fn from_sequence(sequence: NonMinI64) -> Self {
        Self(Some(sequence))
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

impl TryFrom<Time> for TimeInt {
    type Error = TryFromIntError;

    #[inline]
    fn try_from(t: Time) -> Result<Self, Self::Error> {
        let Some(t) = NonMinI64::new(t.nanos_since_epoch()) else {
            return Err(TryFromIntError);
        };
        Ok(Self(Some(t)))
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
