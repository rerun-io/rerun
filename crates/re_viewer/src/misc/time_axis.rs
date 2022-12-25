use std::collections::BTreeMap;

use itertools::Itertools as _;

use re_log_types::{TimeInt, TimeRange, TimeType};

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

        assert!(!times.is_empty());

        // in seconds or nanos
        let time_abs_diff = |a: TimeInt, b: TimeInt| -> u64 { a.as_i64().abs_diff(b.as_i64()) };

        // First determine the threshold for when a gap should be closed.
        // Sometimes, looking at data spanning milliseconds, a single second pause can be an eternity.
        // When looking at data recorded over hours, a few minutes of pause may be nothing.
        // So we start with a small gap and keep expanding it while it decreases the number of gaps.

        // Anything at least this close are considered one thing.
        // Measured in nanos or sequences.
        let min_gap_size: u64 = match time_type {
            TimeType::Sequence => 1,
            TimeType::Time => 1_000_000_000, // nanos!
        };

        // Collect all gaps larger than our minimum gap size.
        let mut gap_sizes = {
            crate::profile_scope!("collect_gaps");
            times
                .keys()
                .tuple_windows()
                .map(|(a, b)| time_abs_diff(*a, *b))
                .filter(|&gap_size| gap_size > min_gap_size)
                .collect_vec()
        };

        gap_sizes.sort_unstable();

        // We can probably improve these heuristics a bit.
        // Currently the gap-detector is only based on the sizes of gaps, not the sizes of runs.
        // If we have a hour-long run, it would make sense that the next gap must be quite big
        // for it to be contracted, yet we don't do anything like that yet.

        let mut gap_threshold = min_gap_size;

        // Progressively expand the gap threshold
        for gap in gap_sizes {
            if gap >= gap_threshold * 2 {
                break; // much bigger gap than anything before, let's stop here
            } else if gap > gap_threshold {
                gap_threshold *= 2;
            }
        }

        // ----
        // We calculated the threshold for creating gaps, so let's collect all the ranges:

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
