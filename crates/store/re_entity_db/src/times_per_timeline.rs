use std::collections::BTreeMap;

use re_chunk_store::{ChunkStoreEvent, ChunkStoreSubscriber};
use re_log_types::{TimeInt, Timeline};

// ---

pub type TimeCounts = BTreeMap<TimeInt, u64>;

/// A [`ChunkStoreSubscriber`] that keeps track of all unique timestamps on each [`Timeline`].
///
/// TODO(#7084): Get rid of [`TimesPerTimeline`] and implement time-stepping with [`crate::TimeHistogram`] instead.
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
                let per_time = self.0.entry(timeline).or_default();

                for time in time_column.times() {
                    let count = per_time.entry(time).or_default();

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
                        per_time.remove(&time);
                    }
                }
            }
        }
    }
}
