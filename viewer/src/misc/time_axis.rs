use std::{
    collections::{BTreeMap, BTreeSet},
    ops::RangeInclusive,
};

use log_types::TimeValue;

use crate::misc::TimePoints;

/// All the different time sources split into separate [`TimeSourceAxis`].
pub(crate) struct TimeSourceAxes {
    pub sources: BTreeMap<String, TimeSourceAxis>,
}

impl TimeSourceAxes {
    pub fn new(time_axes: &TimePoints) -> Self {
        let sources = time_axes
            .0
            .iter()
            .map(|(name, values)| (name.clone(), TimeSourceAxis::new(values)))
            .collect();
        Self { sources }
    }
}

/// A piece-wise linear view of a single time source.
///
/// It is piece-wise linear because we sometimes have huge gaps in the data,
/// and we want to present a compressed view of it.
#[derive(Clone, Debug)]
pub(crate) struct TimeSourceAxis {
    pub ranges: vec1::Vec1<TimeRange>,
}

impl TimeSourceAxis {
    pub fn new(values: &BTreeSet<TimeValue>) -> Self {
        assert!(!values.is_empty());
        let mut values_it = values.iter();
        let mut latest_value = *values_it.next().unwrap();
        let mut ranges = vec1::vec1![TimeRange::point(latest_value)];

        for &new_value in values_it {
            if is_close(latest_value, new_value) {
                ranges.last_mut().add(new_value);
            } else {
                ranges.push(TimeRange::point(new_value));
            }
            latest_value = new_value;
        }

        Self { ranges }
    }

    pub fn sum_time_span(&self) -> f64 {
        self.ranges.iter().map(|t| t.span().unwrap_or(0.0)).sum()
    }

    // pub fn range(&self) -> TimeRange {
    //     TimeRange {
    //         min: self.min(),
    //         max: self.max(),
    //     }
    // }

    pub fn min(&self) -> TimeValue {
        self.ranges.first().min
    }

    // pub fn max(&self) -> TimeValue {
    //     self.ranges.last().max
    // }
}

fn is_close(a: TimeValue, b: TimeValue) -> bool {
    match (a, b) {
        (TimeValue::Sequence(_), TimeValue::Sequence(_)) => true,
        (TimeValue::Sequence(_), TimeValue::Time(_))
        | (TimeValue::Time(_), TimeValue::Sequence(_)) => false,
        (TimeValue::Time(a), TimeValue::Time(b)) => {
            (b - a).as_secs_f32().abs() < 2.0 // TODO: less hacky heuristic!
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct TimeRange {
    pub min: TimeValue,
    pub max: TimeValue,
}

impl TimeRange {
    pub fn new(min: TimeValue, max: TimeValue) -> Self {
        Self { min, max }
    }

    pub fn point(value: TimeValue) -> Self {
        Self {
            min: value,
            max: value,
        }
    }

    pub fn add(&mut self, value: TimeValue) {
        self.min = self.min.min(value);
        self.max = self.max.max(value);
    }

    /// Where in the range is this value? Returns 0-1 if within the range.
    /// Returns <0 if before, >1 if after, and `None` if the unit is wrong.
    pub fn lerp_t(&self, value: TimeValue) -> Option<f32> {
        fn lerp_t_i64(min: i64, value: i64, max: i64) -> f32 {
            if min == max {
                0.5
            } else {
                value.saturating_sub(min) as f32 / max.saturating_sub(min) as f32
            }
        }

        match (self.min, value, self.max) {
            (TimeValue::Time(min), TimeValue::Time(value), TimeValue::Time(max)) => {
                Some(lerp_t_i64(
                    min.nanos_since_epoch(),
                    value.nanos_since_epoch(),
                    max.nanos_since_epoch(),
                ))
            }
            (TimeValue::Sequence(min), TimeValue::Sequence(value), TimeValue::Sequence(max)) => {
                Some(lerp_t_i64(min as _, value as _, max as _))
            }
            _ => None,
        }
    }

    pub fn lerp(&self, t: f32) -> Option<TimeValue> {
        fn lerp_i64(range: RangeInclusive<i64>, t: f32) -> i64 {
            let (min, max) = (*range.start(), *range.end());
            min + ((max - min) as f64 * (t as f64)).round() as i64
        }

        match (self.min, self.max) {
            (TimeValue::Time(min), TimeValue::Time(max)) => {
                Some(TimeValue::Time(log_types::Time::lerp(min..=max, t)))
            }
            (TimeValue::Sequence(min), TimeValue::Sequence(max)) => {
                Some(TimeValue::Sequence(lerp_i64(min as _..=max as _, t) as _))
            }
            _ => None,
        }
    }

    /// The amount of time or sequences covered by this range.
    pub fn span(&self) -> Option<f64> {
        match (self.min, self.max) {
            (TimeValue::Time(min), TimeValue::Time(max)) => Some((max - min).as_nanos() as f64),
            (TimeValue::Sequence(min), TimeValue::Sequence(max)) => Some((max - min) as f64),
            _ => None,
        }
    }
}

impl From<TimeRange> for RangeInclusive<TimeValue> {
    fn from(range: TimeRange) -> RangeInclusive<TimeValue> {
        range.min..=range.max
    }
}

impl From<&TimeRange> for RangeInclusive<TimeValue> {
    fn from(range: &TimeRange) -> RangeInclusive<TimeValue> {
        range.min..=range.max
    }
}
