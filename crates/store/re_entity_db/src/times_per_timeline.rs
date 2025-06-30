use std::collections::BTreeMap;

use re_chunk::TimelineName;
use re_chunk_store::{ChunkStoreEvent, ChunkStoreSubscriber};
use re_log_types::{TimeInt, Timeline};

// ---

pub type TimeCounts = BTreeMap<TimeInt, u64>;

#[derive(Clone)]
pub struct TimelineStats {
    pub timeline: Timeline,
    pub per_time: TimeCounts,
}

impl TimelineStats {
    pub fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            per_time: Default::default(),
        }
    }
}

/// A [`ChunkStoreSubscriber`] that keeps track of all unique timestamps on each [`Timeline`].
///
/// TODO(#7084): Get rid of [`TimesPerTimeline`] and implement time-stepping with [`crate::TimeHistogram`] instead.
#[derive(Clone)]
pub struct TimesPerTimeline(BTreeMap<TimelineName, TimelineStats>);

impl std::ops::Deref for TimesPerTimeline {
    type Target = BTreeMap<TimelineName, TimelineStats>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TimesPerTimeline {
    #[inline]
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> + '_ {
        self.0.values().map(|stats| &stats.timeline)
    }
}

// Always ensure we have a default "log_time" timeline.
impl Default for TimesPerTimeline {
    fn default() -> Self {
        let timeline = Timeline::log_time();
        Self(BTreeMap::from([(
            *timeline.name(),
            TimelineStats::new(timeline),
        )]))
    }
}

impl ChunkStoreSubscriber for TimesPerTimeline {
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
    fn on_events(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!(format!("num_events={}", events.len()));

        for event in events {
            for (&timeline, time_column) in event.chunk.timelines() {
                let stats = self
                    .0
                    .entry(timeline)
                    .or_insert_with(|| TimelineStats::new(*time_column.timeline()));

                for time in time_column.times() {
                    let count = stats.per_time.entry(time).or_default();

                    let delta = event.delta();

                    if delta < 0 {
                        *count = count.checked_sub(delta.unsigned_abs()).unwrap_or_else(|| {
                            re_log::debug!(
                                store_id = %event.store_id,
                                entity_path = %event.chunk.entity_path(),
                                current = count,
                                removed = delta.unsigned_abs(),
                                "book keeping underflowed"
                            );
                            u64::MIN
                        });
                    } else {
                        *count = count.checked_add(delta.unsigned_abs()).unwrap_or_else(|| {
                            re_log::debug!(
                                store_id = %event.store_id,
                                entity_path = %event.chunk.entity_path(),
                                current = count,
                                removed = delta.unsigned_abs(),
                                "book keeping overflowed"
                            );
                            u64::MAX
                        });
                    }

                    if *count == 0 {
                        stats.per_time.remove(&time);
                    }
                }
            }
        }
    }
}
