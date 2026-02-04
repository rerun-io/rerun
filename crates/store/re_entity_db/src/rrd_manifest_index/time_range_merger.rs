//! The goal of this utility is to create non-overlapping ranges without gaps that are
//! depending on a number of chunks to be fully loaded.

use std::{
    collections::BinaryHeap,
    ops::{Deref, DerefMut},
};

use ahash::HashSet;
use re_chunk::{ChunkId, TimeInt};
use re_log_types::AbsoluteTimeRange;
use re_tracing::profile_function;

#[derive(Clone)]
pub struct TimeRange {
    range: AbsoluteTimeRange,
    depends_on: HashSet<ChunkId>,
}

impl TimeRange {
    pub fn new(chunk: ChunkId, range: AbsoluteTimeRange) -> Self {
        Self {
            range,
            depends_on: std::iter::once(chunk).collect(),
        }
    }
}

impl re_byte_size::SizeBytes for TimeRange {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            range: _,
            depends_on,
        } = self;

        depends_on.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl Deref for TimeRange {
    type Target = AbsoluteTimeRange;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.range
    }
}

impl DerefMut for TimeRange {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.range
    }
}

/// Wrapper struct for custom ordering in binary heap.
struct IncomingRange(TimeRange);

impl Deref for IncomingRange {
    type Target = TimeRange;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq for IncomingRange {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for IncomingRange {}

impl PartialOrd for IncomingRange {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IncomingRange {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.range.min.cmp(&other.range.min).reverse().then(
            // Take the largest first.
            self.range
                .abs_length()
                .cmp(&other.range.abs_length())
                .reverse(),
        )
    }
}

struct Ranges {
    new: Vec<TimeRange>,
    incoming: BinaryHeap<IncomingRange>,
}

impl Ranges {
    fn push(&mut self, range: TimeRange) {
        let Some(last_range) = self.new.last_mut() else {
            self.new.push(range);
            return;
        };

        // We handle merging ranges differently depending on their state.
        //
        // The goal here is to keep track of which chunks time ranges depend on to be loaded,
        // and have no gaps or overlaps. For gaps we extend the last state since we want it
        // to represent what ranges a latest at query has available.
        if last_range.depends_on == range.depends_on {
            // Equal dependants for both ranges, combine them.
            //
            // examples:
            // ```text
            // case 1:
            //     last_range: |-chunk0-|
            //          range:     |---chunk0---|
            // result:
            // new last_range: |-----chunk0-----|
            //
            // case 2:
            //     last_range: |--chunk0--|
            //          range:                  |-chunk0-|
            //
            // result:
            // new last_range: |---------chunk0----------|
            //
            // ```

            last_range.max = last_range.max.max(range.max);
        } else if last_range.max <= range.min {
            // Extend the last range until the start of the new one
            // for proper "latest-at" semantics.
            //
            // example:
            // ```text
            //     last_range: |chunk0|
            //          range:              |chunk1|
            //
            // result:
            // new last_range: |---chunk0---|
            //      new range:              |chunk1|
            // ```
            last_range.max = range.min;
            self.new.push(range);
        } else if last_range.min == range.min {
            // Both ranges start at the same time, combine what we can.
            //
            // examples:
            // ```text
            // case 1:
            //     last_range: |----chunk0-----|
            //          range: |----chunk1-----|
            // result:
            // new last_range: |-chunk0,chunk1-|
            //
            // case 2:
            //     last_range: |---------chunk0-------|
            //          range: |----chunk1-----|
            //
            // result:
            // new last_range: |-chunk0,chunk1-|
            //  delayed range:                 |chunk0|
            //
            // case 3:
            //     last_range: |----chunk0-----|
            //          range: |---------chunk1-------|
            //
            // result:
            // new last_range: |-chunk0,chunk1-|
            //  delayed range:                 |chunk1|
            //
            // ```
            if last_range.max < range.max {
                self.incoming.push(IncomingRange(TimeRange {
                    range: AbsoluteTimeRange::new(last_range.max, range.max),
                    depends_on: range.depends_on.clone(),
                }));
            } else if last_range.max > range.max {
                self.incoming.push(IncomingRange(TimeRange {
                    range: AbsoluteTimeRange::new(range.max, last_range.max),
                    depends_on: last_range.depends_on.clone(),
                }));

                last_range.max = range.max;
            }

            last_range.depends_on.extend(range.depends_on);
        } else {
            // New range starts within the last range.
            //
            // examples:
            // ```text
            // case 1:
            //     last_range: |---------chunk0-------|
            //          range:        |----chunk1-----|
            //
            // result:
            // new last_range: |chunk0|
            //      new range:        |-chunk0,chunk1-|
            //
            // case 2:
            //     last_range: |-------chunk0-------|
            //          range:        |-------chunk1-------|
            // result:
            // new last_range: |chunk0|
            //      new range:        |chunk0,chunk1|
            //  delayed range:                      |chunk1|
            //
            // case 3:
            //     last_range: |-----------chunk0----------|
            //          range:        |----chunk1---|
            //
            // result:
            // new last_range: |chunk0|
            //      new range:        |chunk0,chunk1|
            //  delayed range:                      |chunk0|
            //
            // ```
            if last_range.max < range.max {
                self.incoming.push(IncomingRange(TimeRange {
                    range: AbsoluteTimeRange::new(last_range.max, range.max),
                    depends_on: range.depends_on.clone(),
                }));
            } else if last_range.max > range.max {
                self.incoming.push(IncomingRange(TimeRange {
                    range: AbsoluteTimeRange::new(range.max, last_range.max),
                    depends_on: last_range.depends_on.clone(),
                }));
            }
            let new_range =
                AbsoluteTimeRange::new(range.min, TimeInt::min(range.max, last_range.max));
            let depends_on = last_range
                .depends_on
                .union(&range.depends_on)
                .copied()
                .collect();
            last_range.max = range.min;
            self.new.push(TimeRange {
                range: new_range,
                depends_on,
            });
        }
    }
}

#[derive(Clone)]
struct ResolvedRange {
    range: AbsoluteTimeRange,

    /// The number of unloaded chunks in this range, when this reaches 0
    /// we know the chunk is fully loaded.
    unloaded_count: usize,
}

impl re_byte_size::SizeBytes for ResolvedRange {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            range: _,
            unloaded_count: _,
        } = self;

        0
    }

    fn is_pod() -> bool {
        true
    }
}

/// Stores time ranges that keep track if they're loaded or unloaded.
#[derive(Clone)]
pub struct MergedRanges {
    ranges: Vec<ResolvedRange>,
    ranges_from_chunk: ahash::HashMap<ChunkId, Vec<usize>>,
}

impl re_byte_size::SizeBytes for MergedRanges {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            ranges,
            ranges_from_chunk,
        } = self;

        ranges.heap_size_bytes() + ranges_from_chunk.heap_size_bytes()
    }
}

impl MergedRanges {
    pub fn new(ranges: Vec<TimeRange>) -> Self {
        re_tracing::profile_function!();
        let mut ranges_from_chunk = ahash::HashMap::<ChunkId, Vec<_>>::default();

        let new_ranges = ranges
            .into_iter()
            .enumerate()
            .map(|(idx, range)| {
                let unloaded_count = range.depends_on.len();
                for chunk in range.depends_on {
                    ranges_from_chunk.entry(chunk).or_default().push(idx);
                }
                ResolvedRange {
                    range: range.range,
                    unloaded_count,
                }
            })
            .collect();

        Self {
            ranges: new_ranges,
            ranges_from_chunk,
        }
    }

    /// Collects a number of non-overlapping time ranges which are fully loaded.
    pub fn loaded_ranges(&self) -> Vec<AbsoluteTimeRange> {
        profile_function!(format!("ranges: {}", self.ranges.len()));
        let mut loaded_ranges = Vec::new();
        let mut in_progress: Option<AbsoluteTimeRange> = None;

        for range in &self.ranges {
            if range.unloaded_count == 0 {
                if let Some(in_progress) = &mut in_progress {
                    in_progress.max = range.range.max;
                } else {
                    in_progress = Some(range.range);
                }
            } else if let Some(range) = in_progress.take() {
                loaded_ranges.push(range);
            }
        }

        if let Some(range) = in_progress.take() {
            loaded_ranges.push(range);
        }

        loaded_ranges
    }

    pub fn on_chunk_loaded(&mut self, chunk: &ChunkId) {
        let Some(ranges) = self.ranges_from_chunk.get(chunk) else {
            return;
        };

        for range in ranges {
            let resolved_range = &mut self.ranges[*range];

            resolved_range.unloaded_count = resolved_range.unloaded_count.saturating_sub(1);
        }
    }

    pub fn on_chunk_unloaded(&mut self, chunk: &ChunkId) {
        let Some(ranges) = self.ranges_from_chunk.get(chunk) else {
            return;
        };

        for range in ranges {
            self.ranges[*range].unloaded_count += 1;
        }
    }
}

/// Utility to merge multiple time ranges related to a set of chunks into a list of
/// sorted time ranges with no gaps or overlaps.
pub fn merge_ranges(ranges: impl Iterator<Item = TimeRange>) -> Vec<TimeRange> {
    re_tracing::profile_function!();

    let mut ranges = Ranges {
        new: Vec::new(),
        incoming: ranges.map(IncomingRange).collect(),
    };
    re_tracing::profile_scope!(format!("{} ranges", ranges.incoming.len()));

    while let Some(r) = ranges.incoming.pop() {
        ranges.push(r.0);
    }

    ranges.new
}

#[cfg(test)]
mod tests {
    use super::*;
    use re_chunk::TimeInt;

    fn chunk_id(n: u128) -> ChunkId {
        ChunkId::from_u128(n)
    }

    fn time_range(chunk: ChunkId, min: i64, max: i64) -> TimeRange {
        TimeRange::new(
            chunk,
            AbsoluteTimeRange::new(TimeInt::new_temporal(min), TimeInt::new_temporal(max)),
        )
    }

    fn assert_ranges_eq(result: &[TimeRange], expected: &[(i64, i64, &[u128])]) {
        assert_eq!(
            result.len(),
            expected.len(),
            "Range count mismatch.\nGot: {:?}\nExpected: {:?}",
            result
                .iter()
                .map(|r| (r.min.as_i64(), r.max.as_i64()))
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|(min, max, _)| (*min, *max))
                .collect::<Vec<_>>()
        );

        for (i, (range, (exp_min, exp_max, exp_chunks))) in
            result.iter().zip(expected.iter()).enumerate()
        {
            assert_eq!(range.min.as_i64(), *exp_min, "Range {i} min mismatch");
            assert_eq!(range.max.as_i64(), *exp_max, "Range {i} max mismatch");
            let expected_chunks: HashSet<ChunkId> =
                exp_chunks.iter().map(|&id| chunk_id(id)).collect();
            assert_eq!(
                range.depends_on, expected_chunks,
                "Range {i} depends_on mismatch"
            );
        }
    }

    #[test]
    fn test_empty_ranges() {
        let result = merge_ranges(std::iter::empty());
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_range() {
        let result = merge_ranges(std::iter::once(time_range(chunk_id(1), 0, 10)));
        assert_ranges_eq(&result, &[(0, 10, &[1])]);
    }

    #[test]
    fn test_non_overlapping_same_chunk() {
        // Two ranges from the same chunk with a gap between them
        let c1 = chunk_id(1);
        let result = merge_ranges([time_range(c1, 0, 10), time_range(c1, 20, 30)].into_iter());

        // Gap should be filled since they have the same dependency
        assert_ranges_eq(&result, &[(0, 30, &[1])]);
    }

    #[test]
    fn test_non_overlapping_different_chunks() {
        // Two ranges from different chunks with a gap
        let result = merge_ranges(
            [
                time_range(chunk_id(1), 0, 10),
                time_range(chunk_id(2), 20, 30),
            ]
            .into_iter(),
        );

        // First range extended to fill gap, second range starts where first ends
        assert_ranges_eq(&result, &[(0, 20, &[1]), (20, 30, &[2])]);
    }

    #[test]
    fn test_overlapping_same_chunk() {
        // Overlapping ranges from the same chunk
        let c1 = chunk_id(1);
        let result = merge_ranges([time_range(c1, 0, 15), time_range(c1, 10, 25)].into_iter());

        assert_ranges_eq(&result, &[(0, 25, &[1])]);
    }

    #[test]
    fn test_same_start_same_end_different_chunks() {
        // Two ranges with identical bounds but different chunks
        let result = merge_ranges(
            [
                time_range(chunk_id(1), 0, 10),
                time_range(chunk_id(2), 0, 10),
            ]
            .into_iter(),
        );

        assert_ranges_eq(&result, &[(0, 10, &[1, 2])]);
    }

    #[test]
    fn test_same_start_first_longer() {
        // Same start, but first range is longer
        let result = merge_ranges(
            [
                time_range(chunk_id(1), 0, 20),
                time_range(chunk_id(2), 0, 10),
            ]
            .into_iter(),
        );

        assert_ranges_eq(&result, &[(0, 10, &[1, 2]), (10, 20, &[1])]);
    }

    #[test]
    fn test_same_start_second_longer() {
        // Same start, but second range is longer
        let result = merge_ranges(
            [
                time_range(chunk_id(1), 0, 10),
                time_range(chunk_id(2), 0, 20),
            ]
            .into_iter(),
        );

        assert_ranges_eq(&result, &[(0, 10, &[1, 2]), (10, 20, &[2])]);
    }

    #[test]
    fn test_second_starts_within_first_same_end() {
        // Second range starts within first, both end at same point
        let result = merge_ranges(
            [
                time_range(chunk_id(1), 0, 20),
                time_range(chunk_id(2), 10, 20),
            ]
            .into_iter(),
        );

        assert_ranges_eq(&result, &[(0, 10, &[1]), (10, 20, &[1, 2])]);
    }

    #[test]
    fn test_second_starts_within_first_second_longer() {
        // Second range starts within first and extends beyond
        let result = merge_ranges(
            [
                time_range(chunk_id(1), 0, 20),
                time_range(chunk_id(2), 10, 30),
            ]
            .into_iter(),
        );

        assert_ranges_eq(&result, &[(0, 10, &[1]), (10, 20, &[1, 2]), (20, 30, &[2])]);
    }

    #[test]
    fn test_second_contained_within_first() {
        // Second range is entirely contained within first
        let result = merge_ranges(
            [
                time_range(chunk_id(1), 0, 30),
                time_range(chunk_id(2), 10, 20),
            ]
            .into_iter(),
        );

        assert_ranges_eq(&result, &[(0, 10, &[1]), (10, 20, &[1, 2]), (20, 30, &[1])]);
    }

    #[test]
    fn test_three_overlapping_ranges() {
        let result = merge_ranges(
            [
                time_range(chunk_id(1), 0, 20),
                time_range(chunk_id(2), 10, 30),
                time_range(chunk_id(3), 20, 40),
            ]
            .into_iter(),
        );

        assert_ranges_eq(
            &result,
            &[
                (0, 10, &[1]),
                (10, 20, &[1, 2]),
                (20, 30, &[2, 3]),
                (30, 40, &[3]),
            ],
        );
    }

    #[test]
    fn test_merged_ranges_loaded_ranges_all_unloaded() {
        let ranges = merge_ranges(
            [
                time_range(chunk_id(1), 0, 10),
                time_range(chunk_id(2), 10, 20),
            ]
            .into_iter(),
        );

        let merged = MergedRanges::new(ranges);
        let loaded = merged.loaded_ranges();

        // Nothing is loaded yet
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_merged_ranges_on_chunk_loaded() {
        let ranges = merge_ranges(
            [
                time_range(chunk_id(1), 0, 10),
                time_range(chunk_id(2), 10, 20),
            ]
            .into_iter(),
        );

        let mut merged = MergedRanges::new(ranges);

        // Load chunk 1
        merged.on_chunk_loaded(&chunk_id(1));
        let loaded = merged.loaded_ranges();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].min.as_i64(), 0);
        assert_eq!(loaded[0].max.as_i64(), 10);

        // Load chunk 2
        merged.on_chunk_loaded(&chunk_id(2));
        let loaded = merged.loaded_ranges();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].min.as_i64(), 0);
        assert_eq!(loaded[0].max.as_i64(), 20);
    }

    #[test]
    fn test_merged_ranges_on_chunk_unloaded() {
        let ranges = merge_ranges(
            [
                time_range(chunk_id(1), 0, 10),
                time_range(chunk_id(2), 10, 20),
            ]
            .into_iter(),
        );

        let mut merged = MergedRanges::new(ranges);

        // Load both chunks
        merged.on_chunk_loaded(&chunk_id(1));
        merged.on_chunk_loaded(&chunk_id(2));

        // Unload chunk 1
        merged.on_chunk_unloaded(&chunk_id(1));
        let loaded = merged.loaded_ranges();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].min.as_i64(), 10);
        assert_eq!(loaded[0].max.as_i64(), 20);
    }

    #[test]
    fn test_merged_ranges_shared_dependency() {
        // Range that depends on both chunks
        let ranges = merge_ranges(
            [
                time_range(chunk_id(1), 0, 20),
                time_range(chunk_id(2), 10, 30),
            ]
            .into_iter(),
        );

        let mut merged = MergedRanges::new(ranges);

        // Load only chunk 1 - middle range still unloaded because it needs both
        merged.on_chunk_loaded(&chunk_id(1));
        let loaded = merged.loaded_ranges();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].min.as_i64(), 0);
        assert_eq!(loaded[0].max.as_i64(), 10);

        // Now load chunk 2 - everything should be loaded
        merged.on_chunk_loaded(&chunk_id(2));
        let loaded = merged.loaded_ranges();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].min.as_i64(), 0);
        assert_eq!(loaded[0].max.as_i64(), 30);
    }

    #[test]
    fn test_loaded_ranges_with_gap() {
        let ranges = merge_ranges(
            [
                time_range(chunk_id(1), 0, 10),
                time_range(chunk_id(2), 10, 20),
                time_range(chunk_id(3), 20, 30),
            ]
            .into_iter(),
        );

        let mut merged = MergedRanges::new(ranges);

        // Load chunks 1 and 3, leaving 2 unloaded
        merged.on_chunk_loaded(&chunk_id(1));
        merged.on_chunk_loaded(&chunk_id(3));

        let loaded = merged.loaded_ranges();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].min.as_i64(), 0);
        assert_eq!(loaded[0].max.as_i64(), 10);
        assert_eq!(loaded[1].min.as_i64(), 20);
        assert_eq!(loaded[1].max.as_i64(), 30);
    }

    #[test]
    fn test_on_chunk_loaded_unknown_chunk() {
        let ranges = merge_ranges(std::iter::once(time_range(chunk_id(1), 0, 10)));
        let mut merged = MergedRanges::new(ranges);

        // Loading an unknown chunk should not panic
        merged.on_chunk_loaded(&chunk_id(999));

        // State should be unchanged
        let loaded = merged.loaded_ranges();
        assert!(loaded.is_empty());
    }
}
