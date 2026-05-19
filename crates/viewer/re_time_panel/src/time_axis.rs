use re_log_types::{AbsoluteTimeRange, TimeInt, TimeType};
use vec1::Vec1;

/// A piece-wise linear view of a single timeline.
///
/// It is piece-wise linear because we sometimes have big gaps in the data
/// which we collapse in order to present a compressed view of the data.
#[derive(Clone, Debug)]
pub(crate) struct TimelineAxis {
    pub ranges: Vec1<AbsoluteTimeRange>,
}

impl TimelineAxis {
    /// Create a new `TimelineAxis` from sorted, non-overlapping chunk time ranges.
    ///
    /// `chunk_ranges` must be non-empty, sorted by start time, and non-overlapping.
    #[inline]
    pub fn new(time_type: TimeType, data_ranges: &[AbsoluteTimeRange]) -> Self {
        re_tracing::profile_function!();
        assert!(!data_ranges.is_empty());
        let gap_threshold = gap_size_heuristic(time_type, data_ranges);

        Self {
            ranges: create_ranges(data_ranges, gap_threshold),
        }
    }

    /// Total uncollapsed time within a certain bound.
    #[inline]
    pub fn sum_time_lengths(&self) -> u64 {
        self.ranges.iter().map(|t| t.abs_length()).sum()
    }
}

/// First determine the threshold for when a gap should be closed.
/// Sometimes, looking at data spanning milliseconds, a single second pause can be an eternity.
/// When looking at data recorded over hours, a few minutes of pause may be nothing.
/// We also don't want to produce a timeline of only gaps.
/// Finding a perfect heuristic is impossible, but we do our best!
fn gap_size_heuristic(time_type: TimeType, chunk_ranges: &[AbsoluteTimeRange]) -> u64 {
    re_tracing::profile_function!();

    assert!(!chunk_ranges.is_empty());

    let num_ranges = chunk_ranges.len();

    if num_ranges <= 2 {
        return u64::MAX;
    }

    let total_time_span = chunk_ranges
        .first()
        .expect("non-empty")
        .min()
        .as_i64()
        .abs_diff(chunk_ranges.last().expect("non-empty").max().as_i64());

    if total_time_span == 0 {
        return u64::MAX;
    }

    // Don't collapse too many gaps, because then the timeline is all gaps!
    let max_collapses = ((chunk_ranges.len() - 1) / 3).min(20);

    // We start off by a minimum gap size - any gap smaller than this will never be collapsed.
    // This is partially an optimization, and partially something that "feels right".
    let min_gap_size: u64 = match time_type {
        TimeType::Sequence => 9,
        TimeType::DurationNs | TimeType::TimestampNs => {
            TimeInt::from_millis(100.try_into().expect("100 fits in NonMinI64")).as_i64() as _
        }
    };

    // Collect all gaps between consecutive chunk ranges that are larger than our minimum.
    let mut gap_sizes: Vec<u64> = chunk_ranges
        .windows(2)
        .inspect(|w| {
            assert!(w[0].min <= w[0].max);
            assert!(w[1].min <= w[1].max);
        })
        .inspect(|w| assert!(w[0].max < w[1].min, "{:?} < {:?}", w[0], w[1]))
        .map(|w| w[0].max.as_i64().abs_diff(w[1].min.as_i64()))
        .filter(|&gap| gap > min_gap_size)
        .collect();
    gap_sizes.sort_unstable();

    // Only collapse gaps that take up a significant portion of the total time,
    // measured as the fraction of the total time that the gap represents.
    let min_collapse_fraction = (2.0 / (num_ranges - 1) as f64).max(0.35);

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

/// Collapse any gaps larger or equal to the given threshold.
fn create_ranges(
    chunk_ranges: &[AbsoluteTimeRange],
    gap_threshold: u64,
) -> vec1::Vec1<AbsoluteTimeRange> {
    re_tracing::profile_function!();
    let first = chunk_ranges[0];
    let mut ranges = vec1::vec1![first];

    for &range in &chunk_ranges[1..] {
        if ranges.last().max().as_i64().abs_diff(range.min().as_i64()) < gap_threshold {
            // Join with previous range:
            let new_max = ranges.last().max().max(range.max());
            ranges.last_mut().set_max(new_max);
        } else {
            // New range:
            ranges.push(range);
        }
    }

    ranges
}

#[cfg(test)]
mod tests {
    use re_chunk_store::AbsoluteTimeRange;

    use super::*;

    fn ranges(times: &[i64]) -> vec1::Vec1<AbsoluteTimeRange> {
        let mut chunk_ranges: Vec<AbsoluteTimeRange> = times
            .iter()
            .map(|&t| AbsoluteTimeRange::new(t, t))
            .collect();
        chunk_ranges.sort_by_key(|r| r.min());
        TimelineAxis::new(TimeType::Sequence, &chunk_ranges).ranges
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
