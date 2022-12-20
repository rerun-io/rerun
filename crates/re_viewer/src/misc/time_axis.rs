use std::collections::BTreeMap;

use itertools::Itertools as _;

use re_log_types::{Duration, TimeInt, TimeRange, TimeType};

/// A piece-wise linear view of a single timeline.
///
/// It is piece-wise linear because we sometimes have huge gaps in the data,
/// and we want to present a compressed view of it.
#[derive(Clone, Debug)]
pub(crate) struct TimelineAxis {
    pub ranges: vec1::Vec1<TimeRange>,
}

impl TimelineAxis {
    pub fn new<T>(time_type: TimeType, times: &BTreeMap<TimeInt, T>) -> Self {
        crate::profile_function!();

        // in seconds or sequences
        let time_abs_diff = |a: TimeInt, b: TimeInt| -> f64 {
            let abs_diff = (a - b).abs();
            match time_type {
                TimeType::Sequence => abs_diff.as_f64(),
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
            times
                .keys()
                .tuple_windows()
                .map(|(a, b)| time_abs_diff(*a, *b))
                .filter(|&gap_size| gap_size >= MIN_GAP_SIZE)
                .filter(|&gap_size| !gap_size.is_nan())
                .collect_vec()
        };

        gap_sizes.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());

        let mut gap_threshold = gap_sizes
            .first()
            .copied()
            .filter(|&gap_size| gap_size < 10_000.0 * MIN_GAP_SIZE) // exclude huge jumps, e.g. from -∞ (TimeInt::Beginning)
            .unwrap_or(MIN_GAP_SIZE);
        {
            crate::profile_scope!("expand_gap_threshold");
            for gap in gap_sizes {
                if gap >= gap_threshold * 2.0 {
                    break; // much bigger gap than anything before, let's use this
                } else if gap > gap_threshold {
                    gap_threshold *= 2.0;
                }
            }
        }

        // ----

        crate::profile_scope!("create_ranges");
        let mut values_it = times.keys();
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
