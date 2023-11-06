use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;
use nohash_hasher::IntMap;

use re_arrow_store::{StoreDiff, StoreEvent, StoreView};
use re_log_types::{
    ComponentPath, EntityPath, EntityPathPart, PathOp, RowId, TimeInt, TimePoint, Timeline,
};
use re_types_core::{ComponentName, Loggable};

// TODO: SizeBytes?

// ---

/// Keeps track of unique timestamps on each [`Timeline`].
///
/// Does NOT include timeless.
pub struct TimesPerTimeline(BTreeMap<Timeline, BTreeSet<TimeInt>>);

impl TimesPerTimeline {
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> {
        self.0.keys()
    }

    pub fn get(&self, timeline: &Timeline) -> Option<&BTreeSet<TimeInt>> {
        self.0.get(timeline)
    }

    pub fn get_mut(&mut self, timeline: &Timeline) -> Option<&mut BTreeSet<TimeInt>> {
        self.0.get_mut(timeline)
    }

    pub fn insert(&mut self, timeline: Timeline, time: TimeInt) {
        self.0.entry(timeline).or_default().insert(time);
    }

    pub fn remove(&mut self, timeline: Timeline, time: TimeInt) {
        self.0.entry(timeline).or_default().remove(&time);
    }

    pub fn has_timeline(&self, timeline: &Timeline) -> bool {
        self.0.contains_key(timeline)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&Timeline, &BTreeSet<TimeInt>)> {
        self.0.iter()
    }

    pub fn iter_mut(
        &mut self,
    ) -> impl ExactSizeIterator<Item = (&Timeline, &mut BTreeSet<TimeInt>)> {
        self.0.iter_mut()
    }
}

// Always ensure we have a default "log_time" timeline.
impl Default for TimesPerTimeline {
    fn default() -> Self {
        eprintln!("hello");
        Self(BTreeMap::from([(Timeline::log_time(), Default::default())]))
    }
}

// ---

// TODO: dont split the view, wtf
#[derive(Default)]
pub struct TimesPerTimelineView {
    pub times: TimesPerTimeline,

    // TODO: explain: why
    // TODO: merge the counts?
    counts: ahash::HashMap<(Timeline, TimeInt), u64>,
}

impl StoreView for TimesPerTimelineView {
    fn name(&self) -> String {
        "rerun.store_view.TimesPerTimeline".into()
    }

    fn registerable(&self) -> bool {
        false
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
                let count = self.counts.entry((timeline, time)).or_default();

                // first occurence
                if diff.delta > 0 && *count == 0 {
                    self.times.insert(timeline, time);
                }
                // last occurence
                else if diff.delta < 0 && *count <= diff.delta.unsigned_abs() {
                    self.times.remove(timeline, time);
                }

                *count = count.saturating_add_signed(diff.delta);
            }
        }
    }
}
