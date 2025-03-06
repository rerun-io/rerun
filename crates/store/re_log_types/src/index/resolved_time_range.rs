use std::ops::RangeInclusive;

use crate::{NonMinI64, TimeInt, TimeReal};

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ResolvedTimeRange {
    min: TimeInt,
    max: TimeInt,
}

impl ResolvedTimeRange {
    /// Contains no time at all.
    pub const EMPTY: Self = Self {
        min: TimeInt::MAX,
        max: TimeInt::MIN,
    };

    /// Contains all time.
    pub const EVERYTHING: Self = Self {
        min: TimeInt::MIN,
        max: TimeInt::MAX,
    };

    /// Creates a new temporal [`ResolvedTimeRange`].
    ///
    /// The returned range is guaranteed to never include [`TimeInt::STATIC`].
    #[inline]
    pub fn new(min: impl TryInto<TimeInt>, max: impl TryInto<TimeInt>) -> Self {
        let min = min.try_into().unwrap_or(TimeInt::MIN).max(TimeInt::MIN);
        let max = max.try_into().unwrap_or(TimeInt::MIN).max(TimeInt::MIN);
        Self { min, max }
    }

    /// The returned range is guaranteed to never include [`TimeInt::STATIC`].
    #[inline]
    pub fn point(time: impl TryInto<TimeInt>) -> Self {
        let time = time.try_into().unwrap_or(TimeInt::MIN).max(TimeInt::MIN);
        Self {
            min: time,
            max: time,
        }
    }

    #[inline]
    pub fn min(&self) -> TimeInt {
        self.min
    }

    #[inline]
    pub fn max(&self) -> TimeInt {
        self.max
    }

    /// Overwrites the start bound of the range.
    ///
    /// The resulting range is guaranteed to never include [`TimeInt::STATIC`].
    #[inline]
    pub fn set_min(&mut self, time: impl TryInto<TimeInt>) {
        let time = time.try_into().unwrap_or(TimeInt::MIN).max(TimeInt::MIN);
        self.min = time;
    }

    /// Overwrites the end bound of the range.
    ///
    /// The resulting range is guaranteed to never include [`TimeInt::STATIC`].
    #[inline]
    pub fn set_max(&mut self, time: impl TryInto<TimeInt>) {
        let time = time.try_into().unwrap_or(TimeInt::MIN).max(TimeInt::MIN);
        self.max = time;
    }

    /// The amount of time or sequences covered by this range.
    #[inline]
    pub fn abs_length(&self) -> u64 {
        self.min.as_i64().abs_diff(self.max.as_i64())
    }

    #[inline]
    pub fn center(&self) -> TimeInt {
        let center = NonMinI64::new((self.abs_length() / 2) as i64).unwrap_or(NonMinI64::MIN);
        self.min + TimeInt::from(center)
    }

    #[inline]
    pub fn contains(&self, time: TimeInt) -> bool {
        self.min <= time && time <= self.max
    }

    /// Does this range fully contain the other?
    #[inline]
    pub fn contains_range(&self, other: Self) -> bool {
        self.min <= other.min && other.max <= self.max
    }

    #[inline]
    pub fn intersects(&self, other: Self) -> bool {
        self.min <= other.max && self.max >= other.min
    }

    #[inline]
    pub fn union(&self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    pub fn from_relative_time_range(
        range: &re_types_core::datatypes::TimeRange,
        cursor: impl Into<re_types_core::datatypes::TimeInt>,
    ) -> Self {
        let cursor = cursor.into();

        let mut min = range.start.start_boundary_time(cursor);
        let mut max = range.end.end_boundary_time(cursor);

        if min > max {
            std::mem::swap(&mut min, &mut max);
        }

        Self::new(min, max)
    }
}

impl re_byte_size::SizeBytes for ResolvedTimeRange {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

// ----------------------------------------------------------------------------

/// Like [`ResolvedTimeRange`], but using [`TimeReal`] for improved precision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ResolvedTimeRangeF {
    pub min: TimeReal,
    pub max: TimeReal,
}

impl ResolvedTimeRangeF {
    #[inline]
    pub fn new(min: impl Into<TimeReal>, max: impl Into<TimeReal>) -> Self {
        Self {
            min: min.into(),
            max: max.into(),
        }
    }

    #[inline]
    pub fn point(value: impl Into<TimeReal>) -> Self {
        let value = value.into();
        Self {
            min: value,
            max: value,
        }
    }

    /// Inclusive
    pub fn contains(&self, value: TimeReal) -> bool {
        self.min <= value && value <= self.max
    }

    /// Where in the range is this value? Returns 0-1 if within the range.
    ///
    /// Returns <0 if before and >1 if after.
    pub fn inverse_lerp(&self, value: TimeReal) -> f64 {
        if self.min == self.max {
            0.5
        } else {
            (value - self.min).as_f64() / (self.max - self.min).as_f64()
        }
    }

    pub fn lerp(&self, t: f64) -> TimeReal {
        self.min + (self.max - self.min) * t
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.min == self.max
    }

    /// The amount of time or sequences covered by this range.
    #[inline]
    pub fn length(&self) -> TimeReal {
        self.max - self.min
    }
}

impl From<ResolvedTimeRangeF> for RangeInclusive<TimeReal> {
    fn from(range: ResolvedTimeRangeF) -> Self {
        range.min..=range.max
    }
}

impl From<&ResolvedTimeRangeF> for RangeInclusive<TimeReal> {
    fn from(range: &ResolvedTimeRangeF) -> Self {
        range.min..=range.max
    }
}

impl From<ResolvedTimeRange> for ResolvedTimeRangeF {
    fn from(range: ResolvedTimeRange) -> Self {
        Self::new(range.min, range.max)
    }
}
