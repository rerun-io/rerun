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

    // Don't collapse too many gaps, because then the timeline is all gaps!
    let max_collapses = ((times.total_count() - 1) / 3).min(20) as usize;

    // We start off by a minimum gap size - any gap smaller than this will never be collapsed.
    // This is partially an optimization, and partially something that "feels right".
    let min_gap_size: u64 = match time_type {
        TimeType::Sequence => 9,
        TimeType::Time => TimeInt::from_milliseconds(100).as_i64() as _,
    };
    // Collect all gaps larger than our minimum gap size.
    let mut gap_sizes = collect_candidate_gaps(times, min_gap_size, max_collapses);
    gap_sizes.sort_unstable();

    // Only collapse gaps that take up a significant portion of the total time,
    // measured as the fraction of the total time that the gap represents.
    let min_collapse_fraction: f64 = (2.0 / (times.total_count() - 1) as f64).max(0.35);

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

fn collect_candidate_gaps(
    times: &TimeHistogram,
    min_gap_size: u64,
    max_collapses: usize,
) -> Vec<u64> {
    crate::profile_function!();
    // We want this to be fast, even when we have _a lot_ of times.
    // `TimeHistogram::range` has a granularity argument:
    // - if it make it too small, we get too many gaps and run very slow
    // - if it is too large, we will miss gaps that could be important.
    // So we start with a large granularity, and then we reduce it until we get enough gaps.
    // This ensures a logarithmic runtime.

    let max_gap_size = times.max_key().unwrap() - times.min_key().unwrap();
    let mut granularity = max_gap_size as u64;

    let mut gaps = collect_gaps_with_granularity(times, granularity, min_gap_size);
    while gaps.len() < max_collapses && min_gap_size < granularity {
        granularity /= 2;
        gaps = collect_gaps_with_granularity(times, granularity, min_gap_size);
    }
    gaps
}

fn collect_gaps_with_granularity(
    times: &TimeHistogram,
    granularity: u64,
    min_gap_size: u64,
) -> Vec<u64> {
    crate::profile_function!();
    times
        .range(.., granularity)
        .tuple_windows()
        .map(|((a, _), (b, _))| a.max.abs_diff(b.min))
        .filter(|&gap_size| min_gap_size < gap_size)
        .collect_vec()
}

/// Collapse any gaps larger or equals to the given threshold.
fn create_ranges(times: &TimeHistogram, gap_threshold: u64) -> vec1::Vec1<TimeRange> {
    crate::profile_function!();
    let mut it = times.range(.., gap_threshold);
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
        assert_eq!(
            2,
            ranges(&[
                i64::MIN / 2,
                1_000_000_000,
                2_000_000_000,
                3_000_000_000,
                4_000_000_000,
                5_000_000_000,
                6_000_000_000,
            ])
            .len()
        );
        assert_eq!(
            3,
            ranges(&[
                i64::MIN / 2,
                1_000_000_000,
                2_000_000_000,
                3_000_000_000,
                4_000_000_000,
                5_000_000_000,
                100_000_000_000,
            ])
            .len()
        );
    }
}
