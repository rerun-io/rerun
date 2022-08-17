use std::collections::BTreeSet;

use itertools::Itertools as _;

use re_log_types::{Duration, TimeInt, TimeType};

use super::TimeRange;

/// A piece-wise linear view of a single time source.
///
/// It is piece-wise linear because we sometimes have huge gaps in the data,
/// and we want to present a compressed view of it.
#[derive(Clone, Debug)]
pub(crate) struct TimeSourceAxis {
    pub ranges: vec1::Vec1<TimeRange>,
}

impl TimeSourceAxis {
    pub fn new(time_type: TimeType, values: &BTreeSet<TimeInt>) -> Self {
        crate::profile_function!();

        // in seconds or sequences
        let time_abs_diff = |a: TimeInt, b: TimeInt| -> f64 {
            let abs_diff = (a - b).abs();
            match time_type {
                TimeType::Sequence => abs_diff.to_f64(),
                TimeType::Time => Duration::from(abs_diff).as_secs_f64(),
            }
        };

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
                .map(|(a, b)| time_abs_diff(*a, *b))
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
            if time_abs_diff(*last_max, new_value) <= gap_threshold {
                *last_max = new_value; // join previous range
            } else {
                ranges.push(TimeRange::point(new_value)); // new range
            }
        }

        Self { ranges }
    }

    pub fn sum_time_lengths(&self) -> TimeInt {
        self.ranges.iter().map(|t| t.length()).sum()
    }

    // pub fn range(&self) -> TimeRange {
    //     TimeRange {
    //         min: self.min(),
    //         max: self.max(),
    //     }
    // }

    pub fn min(&self) -> TimeInt {
        self.ranges.first().min
    }

    // pub fn max(&self) -> TimeInt {
    //     self.ranges.last().max
    // }
}
