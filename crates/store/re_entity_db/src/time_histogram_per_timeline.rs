use std::collections::BTreeMap;
use std::ops::Bound;

use emath::lerp;
use itertools::Itertools as _;
use re_byte_size::{MemUsageNode, MemUsageTree, MemUsageTreeCapture, SizeBytes as _};
use re_chunk::{TimeInt, Timeline, TimelineName};
use re_chunk_store::{ChunkDirectLineage, ChunkStore, ChunkStoreDiff, ChunkStoreEvent};
use re_log_types::{AbsoluteTimeRange, AbsoluteTimeRangeF, TimeReal};

use crate::RrdManifestIndex;

// ---

/// Number of messages per time.
// TODO(RR-3784): get rid of TimeHistogram completely
#[derive(Clone)]
pub struct TimeHistogram {
    timeline: Timeline,
    hist: re_int_histogram::Int64Histogram,
}

impl std::ops::Deref for TimeHistogram {
    type Target = re_int_histogram::Int64Histogram;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.hist
    }
}

impl TimeHistogram {
    pub fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            hist: Default::default(),
        }
    }

    pub fn timeline(&self) -> Timeline {
        self.timeline
    }

    pub fn num_rows(&self) -> u64 {
        self.hist.total_count()
    }

    pub fn insert(&mut self, time: TimeInt, count: u64) {
        self.hist.increment(time.as_i64(), count as _);
    }

    pub fn increment(&mut self, time: i64, n: u32) {
        self.hist.increment(time, n);
    }

    pub fn decrement(&mut self, time: i64, n: u32) {
        self.hist.decrement(time, n);
    }

    fn min_opt(&self) -> Option<TimeInt> {
        self.min_key().map(TimeInt::new_temporal)
    }

    pub fn min(&self) -> TimeInt {
        self.min_opt().unwrap_or(TimeInt::MIN)
    }

    fn max_opt(&self) -> Option<TimeInt> {
        self.max_key().map(TimeInt::new_temporal)
    }

    pub fn max(&self) -> TimeInt {
        self.max_opt().unwrap_or(TimeInt::MIN)
    }

    pub fn full_range(&self) -> AbsoluteTimeRange {
        AbsoluteTimeRange::new(self.min(), self.max())
    }

    pub fn step_fwd_time(&self, time: TimeReal) -> TimeInt {
        self.next_key_after(time.floor().as_i64())
            .map(TimeInt::new_temporal)
            .unwrap_or_else(|| self.min())
    }

    pub fn step_back_time(&self, time: TimeReal) -> TimeInt {
        self.prev_key_before(time.ceil().as_i64())
            .map(TimeInt::new_temporal)
            .unwrap_or_else(|| self.max())
    }

    pub fn step_fwd_time_looped(
        &self,
        time: TimeReal,
        loop_range: &AbsoluteTimeRangeF,
    ) -> TimeReal {
        if time < loop_range.min || loop_range.max <= time {
            loop_range.min
        } else if let Some(next) = self
            .range(
                (
                    Bound::Excluded(time.floor().as_i64()),
                    Bound::Included(loop_range.max.floor().as_i64()),
                ),
                1,
            )
            .next()
            .map(|(r, _)| r.min)
        {
            TimeReal::from(next)
        } else {
            self.step_fwd_time(time).into()
        }
    }

    pub fn step_back_time_looped(
        &self,
        time: TimeReal,
        loop_range: &AbsoluteTimeRangeF,
    ) -> TimeReal {
        re_tracing::profile_function!();

        if time <= loop_range.min || loop_range.max < time {
            loop_range.max
        } else {
            // Collect all keys in the range and take the last one.
            // Yes, this could be slow :/
            let mut prev_key = None;
            for (range, _) in self.range(
                (
                    Bound::Included(loop_range.min.ceil().as_i64()),
                    Bound::Excluded(time.ceil().as_i64()),
                ),
                1,
            ) {
                prev_key = Some(range.max);
            }
            if let Some(prev) = prev_key {
                TimeReal::from(TimeInt::new_temporal(prev))
            } else {
                self.step_back_time(time).into()
            }
        }
    }
}

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
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = Timeline> {
        self.times.values().map(|h| h.timeline())
    }

    pub fn histograms(&self) -> impl ExactSizeIterator<Item = &TimeHistogram> {
        self.times.values()
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

    /// Increments `n` for each specified time.
    ///
    /// I.e. this adds a total of `n*times.len()`.
    fn add_temporal(&mut self, timeline: &Timeline, times: &[i64], n: u32) {
        re_tracing::profile_function!();

        let histogram = self
            .times
            .entry(*timeline.name())
            .or_insert_with(|| TimeHistogram::new(*timeline));
        for &time in times {
            histogram.increment(time, n);
        }
    }

    /// Decrements `n` for each specified time.
    ///
    /// I.e. this removes a total of `n*times.len()`.
    fn remove_temporal(&mut self, timeline: &Timeline, times: &[i64], n: u32) {
        re_tracing::profile_function!();

        if let Some(histogram) = self.times.get_mut(timeline.name()) {
            for &time in times {
                histogram.decrement(time, n);
            }
            if histogram.is_empty() {
                self.times.remove(timeline.name());
            }
        }
    }

    /// If we know the manifest ahead of time, we can pre-populate
    /// the histogram with a rough estimate of the final form.
    pub fn on_rrd_manifest(&mut self, rrd_manifest_index: &RrdManifestIndex) {
        re_tracing::profile_function!();

        for chunk in rrd_manifest_index.root_chunks() {
            if chunk.temporals.is_empty() {
                self.has_static = true;
            }

            for info in chunk.temporals.values() {
                let histogram = self
                    .times
                    .entry(*info.timeline.name())
                    .or_insert_with(|| TimeHistogram::new(info.timeline));

                apply_estimate(
                    Application::Add,
                    histogram,
                    info.time_range,
                    info.num_rows_for_all_entities_all_components,
                );
            }
        }
    }

    pub fn on_events(
        &mut self,
        store: &ChunkStore,
        rrd_manifest_index: &RrdManifestIndex,
        events: &[ChunkStoreEvent],
    ) {
        re_tracing::profile_function!();

        for event in events {
            match &event.diff {
                ChunkStoreDiff::Addition(add) => {
                    let delta_chunk = add.delta_chunk();

                    let root_chunk_id = add.chunk_before_processing.id();
                    let root_chunk_info = rrd_manifest_index.root_chunk_info(&root_chunk_id);

                    if delta_chunk.is_static() {
                        self.has_static = true;
                    } else {
                        for time_column in delta_chunk.timelines().values() {
                            let times = time_column.times_raw();
                            let timeline = time_column.timeline();

                            if let Some(chunk_info) = root_chunk_info
                                && let Some(timeline_info) =
                                    &chunk_info.temporals.get(timeline.name())
                            {
                                // We added an estimated value for this before, based on the RRD manifest.
                                // Now that we have the whole chunk we need to subtract those fake values again,
                                // before we add in the actual contents of the chunk:

                                let histogram = self
                                    .times
                                    .entry(*timeline.name())
                                    .or_insert_with(|| TimeHistogram::new(*timeline));

                                apply_estimate(
                                    Application::Remove,
                                    histogram,
                                    timeline_info.time_range,
                                    timeline_info.num_rows_for_all_entities_all_components,
                                );
                            }

                            self.add_temporal(
                                time_column.timeline(),
                                times,
                                // This value is incorrect since it doesn't account for the potential sparseness
                                // of individual components.
                                // I.e. this will over-count. For what we use this datastructure for, this is fine.
                                delta_chunk.num_components() as _,
                            );
                        }
                    }
                }

                ChunkStoreDiff::Deletion(del) => {
                    if del.chunk.is_static() {
                        // we don't care
                    } else {
                        // We want to explicitly look for root chunks here, even if that means walking recursively
                        // through the lineage tree.
                        // We will need them in order to re-fill the estimates as best as we can.
                        let root_chunk_ids = store.find_root_chunks(&del.chunk.id());
                        let root_chunk_infos = root_chunk_ids
                            .iter()
                            .filter_map(|cid| rrd_manifest_index.root_chunk_info(cid))
                            .collect_vec();

                        for time_column in del.chunk.timelines().values() {
                            let times = time_column.times_raw();
                            let timeline = time_column.timeline();

                            self.remove_temporal(
                                time_column.timeline(),
                                times,
                                // This value is incorrect since it doesn't account for the potential sparseness
                                // of individual components.
                                // I.e. this will over-count. For what we use this datastructure for, this is fine.
                                del.chunk.num_components() as _,
                            );

                            #[expect(clippy::match_same_arms)] // readability
                            let undo_factor: f64 = match store.direct_lineage(&del.chunk.id()) {
                                // If the removed chunk was part of split lineage of siblings, then only bring that
                                // much of the estimate back.
                                Some(ChunkDirectLineage::SplitFrom(_, sibling_ids)) => {
                                    1.0 / (sibling_ids.len() + 1) as f64
                                }

                                Some(ChunkDirectLineage::CompactedFrom(_)) => 1.0,

                                _ => 1.0,
                            };

                            for chunk_info in &root_chunk_infos {
                                if let Some(timeline_info) =
                                    chunk_info.temporals.get(timeline.name())
                                {
                                    let histogram = self
                                        .times
                                        .entry(*timeline.name())
                                        .or_insert_with(|| TimeHistogram::new(*timeline));

                                    let n = timeline_info.num_rows_for_all_entities_all_components
                                        as f64;
                                    let n = n * undo_factor;
                                    let n = n as u64;

                                    apply_estimate(
                                        Application::Add,
                                        histogram,
                                        timeline_info.time_range,
                                        n,
                                    );
                                }
                            }
                        }
                    }
                }

                ChunkStoreDiff::VirtualAddition(_) => {
                    // TODO(cmc): this should probably replace the `on_rrd_manifest` impl above.
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Application {
    Add,
    Remove,
}

impl Application {
    fn apply(self, histogram: &mut TimeHistogram, position: i64, inc: u32) {
        match self {
            Self::Add => {
                histogram.increment(position, inc);
            }
            Self::Remove => {
                histogram.decrement(position, inc);
            }
        }
    }
}

fn apply_estimate(
    application: Application,
    histogram: &mut TimeHistogram,
    time_range: re_log_types::AbsoluteTimeRange,
    num_rows_for_all_entities_all_components: u64,
) {
    if num_rows_for_all_entities_all_components == 0 {
        return;
    }

    // Assume even spread of chunk (for now):
    let num_pieces = u64::min(num_rows_for_all_entities_all_components, 10);

    if num_pieces == 1 || time_range.min == time_range.max {
        let position = time_range.center();
        application.apply(
            histogram,
            position.as_i64(),
            num_rows_for_all_entities_all_components as u32,
        );
    } else {
        let inc = (num_rows_for_all_entities_all_components / num_pieces) as u32;
        for i in 0..num_pieces {
            let position = lerp(
                time_range.min.as_f64()..=time_range.max.as_f64(),
                i as f64 / (num_pieces as f64 - 1.0),
            )
            .round() as i64;

            application.apply(
                histogram,
                position,
                inc + (i < num_rows_for_all_entities_all_components % num_pieces) as u32,
            );
        }
    }
}

impl re_byte_size::SizeBytes for TimeHistogram {
    fn heap_size_bytes(&self) -> u64 {
        let Self { timeline: _, hist } = self;

        // Calculating the memory use of the time histogram can be very slow.
        // (and not _very_ important), so we do a dumb heuristic here.
        // TODO(RR-3784): get rid of TimeHistogram completely
        hist.total_count() * (std::mem::size_of::<u64>() as u64)
    }
}

impl re_byte_size::SizeBytes for TimeHistogramPerTimeline {
    fn heap_size_bytes(&self) -> u64 {
        let Self { times, has_static } = self;
        times.heap_size_bytes() + has_static.heap_size_bytes()
    }
}

impl MemUsageTreeCapture for TimeHistogramPerTimeline {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        let Self { times, has_static } = self;
        _ = has_static;

        let mut node = MemUsageNode::new();
        for (timeline_name, histogram) in times {
            node.add(
                timeline_name.as_str().to_owned(),
                histogram.total_size_bytes(),
            );
        }
        node.into_tree()
    }
}
