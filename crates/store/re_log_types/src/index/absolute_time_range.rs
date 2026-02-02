use std::ops::RangeInclusive;

use crate::{TimeInt, TimeReal};

// ----------------------------------------------------------------------------

/// An absolute time range using [`TimeInt`].
///
/// Can be resolved from [`re_types_core::datatypes::TimeRange`] (which *may* have relative bounds) using a given timeline & cursor.
///
/// Should not include [`TimeInt::STATIC`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct AbsoluteTimeRange {
    pub min: TimeInt,
    pub max: TimeInt,
}

impl AbsoluteTimeRange {
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

    /// Creates a new temporal [`AbsoluteTimeRange`].
    ///
    /// The returned range is guaranteed to never include [`TimeInt::STATIC`].
    #[inline]
    pub fn new(min: impl TryInto<TimeInt>, max: impl TryInto<TimeInt>) -> Self {
        let min = TimeInt::saturated_temporal(min);
        let max = TimeInt::saturated_temporal(max);
        Self { min, max }
    }

    /// The returned range is guaranteed to never include [`TimeInt::STATIC`].
    #[inline]
    pub fn point(time: impl TryInto<TimeInt>) -> Self {
        let time = TimeInt::saturated_temporal(time);
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
        let time = TimeInt::saturated_temporal(time);
        self.min = time;
    }

    /// Overwrites the end bound of the range.
    ///
    /// The resulting range is guaranteed to never include [`TimeInt::STATIC`].
    #[inline]
    pub fn set_max(&mut self, time: impl TryInto<TimeInt>) {
        let time = TimeInt::saturated_temporal(time);
        self.max = time;
    }

    /// The amount of time or sequences covered by this range.
    #[inline]
    pub fn abs_length(&self) -> u64 {
        self.min.as_i64().abs_diff(self.max.as_i64())
    }

    #[inline]
    pub fn center(&self) -> TimeInt {
        self.min.midpoint(self.max)
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
    pub fn intersection(&self, other: Self) -> Option<Self> {
        self.intersects(other).then(|| Self {
            min: self.min.max(other.min),
            max: self.max.min(other.max),
        })
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

impl re_byte_size::SizeBytes for AbsoluteTimeRange {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl From<AbsoluteTimeRange> for RangeInclusive<TimeInt> {
    fn from(range: AbsoluteTimeRange) -> Self {
        range.min..=range.max
    }
}

// ----------------------------------------------------------------------------

/// Like [`AbsoluteTimeRange`], but using [`TimeReal`] for improved precision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct AbsoluteTimeRangeF {
    pub min: TimeReal,
    pub max: TimeReal,
}

impl AbsoluteTimeRangeF {
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

    /// Returns the point in the center of the range.
    pub fn center(&self) -> TimeReal {
        self.min.midpoint(self.max)
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

    /// Creates an [`AbsoluteTimeRange`] from self by rounding the start
    /// of the range down, and rounding the end of the range up.
    pub fn to_int(self) -> AbsoluteTimeRange {
        AbsoluteTimeRange::new(self.min.floor(), self.max.ceil())
    }
}

impl From<AbsoluteTimeRangeF> for RangeInclusive<TimeReal> {
    fn from(range: AbsoluteTimeRangeF) -> Self {
        range.min..=range.max
    }
}

impl From<&AbsoluteTimeRangeF> for RangeInclusive<TimeReal> {
    fn from(range: &AbsoluteTimeRangeF) -> Self {
        range.min..=range.max
    }
}

impl From<AbsoluteTimeRange> for AbsoluteTimeRangeF {
    fn from(range: AbsoluteTimeRange) -> Self {
        Self::new(range.min, range.max)
    }
}
