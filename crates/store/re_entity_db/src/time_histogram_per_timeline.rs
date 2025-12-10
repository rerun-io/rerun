use std::collections::BTreeMap;

use itertools::Itertools as _;
use re_chunk::TimelineName;
use re_chunk_store::{ChunkStoreDiffKind, ChunkStoreEvent, ChunkStoreSubscriber};

// ---

/// Number of messages per time.
pub type TimeHistogram = re_int_histogram::Int64Histogram;

/// Number of messages per time per timeline.
///
/// Does NOT include static data.
#[derive(Default, Clone)]
pub struct TimeHistogramPerTimeline {
    /// When do we have data? Ignores static data.
    times: BTreeMap<TimelineName, TimeHistogram>,

    /// Extra bookkeeping used to seed any timelines that include static msgs.
    has_static: bool,
}

impl TimeHistogramPerTimeline {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.times.is_empty() && !self.has_static
    }

    #[inline]
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &TimelineName> {
        self.times.keys()
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

    /// Total number of temporal messages over all timelines.
    pub fn num_temporal_messages(&self) -> u64 {
        self.times.values().map(|hist| hist.total_count()).sum()
    }

    pub fn add(&mut self, times_per_timeline: &[(TimelineName, &[i64])], n: u32) {
        re_tracing::profile_function!();

        for &(timeline_name, times) in times_per_timeline {
            let histogram = self.times.entry(timeline_name).or_default();
            for &time in times {
                histogram.increment(time, n);
            }
        }
    }

    pub fn remove(&mut self, times_per_timeline: &[(TimelineName, &[i64])], n: u32) {
        re_tracing::profile_function!();

        for &(timeline_name, times) in times_per_timeline {
            if let Some(histo) = self.times.get_mut(&timeline_name) {
                for &time in times {
                    histo.decrement(time, n);
                }
                if histo.is_empty() {
                    self.times.remove(&timeline_name);
                }
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
            let times = event
                .chunk
                .timelines()
                .iter()
                .map(|(&timeline_name, time_column)| (timeline_name, time_column.times_raw()))
                .collect_vec();
            match event.kind {
                ChunkStoreDiffKind::Addition => {
                    self.add(&times, event.num_components() as _);
                }
                ChunkStoreDiffKind::Deletion => {
                    self.remove(&times, event.num_components() as _);
                }
            }
        }
    }
}
