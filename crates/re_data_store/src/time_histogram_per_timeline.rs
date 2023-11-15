use std::collections::BTreeMap;

use re_arrow_store::{StoreEvent, StoreSubscriber};
use re_log_types::{TimePoint, Timeline};

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

    /// Extra book-keeping used to seed any timelines that include timeless msgs.
    num_timeless_messages: u64,
}

impl TimeHistogramPerTimeline {
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
    pub fn num_timeless_messages(&self) -> u64 {
        self.num_timeless_messages
    }

    pub fn add(&mut self, timepoint: &TimePoint, n: u32) {
        if timepoint.is_timeless() {
            self.num_timeless_messages = self
                .num_timeless_messages
                .checked_add(n as u64)
                .unwrap_or_else(|| {
                    re_log::warn_once!("Timeless counter overflowed, store events are bugged!");
                    u64::MAX
                });
        } else {
            for (timeline, time_value) in timepoint.iter() {
                self.times
                    .entry(*timeline)
                    .or_default()
                    .increment(time_value.as_i64(), n);
            }
        }
    }

    pub fn remove(&mut self, timepoint: &TimePoint, n: u32) {
        if timepoint.is_timeless() {
            self.num_timeless_messages = self
                .num_timeless_messages
                .checked_sub(n as u64)
                .unwrap_or_else(|| {
                    re_log::warn_once!("Timeless counter underflowed, store events are bugged!");
                    0
                });
        } else {
            for (timeline, time_value) in timepoint.iter() {
                self.times
                    .entry(*timeline)
                    .or_default()
                    .decrement(time_value.as_i64(), n);
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
