use std::collections::{BTreeMap, BTreeSet};

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

    // pub fn min(&self) -> TimeValue {
    //     self.ranges.first().min()
    // }

    // pub fn max(&self) -> TimeValue {
    //     self.ranges.last().max()
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

#[derive(Clone, Debug)]
pub(crate) struct TimeRange {
    pub min: TimeValue,
    pub max: TimeValue,
}

impl TimeRange {
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

    /// Where in the range is this value?
    pub fn lerp_t(&self, value: TimeValue) -> Option<f32> {
        value.lerp_t(self.min..=self.max)
    }

    /// The amount of time or sequences covered by this range.
    pub fn span(&self) -> Option<f64> {
        match (self.min, self.max) {
            (TimeValue::Time(min), TimeValue::Time(max)) => Some((max - min).as_secs_f64()),
            (TimeValue::Sequence(min), TimeValue::Sequence(max)) => Some((max - min) as f64),
            _ => None,
        }
    }
}
