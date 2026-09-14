use std::collections::BTreeMap;

use ahash::HashMap;
use re_chunk::{ChunkId, TimelineName};
use re_chunk_store::ChunkStore;
use re_log_types::AbsoluteTimeRange;

use super::RootChunkInfo;

/// Chunks spanning more than 20% of the full timeline are considered "large"
/// and will be scanned for internal gaps once loaded.
fn large_chunk_threshold(timeline_range: AbsoluteTimeRange) -> u64 {
    (timeline_range.abs_length() / 5).max(10)
}

/// Sort ranges by start time and merge overlapping/adjacent ones.
fn merge_and_sort_ranges(ranges: &[AbsoluteTimeRange]) -> Vec<AbsoluteTimeRange> {
    let Some(sorted) = vec1::Vec1::try_from_vec({
        let mut v = ranges.to_vec();
        v.sort_by_key(|r| r.min.as_i64());
        v
    })
    .ok() else {
        return Vec::new();
    };

    let (first, rest) = sorted.split_off_first();
    let mut merged = vec1::vec1![first];
    for range in rest {
        let last = merged.last_mut();
        if range.min.as_i64() <= last.max.as_i64() + 1 {
            if range.max.as_i64() > last.max.as_i64() {
                last.max = range.max;
            }
        } else {
            merged.push(range);
        }
    }

    merged.into()
}

/// Split a time column into sub-ranges at gaps larger than `gap_threshold`.
fn split_time_column_at_gaps(
    time_column: &re_chunk::TimeColumn,
    gap_threshold: u64,
) -> Vec<AbsoluteTimeRange> {
    let times = time_column.times_raw();
    if times.len() < 2 || !time_column.is_sorted() {
        return vec![time_column.time_range()];
    }

    let mut ranges = Vec::new();
    let mut start = times[0];
    let mut prev = times[0];

    for &t in &times[1..] {
        if prev.abs_diff(t) > gap_threshold {
            ranges.push(AbsoluteTimeRange::new(start, prev));
            start = t;
        }
        prev = t;
    }
    ranges.push(AbsoluteTimeRange::new(start, prev));
    ranges
}

/// Compute data time ranges from manifest chunk ranges.
///
/// This merges all chunk time ranges per timeline to detect gaps between chunks.
/// Chunks spanning large durations are tracked for recalculation when they get loaded.
pub fn compute_data_time_ranges(
    root_chunks: &HashMap<ChunkId, RootChunkInfo>,
) -> BTreeMap<TimelineName, Vec<AbsoluteTimeRange>> {
    re_tracing::profile_function!();

    let mut ranges_per_timeline: BTreeMap<TimelineName, Vec<AbsoluteTimeRange>> = BTreeMap::new();

    for chunk_info in root_chunks.values() {
        for (timeline_name, temporal_info) in &chunk_info.temporals {
            ranges_per_timeline
                .entry(*timeline_name)
                .or_default()
                .push(temporal_info.time_range);
        }
    }

    let mut result = BTreeMap::new();
    for (timeline_name, ranges) in &ranges_per_timeline {
        let merged = merge_and_sort_ranges(ranges);
        result.insert(*timeline_name, merged);
    }
    result
}

/// Refine data time ranges for a timeline by scanning all loaded physical chunks
/// for internal gaps within large time ranges.
pub fn calculate_data_ranges_for_timeline(
    root_chunks: &HashMap<ChunkId, RootChunkInfo>,
    timelines: &BTreeMap<TimelineName, AbsoluteTimeRange>,
    store: &ChunkStore,
    timeline_name: &TimelineName,
) -> Option<Vec<AbsoluteTimeRange>> {
    re_tracing::profile_function!();

    let &timeline_range = timelines.get(timeline_name)?;
    let threshold = large_chunk_threshold(timeline_range);

    let mut all_ranges: Vec<AbsoluteTimeRange> = Vec::new();

    // Loaded chunks: scan for internal gaps in large ones
    for chunk in store.physical_chunks() {
        if let Some(time_col) = chunk.timelines().get(timeline_name) {
            let range = time_col.time_range();
            let duration = range.abs_length();
            if duration > threshold {
                all_ranges.extend(split_time_column_at_gaps(time_col, threshold));
            } else {
                all_ranges.push(range);
            }
        }
    }

    // Unloaded chunks: include their manifest ranges as-is
    for chunk_info in root_chunks.values() {
        if !chunk_info.is_fully_loaded()
            && let Some(temporal_info) = chunk_info.temporals.get(timeline_name)
        {
            all_ranges.push(temporal_info.time_range);
        }
    }

    Some(merge_and_sort_ranges(&all_ranges))
}

/// Check if a newly loaded chunk is "large" enough to warrant recalculating data ranges
/// for its timelines.
pub fn should_recalculate_for_chunk(
    chunk_info: &RootChunkInfo,
    timelines: &BTreeMap<TimelineName, AbsoluteTimeRange>,
) -> Vec<TimelineName> {
    let mut result = Vec::new();
    for (timeline_name, temporal_info) in &chunk_info.temporals {
        if let Some(&timeline_range) = timelines.get(timeline_name) {
            let threshold = large_chunk_threshold(timeline_range);
            let duration = temporal_info.time_range.abs_length();
            if duration > threshold {
                result.push(*timeline_name);
            }
        }
    }
    result
}
