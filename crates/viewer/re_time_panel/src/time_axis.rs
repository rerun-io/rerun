// TODO(#6330): remove unwrap()
#![expect(clippy::unwrap_used)]

use re_entity_db::TimeHistogram;
use re_log_types::{AbsoluteTimeRange, TimeInt, TimeType};
use vec1::Vec1;

#[derive(Clone, Copy, Debug)]
pub struct LinearTimeRange {
    pub data_considered_valid: bool,
    pub time_range: AbsoluteTimeRange,
}

impl LinearTimeRange {
    pub fn new_valid(time_range: AbsoluteTimeRange) -> Self {
        Self {
            data_considered_valid: true,
            time_range,
        }
    }

    pub fn new_invalid(time_range: AbsoluteTimeRange) -> Self {
        Self {
            data_considered_valid: false,
            time_range,
        }
    }
}

/// A piece-wise linear view of a single timeline.
///
/// It is piece-wise linear because we sometimes have big gaps in the data
/// which we collapse in order to present a compressed view of the data.
#[derive(Clone, Debug)]
pub(crate) struct TimelineAxis {
    pub ranges: Vec1<LinearTimeRange>,
}

impl TimelineAxis {
    /// Determines all ranges that are in-range on the timeline and where there's gaps.
    ///
    /// Usually this is just everywhere where there's data, but this may be further cut down by
    /// `valid_time_ranges` which allows to mark things being outside even if there's data available.
    #[inline]
    pub fn new(
        time_type: TimeType,
        times: &TimeHistogram,
        valid_time_ranges: &[AbsoluteTimeRange],
    ) -> Self {
        re_tracing::profile_function!();
        assert!(!times.is_empty());
        let gap_threshold = gap_size_heuristic(time_type, times);
        let ranges = create_ranges(times, gap_threshold);

        // Chop further along valid ranges if needed. (most of the time everything is marked as valid)
        let ranges = subdivide_ranges_on_validity(&ranges, valid_time_ranges);

        Self { ranges }
    }

    /// Total uncollapsed time.
    #[inline]
    pub fn sum_time_lengths(&self) -> u64 {
        // TODO: invalid handling
        self.ranges.iter().map(|t| t.time_range.abs_length()).sum()
    }

    #[inline]
    pub fn min_valid(&self) -> TimeInt {
        self.ranges
            .iter()
            .find_map(|t| t.data_considered_valid.then_some(t.time_range.min))
            .unwrap_or(self.ranges.first().time_range.min)
    }
}

/// First determine the threshold for when a gap should be closed.
/// Sometimes, looking at data spanning milliseconds, a single second pause can be an eternity.
/// When looking at data recorded over hours, a few minutes of pause may be nothing.
/// We also don't want to produce a timeline of only gaps.
/// Finding a perfect heuristic is impossible, but we do our best!
fn gap_size_heuristic(time_type: TimeType, times: &TimeHistogram) -> u64 {
    re_tracing::profile_function!();

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
        TimeType::DurationNs | TimeType::TimestampNs => {
            TimeInt::from_millis(100.try_into().unwrap()).as_i64() as _
        }
    };
    // Collect all gaps larger than our minimum gap size.
    let mut gap_sizes =
        collect_candidate_gaps(times, min_gap_size, max_collapses).unwrap_or_default();
    gap_sizes.sort_unstable();

    // Only collapse gaps that take up a significant portion of the total time,
    // measured as the fraction of the total time that the gap represents.
    let min_collapse_fraction = (2.0 / (times.total_count() - 1) as f64).max(0.35);

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

/// Returns `None` to signal an abort.
fn collect_candidate_gaps(
    times: &TimeHistogram,
    min_gap_size: u64,
    max_collapses: usize,
) -> Option<Vec<u64>> {
    re_tracing::profile_function!();
    // We want this to be fast, even when we have _a lot_ of times.
    // `TimeHistogram::range` has a granularity argument:
    // - if it make it too small, we get too many gaps and run very slow
    // - if it is too large, we will miss gaps that could be important.
    // So we start with a large granularity, and then we reduce it until we get enough gaps.
    // This ensures a logarithmic runtime.

    let max_gap_size = times.max_key().unwrap() - times.min_key().unwrap();
    let mut granularity = max_gap_size as u64;

    let mut gaps = collect_gaps_with_granularity(times, granularity, min_gap_size)?;
    while gaps.len() < max_collapses && min_gap_size < granularity {
        granularity /= 2;
        gaps = collect_gaps_with_granularity(times, granularity, min_gap_size)?;
    }
    Some(gaps)
}

/// Returns `None` to signal an abort.
fn collect_gaps_with_granularity(
    times: &TimeHistogram,
    granularity: u64,
    min_gap_size: u64,
) -> Option<Vec<u64>> {
    re_tracing::profile_function!();

    let mut non_gap_time_span = 0;

    let mut gaps = vec![];
    let mut last_range: Option<re_int_histogram::RangeI64> = None;

    for (range, _count) in times.range(.., granularity) {
        non_gap_time_span += range.length();

        if let Some(last_range) = last_range {
            let gap_size = last_range.max.abs_diff(range.min);
            if min_gap_size < gap_size {
                gaps.push(gap_size);
            }
        }
        last_range = Some(range);
    }

    if min_gap_size * 100 < non_gap_time_span {
        // If the gap is such a small fracion of the total time, we don't care about it,
        // and we abort the gap-search, which is an important early-out.
        return None;
    }

    Some(gaps)
}

/// Collapse any gaps larger or equals to the given threshold.
fn create_ranges(times: &TimeHistogram, gap_threshold: u64) -> vec1::Vec1<AbsoluteTimeRange> {
    re_tracing::profile_function!();
    let mut it = times.range(.., gap_threshold);
    let first_range = it.next().unwrap().0;
    let mut ranges = vec1::vec1![AbsoluteTimeRange::new(first_range.min, first_range.max,)];

    for (new_range, _count) in it {
        if ranges.last_mut().max().as_i64().abs_diff(new_range.min) < gap_threshold {
            // join previous range:
            ranges.last_mut().set_max(new_range.max);
        } else {
            // new range:
            ranges.push(AbsoluteTimeRange::new(new_range.min, new_range.max));
        }
    }

    ranges
}

fn subdivide_ranges_on_validity(
    ranges: &[AbsoluteTimeRange],
    valid_time_ranges: &[AbsoluteTimeRange],
) -> Vec1<LinearTimeRange> {
    let mut new_ranges: Vec<LinearTimeRange> = Vec::new();

    // We expect very few valid ranges, so not worrying for now about optimizing this.
    for source_range in ranges {
        for valid_range in valid_time_ranges {
            let Some(intersection_range) = source_range.intersection(*valid_range) else {
                if source_range.max < valid_range.min {
                    break;
                }
                continue;
            };

            // Everything since the last range is invalid.
            // (Yes, invalid ranges can follow after invalid ranges if there was time gap in between!)
            let invalid_range_start = if let Some(last) = new_ranges.last_mut() {
                last.time_range.max
            } else {
                source_range.min
            };
            if invalid_range_start != intersection_range.min {
                new_ranges.push(LinearTimeRange::new_invalid(AbsoluteTimeRange::new(
                    invalid_range_start,
                    intersection_range.min,
                )));
            }

            new_ranges.push(LinearTimeRange::new_valid(intersection_range));
        }

        // Left-overs that need to be marked invalid?
        if let Some(last_range) = new_ranges.last_mut().copied() {
            if last_range.time_range.max < source_range.max {
                new_ranges.push(LinearTimeRange::new_invalid(AbsoluteTimeRange::new(
                    last_range.time_range.max,
                    source_range.max,
                )));
            }
        }
    }

    Vec1::try_from_vec(new_ranges).unwrap_or_else(|_| {
        vec1::vec1![LinearTimeRange {
            time_range: AbsoluteTimeRange::EMPTY,
            data_considered_valid: false,
        }]
    })
}

// TODO:
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use re_chunk_store::AbsoluteTimeRange;
//     use re_log_types::TimeInt;

//     fn ranges(times: &[i64]) -> vec1::Vec1<AbsoluteTimeRange> {
//         let mut time_histogram = TimeHistogram::default();
//         for &time in times {
//             time_histogram.increment(time, 1);
//         }
//         TimelineAxis::new(
//             TimeType::Sequence,
//             &time_histogram,
//             &[AbsoluteTimeRange::EVERYTHING],
//         )
//         .ranges
//     }

//     #[test]
//     fn test_time_axis() {
//         assert_eq!(1, ranges(&[1]).len());
//         assert_eq!(1, ranges(&[1, 2, 3, 4]).len());
//         assert_eq!(1, ranges(&[10, 20, 30, 40]).len());
//         assert_eq!(1, ranges(&[1, 2, 3, 11, 12, 13]).len(), "Too small gap");
//         assert_eq!(2, ranges(&[10, 20, 30, 110, 120, 130]).len());
//         assert_eq!(1, ranges(&[10, 1000]).len(), "not enough numbers");
//         assert_eq!(
//             2,
//             ranges(&[
//                 i64::MIN / 2,
//                 1_000_000_000,
//                 2_000_000_000,
//                 3_000_000_000,
//                 4_000_000_000,
//                 5_000_000_000,
//                 6_000_000_000,
//             ])
//             .len()
//         );
//         assert_eq!(
//             3,
//             ranges(&[
//                 i64::MIN / 2,
//                 1_000_000_000,
//                 2_000_000_000,
//                 3_000_000_000,
//                 4_000_000_000,
//                 5_000_000_000,
//                 100_000_000_000,
//             ])
//             .len()
//         );
//     }

//     #[test]
//     fn test_cut_up_along_valid_ranges() {
//         // single range that doesn't need cutting
//         let ranges = [AbsoluteTimeRange::new(
//             TimeInt::new_temporal(10),
//             TimeInt::new_temporal(20),
//         )];
//         let valid_ranges = [AbsoluteTimeRange::EVERYTHING];
//         let result = cut_up_along_valid_ranges(&ranges, &valid_ranges);
//         assert_eq!(result, ranges);

//         // cutting a range in the middle
//         let ranges = [AbsoluteTimeRange::new(
//             TimeInt::new_temporal(0),
//             TimeInt::new_temporal(100),
//         )];
//         let valid_ranges = [AbsoluteTimeRange::new(
//             TimeInt::new_temporal(20),
//             TimeInt::new_temporal(80),
//         )];
//         let result = cut_up_along_valid_ranges(&ranges, &valid_ranges);
//         assert_eq!(result, valid_ranges);

//         // multiple valid ranges creating multiple cuts
//         let ranges = [AbsoluteTimeRange::new(
//             TimeInt::new_temporal(0),
//             TimeInt::new_temporal(100),
//         )];
//         let valid_ranges = [
//             AbsoluteTimeRange::new(TimeInt::new_temporal(10), TimeInt::new_temporal(30)),
//             AbsoluteTimeRange::new(TimeInt::new_temporal(70), TimeInt::new_temporal(90)),
//         ];
//         let result = cut_up_along_valid_ranges(&ranges, &valid_ranges);
//         assert_eq!(result, valid_ranges);

//         // no intersection (valid range outside data range)
//         let ranges = [AbsoluteTimeRange::new(
//             TimeInt::new_temporal(50),
//             TimeInt::new_temporal(60),
//         )];
//         let valid_ranges = [AbsoluteTimeRange::new(
//             TimeInt::new_temporal(0),
//             TimeInt::new_temporal(10),
//         )];
//         let result = cut_up_along_valid_ranges(&ranges, &valid_ranges);
//         assert_eq!(result, [AbsoluteTimeRange::EMPTY]);

//         // multiple data & valid ranges.
//         let ranges = [
//             AbsoluteTimeRange::new(TimeInt::new_temporal(0), TimeInt::new_temporal(50)),
//             AbsoluteTimeRange::new(TimeInt::new_temporal(100), TimeInt::new_temporal(150)),
//         ];
//         let valid_ranges = [AbsoluteTimeRange::new(
//             TimeInt::new_temporal(25),
//             TimeInt::new_temporal(125),
//         )];
//         let result = cut_up_along_valid_ranges(&ranges, &valid_ranges);
//         assert_eq!(
//             result,
//             [
//                 AbsoluteTimeRange::new(TimeInt::new_temporal(25), TimeInt::new_temporal(50)),
//                 AbsoluteTimeRange::new(TimeInt::new_temporal(100), TimeInt::new_temporal(125))
//             ]
//         );
//     }
// }
