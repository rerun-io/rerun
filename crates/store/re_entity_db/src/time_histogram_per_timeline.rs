use std::collections::BTreeMap;

use emath::lerp;
use itertools::Itertools as _;
use re_chunk::TimelineName;
use re_chunk_store::{ChunkStoreDiffKind, ChunkStoreEvent};
use re_log_encoding::RrdManifestTemporalMapEntry;

use crate::RrdManifestIndex;

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

    /// If we know the manifest ahead of time, we can pre-populate
    /// the histogram with a rough estimate of the final form.
    pub fn on_rrd_manifest(
        &mut self,
        rrd_manifest: &re_log_encoding::RrdManifest,
    ) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        let native_temporal_map = rrd_manifest.get_temporal_data_as_a_map()?;

        for timelines in native_temporal_map.values() {
            for (timeline, comps) in timelines {
                let histogram = self.times.entry(*timeline.name()).or_default();
                for chunks in comps.values() {
                    for entry in chunks.values() {
                        let RrdManifestTemporalMapEntry {
                            time_range,
                            num_rows,
                        } = *entry;

                        apply_fake(Application::Add, histogram, time_range, num_rows);
                    }
                }
            }
        }

        Ok(())
    }

    fn add(&mut self, times_per_timeline: &[(TimelineName, &[i64])], n: u32) {
        re_tracing::profile_function!();

        for &(timeline_name, times) in times_per_timeline {
            let histogram = self.times.entry(timeline_name).or_default();
            for &time in times {
                histogram.increment(time, n);
            }
        }
    }

    fn remove(&mut self, times_per_timeline: &[(TimelineName, &[i64])], n: u32) {
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

    pub fn on_events(&mut self, rrd_manifest_index: &RrdManifestIndex, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        for event in events {
            let times = event
                .chunk
                .timelines()
                .iter()
                .map(|(&timeline_name, time_column)| (timeline_name, time_column.times_raw()))
                .collect_vec();

            let original_chunk_id = if let Some(chunk_id) = event.diff.split_source {
                chunk_id
            } else {
                event.chunk.id()
            };

            match event.kind {
                ChunkStoreDiffKind::Addition => {
                    if let Some(info) = rrd_manifest_index.remote_chunk_info(&original_chunk_id)
                        && let Some(info) = &info.temporal
                    {
                        // We added fake value for this before. Now that we have the whole chunk we need to subtract those fake values again.
                        let histogram = self.times.entry(*info.timeline.name()).or_default();
                        apply_fake(
                            Application::Remove,
                            histogram,
                            info.time_range,
                            info.num_rows,
                        );
                    }

                    self.add(&times, event.num_components() as _);
                }
                ChunkStoreDiffKind::Deletion => {
                    self.remove(&times, event.num_components() as _);

                    // We GCed the full chunk, so add back the fake:

                    if let Some(info) = rrd_manifest_index.remote_chunk_info(&original_chunk_id)
                        && let Some(info) = &info.temporal
                    {
                        // We added fake value for this before. Now that we have the whole chunk we need to subtract those fake values again.
                        let histogram = self.times.entry(*info.timeline.name()).or_default();
                        apply_fake(Application::Add, histogram, info.time_range, info.num_rows);
                    }
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
    fn apply(self, histogram: &mut re_int_histogram::Int64Histogram, position: i64, inc: u32) {
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

fn apply_fake(
    application: Application,
    histogram: &mut re_int_histogram::Int64Histogram,
    time_range: re_log_types::AbsoluteTimeRange,
    num_rows: u64,
) {
    if num_rows == 0 {
        return;
    }

    // Assume even spread of chunk (for now):
    let num_pieces = u64::min(num_rows, 10);

    if num_pieces == 1 || time_range.min == time_range.max {
        let position = time_range.center();
        application.apply(histogram, position.as_i64(), num_rows as u32);
    } else {
        let inc = (num_rows / num_pieces) as _;
        for i in 0..num_pieces {
            let position = lerp(
                time_range.min.as_f64()..=time_range.max.as_f64(),
                i as f64 / (num_pieces as f64 - 1.0),
            )
            .round() as i64;

            match application {
                Application::Add => {
                    histogram.increment(position, inc);
                }
                Application::Remove => {
                    histogram.decrement(position, inc);
                }
            }
        }
    }
}
