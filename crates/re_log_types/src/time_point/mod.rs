use std::collections::BTreeMap;

pub mod timeline;
pub use timeline::*;

// ----------------------------------------------------------------------------

/// A point in time.
///
/// It can be represented by [`Time`], a sequence index, or a mix of several things.
///
/// If this is empty, the data is _timeless_.
/// Timeless data will show up on all timelines, past and future,
/// and will hit all time queries. In other words, it is always there.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimePoint(pub BTreeMap<Timeline, TimeInt>);

impl TimePoint {
    /// Logging to this time means the data will show upp in all timelines,
    /// past and future. The time will be [`TimeInt::BEGINNING`], meaning it will
    /// always be in range for any time query.
    pub fn timeless() -> Self {
        Self::default()
    }

    #[inline]
    pub fn is_timeless(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> {
        self.0.keys()
    }
}

// ----------------------------------------------------------------------------

/// The type of a [`TimeInt`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, num_derive::FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TimeType {
    /// Normal wall time.
    Time,

    /// Used e.g. for frames in a film.
    Sequence,
}

impl TimeType {
    fn hash(&self) -> u64 {
        match self {
            Self::Time => 0,
            Self::Sequence => 1,
        }
    }

    pub fn format(&self, time_int: TimeInt) -> String {
        if time_int <= TimeInt::BEGINNING {
            "-∞".into()
        } else {
            match self {
                Self::Time => Time::from(time_int).format(),
                Self::Sequence => format!("#{}", time_int.0),
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// Either nanoseconds or sequence numbers.
///
/// Must be matched with a [`TimeType`] to know what.
///
/// Used both for time points and durations.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimeInt(i64);

impl TimeInt {
    /// The beginning of time.
    ///
    /// Special value used for timeless data.
    ///
    /// NOTE: this is not necessarily [`i64::MIN`].
    // The reason we don't use i64::MIN is because in the time panel we need
    // to be able to pan to before the `TimeInt::BEGINNING`, and so we need
    // a bit of leeway.
    pub const BEGINNING: TimeInt = TimeInt(i64::MIN / 2);

    pub const MAX: TimeInt = TimeInt(i64::MAX);

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
    fn from(int: TimeInt) -> Self {
        Self::from_ns_since_epoch(int.as_i64())
    }
}

impl From<TimeInt> for Duration {
    fn from(int: TimeInt) -> Self {
        Self::from_nanos(int.as_i64())
    }
}

impl std::ops::Neg for TimeInt {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(self.0.saturating_neg())
    }
}

impl std::ops::Add for TimeInt {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl std::ops::Sub for TimeInt {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl std::ops::AddAssign for TimeInt {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl std::ops::SubAssign for TimeInt {
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

// ----------------------------------------------------------------------------

#[inline]
pub fn time_point(
    fields: impl IntoIterator<Item = (&'static str, TimeType, TimeInt)>,
) -> TimePoint {
    TimePoint(
        fields
            .into_iter()
            .map(|(name, tt, ti)| (Timeline::new(name, tt), ti))
            .collect(),
    )
}
