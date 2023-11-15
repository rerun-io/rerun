use std::collections::BTreeMap;

use re_arrow_store::{StoreEvent, StoreSubscriber};
use re_log_types::{TimeInt, Timeline};

// ---

pub type TimeCounts = BTreeMap<TimeInt, u64>;

/// A [`StoreSubscriber`] that keeps track of all unique timestamps on each [`Timeline`].
pub struct TimesPerTimeline(BTreeMap<Timeline, TimeCounts>);

impl std::ops::Deref for TimesPerTimeline {
    type Target = BTreeMap<Timeline, TimeCounts>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TimesPerTimeline {
    #[inline]
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> {
        self.0.keys()
    }
}

// Always ensure we have a default "log_time" timeline.
impl Default for TimesPerTimeline {
    fn default() -> Self {
        Self(BTreeMap::from([(Timeline::log_time(), Default::default())]))
    }
}

impl StoreSubscriber for TimesPerTimeline {
    #[inline]
    fn name(&self) -> String {
        "rerun.store_subscriber.TimesPerTimeline".into()
    }

    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    #[inline]
    fn on_events(&mut self, events: &[StoreEvent]) {
        re_tracing::profile_function!(format!("num_events={}", events.len()));

        for event in events {
            for (&timeline, &time) in &event.timepoint {
                let per_time = self.0.entry(timeline).or_default();
                let count = per_time.entry(time).or_default();

                let delta = event.delta();

                #[allow(clippy::collapsible_else_if)] // readability
                if delta < 0 {
                    if let Some(new) = count.checked_sub(delta.unsigned_abs()) {
                        *count = new;
                    } else {
                        re_log::warn_once!("Time counter underflowed, store events are bugged!");
                        *count = 0;
                    }
                } else {
                    if let Some(new) = count.checked_add(delta.unsigned_abs()) {
                        *count = new;
                    } else {
                        re_log::warn_once!("Time counter overflowed, store events are bugged!");
                        *count = u64::MAX;
                    }
                }

                if *count == 0 {
                    per_time.remove(&time);
                }
            }
        }
    }
}
