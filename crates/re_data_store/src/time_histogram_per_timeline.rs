use std::collections::BTreeMap;

use re_arrow_store::{StoreEvent, StoreView};
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

    // TODO(cmc): pub(crate) is temporary while we turn StoreDb/EntityDb/EntityTree into event subscribers.
    /// Extra book-keeping used to seed any timelines that include timeless msgs.
    pub(crate) num_timeless_messages: u64,
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

    // TODO(cmc): temporary while we turn StoreDb/EntityDb/EntityTree into event subscribers.
    #[inline]
    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = (&Timeline, &mut TimeHistogram)> {
        self.times.iter_mut()
    }

    #[inline]
    pub fn num_timeless_messages(&self) -> u64 {
        self.num_timeless_messages
    }

    pub fn add(&mut self, timepoint: &TimePoint) {
        // If the `timepoint` is timeless…
        if timepoint.is_timeless() {
            self.num_timeless_messages += 1;
        } else {
            for (timeline, time_value) in timepoint.iter() {
                self.times
                    .entry(*timeline)
                    .or_default()
                    .increment(time_value.as_i64(), 1);
            }
        }
    }

    pub fn remove(&mut self, timepoint: &TimePoint) {
        // If the `timepoint` is timeless…
        if timepoint.is_timeless() {
            self.num_timeless_messages -= 1;
        } else {
            for (timeline, time_value) in timepoint.iter() {
                self.times
                    .entry(*timeline)
                    .or_default()
                    .decrement(time_value.as_i64(), 1);
            }
        }
    }
}

impl StoreView for TimeHistogramPerTimeline {
    #[inline]
    fn name(&self) -> String {
        "rerun.store_view.TimeHistogramPerTimeline".into()
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
