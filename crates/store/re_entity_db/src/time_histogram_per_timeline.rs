use std::collections::BTreeMap;
use std::ops::Bound;

use re_chunk::{TimeInt, Timeline, TimelineName};
use re_chunk_store::{ChunkStoreDiffKind, ChunkStoreEvent, ChunkStoreSubscriber};
use re_log_types::{AbsoluteTimeRange, AbsoluteTimeRangeF, TimeReal};

// ---

/// Number of messages per time.
#[derive(Clone)]
pub struct TimeHistogram {
    timeline: Timeline,
    hist: re_int_histogram::Int64Histogram,
}

impl std::ops::Deref for TimeHistogram {
    type Target = re_int_histogram::Int64Histogram;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.hist
    }
}

impl TimeHistogram {
    pub fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            hist: Default::default(),
        }
    }

    pub fn timeline(&self) -> Timeline {
        self.timeline
    }

    pub fn increment(&mut self, time: i64, n: u32) {
        self.hist.increment(time, n);
    }

    pub fn decrement(&mut self, time: i64, n: u32) {
        self.hist.decrement(time, n);
    }

    pub fn min_opt(&self) -> Option<TimeInt> {
        self.min_key().map(TimeInt::new_temporal)
    }

    pub fn min(&self) -> TimeInt {
        self.min_opt().unwrap_or(TimeInt::MIN)
    }

    pub fn max_opt(&self) -> Option<TimeInt> {
        self.max_key().map(TimeInt::new_temporal)
    }

    pub fn max(&self) -> TimeInt {
        self.max_opt().unwrap_or(TimeInt::MIN)
    }

    pub fn full_range(&self) -> AbsoluteTimeRange {
        AbsoluteTimeRange::new(self.min(), self.max())
    }

    pub fn step_fwd_time(&self, time: TimeReal) -> TimeInt {
        self.next_key_after(time.floor().as_i64())
            .map(TimeInt::new_temporal)
            .unwrap_or_else(|| self.min())
    }

    pub fn step_back_time(&self, time: TimeReal) -> TimeInt {
        self.prev_key_before(time.ceil().as_i64())
            .map(TimeInt::new_temporal)
            .unwrap_or_else(|| self.max())
    }

    pub fn step_fwd_time_looped(
        &self,
        time: TimeReal,
        loop_range: &AbsoluteTimeRangeF,
    ) -> TimeReal {
        if time < loop_range.min || loop_range.max <= time {
            loop_range.min
        } else if let Some(next) = self
            .range(
                (
                    Bound::Excluded(time.floor().as_i64()),
                    Bound::Included(loop_range.max.floor().as_i64()),
                ),
                1,
            )
            .next()
            .map(|(r, _)| r.min)
        {
            TimeReal::from(next)
        } else {
            self.step_fwd_time(time).into()
        }
    }

    pub fn step_back_time_looped(
        &self,
        time: TimeReal,
        loop_range: &AbsoluteTimeRangeF,
    ) -> TimeReal {
        re_tracing::profile_function!();

        if time <= loop_range.min || loop_range.max < time {
            loop_range.max
        } else {
            // Collect all keys in the range and take the last one.
            // Yes, this could be slow :/
            let mut prev_key = None;
            for (range, _) in self.range(
                (
                    Bound::Included(loop_range.min.ceil().as_i64()),
                    Bound::Excluded(time.ceil().as_i64()),
                ),
                1,
            ) {
                prev_key = Some(range.max);
            }
            if let Some(prev) = prev_key {
                TimeReal::from(TimeInt::new_temporal(prev))
            } else {
                self.step_back_time(time).into()
            }
        }
    }
}

/// Number of messages per time per timeline.
///
/// Does NOT include static data.
#[derive(Default, Clone)]
pub struct TimeHistogramPerTimeline {
    /// When do we have data? Ignores static data.
    times: BTreeMap<TimelineName, TimeHistogram>,

    /// Extra bookkeeping used to seed any timelines that include static msgs.
    num_static_messages: u64,
}

impl TimeHistogramPerTimeline {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.times.is_empty() && self.num_static_messages == 0
    }

    #[inline]
    pub fn is_static(&self) -> bool {
        self.num_static_messages > 0
    }

    #[inline]
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = Timeline> {
        self.times.values().map(|h| h.timeline())
    }

    #[inline]
    pub fn get(&self, timeline: &TimelineName) -> Option<&TimeHistogram> {
        self.times.get(timeline)
    }

    #[inline]
    pub fn has_timeline(&self, timeline: &TimelineName) -> bool {
        self.times.contains_key(timeline)
    }

    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&TimelineName, &TimeHistogram)> {
        self.times.iter()
    }

    #[inline]
    pub fn num_static_messages(&self) -> u64 {
        self.num_static_messages
    }

    /// Total number of temporal messages over all timelines.
    pub fn num_temporal_messages(&self) -> u64 {
        self.times.values().map(|hist| hist.total_count()).sum()
    }

    fn add_static(&mut self, n: u32) {
        self.num_static_messages = self
            .num_static_messages
            .checked_add(n as u64)
            .unwrap_or_else(|| {
                re_log::debug!(
                    current = self.num_static_messages,
                    added = n,
                    "bookkeeping overflowed"
                );
                u64::MAX
            });
    }

    fn remove_static(&mut self, n: u32) {
        self.num_static_messages = self
            .num_static_messages
            .checked_sub(n as u64)
            .unwrap_or_else(|| {
                // We used to hit this on plots demo, see https://github.com/rerun-io/rerun/issues/4355.
                re_log::debug!(
                    current = self.num_static_messages,
                    removed = n,
                    "bookkeeping underflowed"
                );
                u64::MIN
            });
    }

    fn add_temporal(&mut self, timeline: &Timeline, times: &[i64], n: u32) {
        re_tracing::profile_function!();

        let histogram = self
            .times
            .entry(*timeline.name())
            .or_insert_with(|| TimeHistogram::new(*timeline));
        for &time in times {
            histogram.increment(time, n);
        }
    }

    fn remove_temporal(&mut self, timeline: &Timeline, times: &[i64], n: u32) {
        re_tracing::profile_function!();

        if let Some(histo) = self.times.get_mut(timeline.name()) {
            for &time in times {
                histo.decrement(time, n);
            }
            if histo.is_empty() {
                self.times.remove(timeline.name());
            }
        }
    }
}

// NOTE: This is only to let people know that this is in fact a [`ChunkStoreSubscriber`], so they A) don't try
// to implement it on their own and B) don't try to register it.
impl ChunkStoreSubscriber for TimeHistogramPerTimeline {
    #[inline]
    fn name(&self) -> String {
        "rerun.store_subscriber.TimeHistogramPerTimeline".into()
    }

    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        for event in events {
            if event.chunk.is_static() {
                match event.kind {
                    ChunkStoreDiffKind::Addition => {
                        self.add_static(event.num_components() as _);
                    }
                    ChunkStoreDiffKind::Deletion => {
                        self.remove_static(event.num_components() as _);
                    }
                }
            } else {
                for time_column in event.chunk.timelines().values() {
                    let times = time_column.times_raw();
                    match event.kind {
                        ChunkStoreDiffKind::Addition => {
                            self.add_temporal(
                                time_column.timeline(),
                                times,
                                event.num_components() as _,
                            );
                        }
                        ChunkStoreDiffKind::Deletion => {
                            self.remove_temporal(
                                time_column.timeline(),
                                times,
                                event.num_components() as _,
                            );
                        }
                    }
                }
            }
        }
    }
}
