use std::collections::BTreeMap;

use re_arrow_store::{StoreEvent, StoreView};
use re_log_types::{TimeInt, Timeline};

// ---

pub type TimeCounts = BTreeMap<TimeInt, u64>;

/// A [`StoreView`] that keeps track of all unique timestamps on each [`Timeline`].
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

impl StoreView for TimesPerTimeline {
    #[inline]
    fn name(&self) -> String {
        "rerun.store_view.TimesPerTimeline".into()
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
                *count = count.saturating_add_signed(event.delta());

                if *count == 0 {
                    per_time.remove(&time);
                }
            }
        }
    }
}
