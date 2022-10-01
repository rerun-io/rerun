use std::ops::RangeInclusive;

use super::TimeReal;
use re_log_types::TimeInt;

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct TimeRange {
    pub min: TimeInt,
    pub max: TimeInt,
}

impl TimeRange {
    #[inline]
    pub fn new(min: impl Into<TimeInt>, max: impl Into<TimeInt>) -> Self {
        Self {
            min: min.into(),
            max: max.into(),
        }
    }

    #[inline]
    pub fn point(value: impl Into<TimeInt>) -> Self {
        let value = value.into();
        Self {
            min: value,
            max: value,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.min == self.max
    }

    /// The amount of time or sequences covered by this range.
    #[inline]
    pub fn length(&self) -> TimeInt {
        self.max - self.min
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

/// Like [`TimeRange`], but using [`TimeReal`] for improved precison.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct TimeRangeF {
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
    pub fn inverse_lerp(&self, value: TimeReal) -> f32 {
        if self.min == self.max {
            0.5
        } else {
            (value - self.min).as_f32() / (self.max - self.min).as_f32()
        }
    }

    pub fn lerp(&self, t: f32) -> TimeReal {
        self.min + (self.max - self.min) * t as f64
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

impl From<TimeRangeF> for TimeRange {
    fn from(range: TimeRangeF) -> Self {
        Self::new(range.min, range.max)
    }
}
