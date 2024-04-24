use std::collections::BTreeMap;

use re_data_store::{StoreEvent, StoreSubscriber};
use re_log_types::{TimeInt, TimePoint, Timeline};

// ---

/// Number of messages per time.
pub type TimeHistogram = re_int_histogram::Int64Histogram;

/// Number of messages per time per timeline.
///
/// Does NOT include timeless.
#[derive(Default)]
pub struct TimeHistogramPerTimeline {
    /// When do we have data? Ignores timeless.
    times: BTreeMap<Timeline, TimeHistogram>,

    /// Extra bookkeeping used to seed any timelines that include timeless msgs.
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
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> {
        self.times.keys()
    }

    #[inline]
    pub fn get(&self, timeline: &Timeline) -> Option<&TimeHistogram> {
        self.times.get(timeline)
    }

    #[inline]
    pub fn has_timeline(&self, timeline: &Timeline) -> bool {
        self.times.contains_key(timeline)
    }

    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&Timeline, &TimeHistogram)> {
        self.times.iter()
    }

    #[inline]
    pub fn num_static_messages(&self) -> u64 {
        self.num_static_messages
    }

    /// Total number of (non-static) message over all timelines.
    pub fn num_timeline_messages(&self) -> u64 {
        self.times.values().map(|hist| hist.total_count()).sum()
    }

    pub fn add(&mut self, times: &[(Timeline, TimeInt)], n: u32) {
        if times.is_empty() {
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
        } else {
            for &(timeline, time) in times {
                self.times
                    .entry(timeline)
                    .or_default()
                    .increment(time.as_i64(), n);
            }
        }
    }

    pub fn remove(&mut self, timepoint: &TimePoint, n: u32) {
        if timepoint.is_static() {
            self.num_static_messages = self
                .num_static_messages
                .checked_sub(n as u64)
                .unwrap_or_else(|| {
                    // TODO(#4355): hitting this on plots demo
                    re_log::debug!(
                        current = self.num_static_messages,
                        removed = n,
                        "bookkeeping underflowed"
                    );
                    u64::MIN
                });
        } else {
            for (timeline, time_value) in timepoint.iter() {
                let remaining_count = self
                    .times
                    .entry(*timeline)
                    .or_default()
                    .decrement(time_value.as_i64(), n);
                if remaining_count == 0 {
                    self.times.remove(timeline);
                }
            }
        }
    }
}

// NOTE: This is only to let people know that this is in fact a [`StoreSubscriber`], so they A) don't try
// to implement it on their own and B) don't try to register it.
impl StoreSubscriber for TimeHistogramPerTimeline {
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

    #[allow(clippy::unimplemented)]
    fn on_events(&mut self, _events: &[StoreEvent]) {
        unimplemented!(
            r"TimeHistogramPerTimeline view is maintained as a sub-view of `EntityTree`",
        );
    }
}
