use itertools::Itertools as _;

use re_data_store::TimeHistogram;
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
    pub fn new(time_type: TimeType, times: &TimeHistogram) -> Self {
        crate::profile_function!();
        assert!(!times.is_empty());
        let gap_threshold = gap_size_heuristic(time_type, times);
        Self {
            ranges: create_ranges(times, gap_threshold),
        }
    }

    /// Total uncollapsed time.
    pub fn sum_time_lengths(&self) -> u64 {
        self.ranges.iter().map(|t| t.abs_length()).sum()
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

/// First determine the threshold for when a gap should be closed.
/// Sometimes, looking at data spanning milliseconds, a single second pause can be an eternity.
/// When looking at data recorded over hours, a few minutes of pause may be nothing.
/// We also don't want to produce a timeline of only gaps.
/// Finding a perfect heuristic is impossible, but we do our best!
fn gap_size_heuristic(time_type: TimeType, times: &TimeHistogram) -> u64 {
    crate::profile_function!();

    assert!(!times.is_empty());

    if times.total_count() <= 2 {
        return u64::MAX;
    }

    let total_time_span = times.min_key().unwrap().abs_diff(times.max_key().unwrap());

    if total_time_span == 0 {
        return u64::MAX;
    }

    // We start off by a minimum gap size - any gap smaller than this will never be collapsed.
    // This is partially an optimization, and partially something that "feels right".
    let min_gap_size: u64 = match time_type {
        TimeType::Sequence => 9,
        TimeType::Time => TimeInt::from_milliseconds(100).as_i64() as _,
    };
    let cutoff_size = min_gap_size; // TODO(emilk): we could make the cutoff_size even larger, which will be faster when there is a lot of data points.

    // Collect all gaps larger than our minimum gap size.
    let mut gap_sizes = {
        crate::profile_scope!("collect_gaps");
        times
            .range(.., cutoff_size)
            .tuple_windows()
            .map(|((a, _), (b, _))| a.max.abs_diff(b.min))
            .filter(|&gap_size| min_gap_size < gap_size)
            .collect_vec()
    };
    gap_sizes.sort_unstable();

    // Don't collapse too many gaps, because then the timeline is all gaps!
    let max_collapses = ((times.total_count() - 1) / 3).min(20);

    // Only collapse gaps that take up a significant portion of the total time,
    // measured as the fraction of the total time that the gap represents.
    let min_collapse_fraction: f64 = (2.0 / (times.total_count() - 1) as f64).max(0.35);

    let mut gap_threshold = u64::MAX;
    let mut uncollapsed_time = total_time_span;

    // Go through the gaps, largest to smallest:
    for &gap in gap_sizes.iter().rev().take(max_collapses as _) {
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
fn create_ranges(times: &TimeHistogram, gap_threshold: u64) -> vec1::Vec1<TimeRange> {
    crate::profile_function!();
    let cutoff_size = 1; // TODO(emilk): take larger steps when possible, to speed up the case when we have many data points
    let mut it = times.range(.., cutoff_size);
    let first_range = it.next().unwrap().0;
    let mut ranges = vec1::vec1![TimeRange::new(
        first_range.min.into(),
        first_range.max.into()
    )];

    for (new_range, _count) in it {
        let last_max = &mut ranges.last_mut().max;
        if last_max.as_i64().abs_diff(new_range.min) < gap_threshold {
            // join previous range:
            *last_max = new_range.max.into();
        } else {
            // new range:
            ranges.push(TimeRange::new(new_range.min.into(), new_range.max.into()));
        }
    }

    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use re_arrow_store::TimeRange;

    fn ranges(times: &[i64]) -> vec1::Vec1<TimeRange> {
        let mut time_histogram = TimeHistogram::default();
        for &time in times {
            time_histogram.increment(time, 1);
        }
        TimelineAxis::new(TimeType::Sequence, &time_histogram).ranges
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
