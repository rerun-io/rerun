use std::collections::BTreeMap;

use itertools::Itertools as _;

use re_log_types::{TimeInt, TimeRange, TimeType};

/// A piece-wise linear view of a single timeline.
///
/// It is piece-wise linear because we sometimes have big gaps in the data
/// which we collapse in order to  present a compressed view of the data.
#[derive(Clone, Debug)]
pub(crate) struct TimelineAxis {
    pub ranges: vec1::Vec1<TimeRange>,
}

impl TimelineAxis {
    pub fn new<T>(time_type: TimeType, times: &BTreeMap<TimeInt, T>) -> Self {
        crate::profile_function!();
        assert!(!times.is_empty());
        let gap_threshold = gap_size_heuristic(time_type, times);
        Self {
            ranges: create_ranges(times, gap_threshold),
        }
    }

    /// Total uncollapsed time.
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

// in seconds or nanos
fn time_abs_diff(a: TimeInt, b: TimeInt) -> u64 {
    a.as_i64().abs_diff(b.as_i64())
}

/// First determine the threshold for when a gap should be closed.
/// Sometimes, looking at data spanning milliseconds, a single second pause can be an eternity.
/// When looking at data recorded over hours, a few minutes of pause may be nothing.
/// We also don't want to produce a timeline of only gaps.
/// Finding a perfect heuristic is impossible, but we do our best!
fn gap_size_heuristic<T>(time_type: TimeType, times: &BTreeMap<TimeInt, T>) -> u64 {
    crate::profile_function!();

    assert!(!times.is_empty());

    if times.len() <= 2 {
        return u64::MAX;
    }

    let total_time_span = time_abs_diff(
        *times.first_key_value().unwrap().0,
        *times.last_key_value().unwrap().0,
    );

    // We start off by a minimum gap size - any gap smaller than this will never be collapsed.
    // This is partially an optimization, and partially something that "feels right".
    let min_gap_size: u64 = match time_type {
        TimeType::Sequence => 9,
        TimeType::Time => 100_000_000, // nanos!
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

    // Don't collapse too many gaps, because then the timeline is all gaps!
    let max_collapses: usize = ((times.len() - 1) / 3).min(20);

    // Only collapse gaps that take up a significant portion of the total time,
    // measured as the fraction of the total time that the gap represents.
    let min_collapse_fraction: f64 = (2.0 / (times.len() - 1) as f64).max(0.35);

    let mut gap_threshold = u64::MAX;
    let mut uncollapsed_time = total_time_span;

    // Go through the gaps, largest to smallest:
    for &gap in gap_sizes.iter().rev().take(max_collapses) {
        // How big is the gap relative to the total uncollapsed time?
        let gap_fraction = gap as f64 / uncollapsed_time as f64;
        if gap_fraction > min_collapse_fraction {
            // Collapse this gap
            gap_threshold = gap;
            uncollapsed_time -= gap;
        } else {
            break; // gap is too small to collapse, and so will all following gaps be
        }
    }

    gap_threshold
}

/// Collapse any gaps larger or equals to the given threshold.
fn create_ranges<T>(times: &BTreeMap<TimeInt, T>, gap_threshold: u64) -> vec1::Vec1<TimeRange> {
    crate::profile_function!();
    let mut values_it = times.keys();
    let mut ranges = vec1::vec1![TimeRange::point(*values_it.next().unwrap())];

    for &new_value in values_it {
        let last_max = &mut ranges.last_mut().max;
        if time_abs_diff(*last_max, new_value) < gap_threshold {
            *last_max = new_value; // join previous range
        } else {
            ranges.push(TimeRange::point(new_value)); // new range
        }
    }

    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use re_arrow_store::TimeRange;

    fn ranges(times: &[i64]) -> vec1::Vec1<TimeRange> {
        #[allow(clippy::zero_sized_map_values)]
        let times: BTreeMap<TimeInt, ()> = times
            .iter()
            .map(|&seq| (TimeInt::from_sequence(seq), ()))
            .collect();
        TimelineAxis::new(TimeType::Sequence, &times).ranges
    }

    #[test]
    fn test_time_axis() {
        assert_eq!(1, ranges(&[1]).len());
        assert_eq!(1, ranges(&[1, 2, 3, 4]).len());
        assert_eq!(1, ranges(&[10, 20, 30, 40]).len());
        assert_eq!(1, ranges(&[1, 2, 3, 11, 12, 13]).len(), "Too small gap");
        assert_eq!(2, ranges(&[10, 20, 30, 110, 120, 130]).len());
        assert_eq!(1, ranges(&[10, 1000]).len(), "not enough numbers");
    }
}
