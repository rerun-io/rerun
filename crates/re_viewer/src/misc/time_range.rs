use std::ops::RangeInclusive;

use re_log_types::TimeInt;

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct TimeRange {
    pub min: TimeInt,
    pub max: TimeInt,
}

impl TimeRange {
    pub fn new(min: TimeInt, max: TimeInt) -> Self {
        Self { min, max }
    }

    pub fn point(value: TimeInt) -> Self {
        Self {
            min: value,
            max: value,
        }
    }

    // pub fn add(&mut self, value: TimeInt) {
    //     self.min = self.min.min(value);
    //     self.max = self.max.max(value);
    // }

    /// Inclusive
    pub fn contains(&self, value: TimeInt) -> bool {
        self.min <= value && value <= self.max
    }

    /// Where in the range is this value? Returns 0-1 if within the range.
    ///
    /// Returns <0 if before and >1 if after.
    pub fn inverse_lerp(&self, value: TimeInt) -> f32 {
        if self.min == self.max {
            0.5
        } else {
            (value - self.min).as_f32() / (self.max - self.min).as_f32()
        }
    }

    pub fn lerp(&self, t: f32) -> TimeInt {
        let t = t as f64;
        let (min, max) = (self.min.as_f64(), self.max.as_f64());
        TimeInt::from((min + t * (max - min)).round() as i64)
    }

    /// The amount of time or sequences covered by this range.
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
