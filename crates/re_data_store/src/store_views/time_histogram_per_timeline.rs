use std::collections::BTreeMap;

use re_arrow_store::{StoreEvent, StoreView};
use re_log_types::{TimeInt, TimePoint, Timeline};

// TODO: inline

// ---

/// Number of messages per time
pub type TimeHistogram = re_int_histogram::Int64Histogram;

/// Number of messages per time per timeline.
///
/// Does NOT include timeless.
#[derive(Default)]
pub struct TimeHistogramPerTimeline {
    /// When do we have data? Ignores timeless.
    pub times: BTreeMap<Timeline, TimeHistogram>,

    /// Extra book-keeping used to seed any timelines that include timeless msgs.
    pub num_timeless_messages: u64,
}

impl TimeHistogramPerTimeline {
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> {
        self.times.keys()
    }

    pub fn get(&self, timeline: &Timeline) -> Option<&TimeHistogram> {
        self.times.get(timeline)
    }

    pub fn has_timeline(&self, timeline: &Timeline) -> bool {
        self.times.contains_key(timeline)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&Timeline, &TimeHistogram)> {
        self.times.iter()
    }

    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = (&Timeline, &mut TimeHistogram)> {
        self.times.iter_mut()
    }

    pub fn increment(&mut self, timeline: impl Into<Timeline>, time: impl Into<TimeInt>, inc: u32) {
        self.times
            .entry(timeline.into())
            .or_default()
            .increment(time.into().as_i64(), inc);
    }

    pub fn decrement(&mut self, timeline: impl Into<Timeline>, time: impl Into<TimeInt>, dec: u32) {
        self.times.entry(timeline.into()).and_modify(|histo| {
            histo.decrement(time.into().as_i64(), dec);
        });
    }

    pub fn num_timeless_messages(&self) -> u64 {
        self.num_timeless_messages
    }

    // TODO: remove
    pub fn add(&mut self, timepoint: &TimePoint) {
        // If the `time_point` is timeless…
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

    // // TODO: remove
    // pub fn purge(&mut self, deleted: &ActuallyDeleted) {
    //     re_tracing::profile_function!();
    //
    //     for (timeline, histogram) in &mut self.times {
    //         if let Some(times) = deleted.timeful.get(timeline) {
    //             for &time in times {
    //                 histogram.decrement(time.as_i64(), 1);
    //             }
    //         }
    //
    //         // NOTE: we don't include timeless in the histogram.
    //     }
    // }
}

impl StoreView for TimeHistogramPerTimeline {
    fn name(&self) -> String {
        "rerun.store_views.TimeHistogramPerTimeline".into()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[StoreEvent]) {
        re_tracing::profile_function!(format!("num_events={}", events.len()));

        for event in events {
            let diff = &event.diff;

            if let Some((timeline, time)) = diff.timestamp {
                if diff.delta < 0 {
                    self.decrement(timeline, time, diff.delta.unsigned_abs() as u32);
                } else {
                    self.increment(timeline, time, diff.delta as u32);
                }
            } else {
                self.num_timeless_messages =
                    self.num_timeless_messages.saturating_add_signed(diff.delta);
            }
        }
    }
}
