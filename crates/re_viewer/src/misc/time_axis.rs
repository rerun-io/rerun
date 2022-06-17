use std::{collections::BTreeSet, ops::RangeInclusive};

use itertools::Itertools;
use re_log_types::TimeValue;

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
        crate::profile_function!();

        /// in seconds or sequences
        fn time_diff(a: &TimeValue, b: &TimeValue) -> f64 {
            match (*a, *b) {
                (TimeValue::Sequence(a), TimeValue::Sequence(b)) => a.abs_diff(b) as f64,
                (TimeValue::Sequence(_), TimeValue::Time(_))
                | (TimeValue::Time(_), TimeValue::Sequence(_)) => f64::INFINITY,
                (TimeValue::Time(a), TimeValue::Time(b)) => (b - a).as_secs_f64().abs(),
            }
        }

        // First determine the threshold for when a gap should be closed.
        // Sometimes, looking at data spanning milliseconds, a single second pause can be an eternity.
        // When looking at data recorded over hours, a few minutes of pause may be nothing.
        // So we start with a small gap and keep expanding it while it decreases the number of gaps.

        /// measured in seconds or sequences.
        /// Anything at least this close are considered one thing.
        const MIN_GAP_SIZE: f64 = 1.0;

        let mut gap_sizes = {
            crate::profile_scope!("collect_gaps");
            values
                .iter()
                .tuple_windows()
                .map(|(a, b)| time_diff(a, b))
                .filter(|&gap_size| gap_size >= MIN_GAP_SIZE)
                .filter(|&gap_size| !gap_size.is_nan())
                .collect_vec()
        };

        gap_sizes.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());

        let mut gap_threshold = MIN_GAP_SIZE;
        for gap in gap_sizes {
            if gap >= gap_threshold * 2.0 {
                break; // much bigger gap than anything before, let's use this
            } else if gap > gap_threshold {
                gap_threshold *= 2.0;
            }
        }

        // ----

        crate::profile_scope!("create_ranges");
        let mut values_it = values.iter();
        let mut ranges = vec1::vec1![TimeRange::point(*values_it.next().unwrap())];

        for &new_value in values_it {
            let last_max = &mut ranges.last_mut().max;
            if time_diff(last_max, &new_value) <= gap_threshold {
                *last_max = new_value; // join previous range
            } else {
                ranges.push(TimeRange::point(new_value)); // new range
            }
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

    // pub fn add(&mut self, value: TimeValue) {
    //     self.min = self.min.min(value);
    //     self.max = self.max.max(value);
    // }

    /// Inclusive
    pub fn contains(&self, value: TimeValue) -> bool {
        self.min <= value && value <= self.max
    }

    /// Where in the range is this value? Returns 0-1 if within the range.
    /// Returns <0 if before, >1 if after, and `None` if the unit is wrong.
    pub fn inverse_lerp(&self, value: TimeValue) -> Option<f32> {
        fn inverse_lerp_i64(min: i64, value: i64, max: i64) -> f32 {
            if min == max {
                0.5
            } else {
                value.saturating_sub(min) as f32 / max.saturating_sub(min) as f32
            }
        }

        match (self.min, value, self.max) {
            (TimeValue::Time(min), TimeValue::Time(value), TimeValue::Time(max)) => {
                Some(inverse_lerp_i64(
                    min.nanos_since_epoch(),
                    value.nanos_since_epoch(),
                    max.nanos_since_epoch(),
                ))
            }
            (TimeValue::Sequence(min), TimeValue::Sequence(value), TimeValue::Sequence(max)) => {
                Some(inverse_lerp_i64(min, value, max))
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
                Some(TimeValue::Time(re_log_types::Time::lerp(min..=max, t)))
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

    /// Human-readable description of the time range _size_.
    pub fn format_size(&self) -> String {
        match (self.min, self.max) {
            (TimeValue::Time(min), TimeValue::Time(max)) => (max - min).to_string(),
            (TimeValue::Sequence(min), TimeValue::Sequence(max)) => {
                format!("{}", max.abs_diff(min))
            }
            _ => Default::default(),
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
