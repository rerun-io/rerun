use std::collections::BTreeMap;

use re_chunk::TimelineName;
use re_entity_db::EntityDb;
use re_log_types::{AbsoluteTimeRange, TimeInt, Timeline};

/// Provides timeline data for [`super::TimeControl`] step, move, and range operations.
///
/// Implemented by [`re_entity_db::EntityDb`] for real recordings. Also implemented by
/// [`PreviewRecordingsDb`] for synchronized preview playback across multiple recordings.
pub trait TimeControlDb {
    /// Returns the previous time with data on the given timeline, strictly before `before`.
    fn prev_time_on_timeline(&self, timeline: &TimelineName, time: TimeInt) -> Option<TimeInt>;

    /// Returns the next time with data on the given timeline, strictly after `after`.
    fn next_time_on_timeline(&self, timeline: &TimelineName, time: TimeInt) -> Option<TimeInt>;

    /// Returns the time range of data on the given timeline, ignoring any static times.
    fn time_range_for(&self, timeline: &TimelineName) -> Option<AbsoluteTimeRange>;

    fn timelines(&self) -> BTreeMap<TimelineName, Timeline>;

    /// Returns the total number of temporal rows on the given timeline across all entities.
    fn num_temporal_rows_on_timeline(&self, timeline: &TimelineName) -> u64;
}

impl TimeControlDb for EntityDb {
    fn prev_time_on_timeline(&self, timeline: &TimelineName, time: TimeInt) -> Option<TimeInt> {
        self.prev_time_on_timeline(timeline, time)
    }

    fn next_time_on_timeline(&self, timeline: &TimelineName, time: TimeInt) -> Option<TimeInt> {
        self.next_time_on_timeline(timeline, time)
    }

    fn time_range_for(&self, timeline: &TimelineName) -> Option<AbsoluteTimeRange> {
        self.time_range_for(timeline)
    }

    fn timelines(&self) -> BTreeMap<TimelineName, Timeline> {
        self.timelines()
    }

    fn num_temporal_rows_on_timeline(&self, timeline: &TimelineName) -> u64 {
        self.num_temporal_rows_on_timeline(timeline)
    }
}

/// A [`TimeControlDb`] that aggregates across multiple recordings for synchronized preview playback.
///
/// The preview [`super::TimeControl`] cursor is a 0-based offset into all clips. Each recording's data starts at its
/// own `range.min`, so we convert offset to absolute before querying each recording and convert
/// the result back to offset space.
pub struct PreviewRecordingsDb<'a> {
    pub recordings: &'a [&'a EntityDb],
}

// For preview recordings the time is an offset from the start for each individual recording.
impl TimeControlDb for PreviewRecordingsDb<'_> {
    fn prev_time_on_timeline(&self, timeline: &TimelineName, offset: TimeInt) -> Option<TimeInt> {
        // For each recording, map offset to absolute time, query, map result back to offset space.
        // Return the max offset, which is the closest previous data point across all recordings.
        self.recordings
            .iter()
            .filter_map(|rec| {
                let range = rec.time_range_for(timeline)?;
                let absolute = range.min + offset;
                let prev_abs = rec.prev_time_on_timeline(timeline, absolute)?;
                Some(prev_abs - range.min)
            })
            .max()
    }

    fn next_time_on_timeline(&self, timeline: &TimelineName, offset: TimeInt) -> Option<TimeInt> {
        // Return the min offset, which is the closest next data point across all recordings.
        self.recordings
            .iter()
            .filter_map(|rec| {
                let range = rec.time_range_for(timeline)?;
                let absolute = range.min + offset;
                let next_abs = rec.next_time_on_timeline(timeline, absolute)?;
                Some(next_abs - range.min)
            })
            .min()
    }

    fn time_range_for(&self, timeline: &TimelineName) -> Option<AbsoluteTimeRange> {
        // The combined range in offset space is `0..max(span)` across all recordings.
        let max_span = self
            .recordings
            .iter()
            .filter_map(|rec| {
                let range = rec.time_range_for(timeline)?;
                Some(range.max - range.min)
            })
            .max()?;
        Some(AbsoluteTimeRange::new(TimeInt::ZERO, max_span))
    }

    fn timelines(&self) -> BTreeMap<TimelineName, Timeline> {
        self.recordings
            .iter()
            .flat_map(|rec| rec.timelines())
            .collect()
    }

    fn num_temporal_rows_on_timeline(&self, timeline: &TimelineName) -> u64 {
        self.recordings
            .iter()
            .map(|rec| rec.num_temporal_rows_on_timeline(timeline))
            .sum()
    }
}
