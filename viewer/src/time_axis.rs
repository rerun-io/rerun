use std::collections::{BTreeMap, BTreeSet};

use log_types::TimeValue;

use crate::misc::TimePoints;

/// All the different time sources split into separate [`TimeSourceAxis`].
pub(crate) struct TimeSourceAxes {
    pub sources: BTreeMap<String, TimeSourceAxis>,
}

/// A piece-wise linear view of a single time source.
///
/// It is piece-wise linear because we sometimes have huge gaps in the data,
/// and we want to present a compressed view of it.
#[derive(Clone, Debug)]
pub(crate) struct TimeSourceAxis {
    pub segments: vec1::Vec1<TimeSegment>,
}

/// A linear segment of a time axis.
#[derive(Clone, Debug)]
pub(crate) struct TimeSegment {
    /// Never empty.
    pub values: BTreeSet<TimeValue>,
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

impl TimeSourceAxis {
    pub fn new(values: &BTreeSet<TimeValue>) -> Self {
        assert!(!values.is_empty());
        let mut values_it = values.iter();
        let mut latest_value = *values_it.next().unwrap();
        let mut segments = vec1::vec1![TimeSegment::new(latest_value)];

        for &new_value in values_it {
            if is_close(latest_value, new_value) {
                segments.last_mut().add(new_value);
            } else {
                segments.push(TimeSegment::new(new_value));
            }
            latest_value = new_value;
        }

        Self { segments }
    }

    // pub fn min(&self) -> TimeValue {
    //     self.segments.first().min()
    // }

    // pub fn max(&self) -> TimeValue {
    //     self.segments.last().max()
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

impl TimeSegment {
    pub fn new(value: TimeValue) -> Self {
        Self {
            values: [value].into(),
        }
    }

    pub fn add(&mut self, value: TimeValue) {
        self.values.insert(value);
    }

    pub fn min(&self) -> TimeValue {
        *self.values.iter().next().unwrap()
    }

    pub fn max(&self) -> TimeValue {
        *self.values.iter().rev().next().unwrap()
    }

    /// Where in the range is this value?
    pub fn lerp_t(&self, value: TimeValue) -> Option<f32> {
        value.lerp_t(self.min()..=self.max())
    }
}
