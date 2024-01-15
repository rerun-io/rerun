use std::ops::RangeInclusive;

use crate::{TimeInt, TimeReal};

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimeRange {
    pub min: TimeInt,
    pub max: TimeInt,
}

impl TimeRange {
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

    #[inline]
    pub fn new(min: TimeInt, max: TimeInt) -> Self {
        Self { min, max }
    }

    #[inline]
    pub fn point(value: impl Into<TimeInt>) -> Self {
        let value = value.into();
        Self {
            min: value,
            max: value,
        }
    }

    /// The amount of time or sequences covered by this range.
    #[inline]
    pub fn abs_length(&self) -> u64 {
        self.min.as_i64().abs_diff(self.max.as_i64())
    }

    #[inline]
    pub fn center(&self) -> TimeInt {
        self.min + TimeInt::from((self.abs_length() / 2) as i64)
    }

    #[inline]
    pub fn contains(&self, time: TimeInt) -> bool {
        self.min <= time && time <= self.max
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
}

impl re_types_core::SizeBytes for TimeRange {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

impl From<TimeRange> for RangeInclusive<TimeInt> {
    fn from(range: TimeRange) -> RangeInclusive<TimeInt> {
        range.min..=range.max
    }
}

impl From<&TimeRange> for RangeInclusive<TimeInt> {
    fn from(range: &TimeRange) -> RangeInclusive<TimeInt> {
        range.min..=range.max
    }
}

// ----------------------------------------------------------------------------

/// Like [`TimeRange`], but using [`TimeReal`] for improved precision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimeRangeF {
    pub min: TimeReal,
    pub max: TimeReal,
}

impl TimeRangeF {
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

    // pub fn add(&mut self, value: TimeReal) {
    //     self.min = self.min.min(value);
    //     self.max = self.max.max(value);
    // }

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

impl From<TimeRangeF> for RangeInclusive<TimeReal> {
    fn from(range: TimeRangeF) -> RangeInclusive<TimeReal> {
        range.min..=range.max
    }
}

impl From<&TimeRangeF> for RangeInclusive<TimeReal> {
    fn from(range: &TimeRangeF) -> RangeInclusive<TimeReal> {
        range.min..=range.max
    }
}

impl From<TimeRange> for TimeRangeF {
    fn from(range: TimeRange) -> Self {
        Self::new(range.min, range.max)
    }
}
