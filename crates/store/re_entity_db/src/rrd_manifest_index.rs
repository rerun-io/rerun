use std::collections::BTreeMap;

use ahash::{HashMap, HashSet};
use arrow::array::{Int32Array, RecordBatch};
use arrow::compute::take_record_batch;
use itertools::{Either, Itertools as _, izip};
use parking_lot::Mutex;
use re_arrow_util::RecordBatchExt as _;
use re_chunk::{ChunkId, Timeline, TimelineName};
use re_chunk_store::ChunkStoreEvent;
use re_log_encoding::{CodecResult, NativeTemporalMapEntry, RrdManifest};
use re_log_types::{AbsoluteTimeRange, StoreKind, TimeType};

use crate::{TimelineStats, TimesPerTimeline};

/// Is the following chunk loaded?
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoadState {
    /// The chunk is not loaded, nor being loaded.
    #[default]
    Unloaded,

    /// We have requested it.
    InTransit,

    /// We have the chole chunk in memory.
    Loaded,
}

/// Info about a single chunk that we know ahead of loading it.
pub struct ChunkInfo {
    pub state: Mutex<LoadState>, // Mutex here is a bit ugly…
}

impl Clone for ChunkInfo {
    fn clone(&self) -> Self {
        Self {
            state: Mutex::new(*self.state.lock()),
        }
    }
}

impl std::fmt::Debug for ChunkInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkInfo")
            .field("state", &*self.state.lock())
            .finish()
    }
}

impl Default for ChunkInfo {
    fn default() -> Self {
        Self {
            state: Mutex::new(LoadState::Unloaded),
        }
    }
}

/// A secondary index that keeps track of which chunks have been loaded into memory.
///
/// This is currently used to show a progress bar.
///
/// This is constructed from an [`RrdManifest`], which is what
/// the server sends to the client/viewer.
/// TODO(RR-2999): use this for larger-than-RAM.
#[derive(Default, Debug, Clone)]
pub struct RrdManifestIndex {
    /// The raw manifest.
    ///
    /// This is known ahead-of-time for _some_ data sources.
    manifest: Option<RrdManifest>,

    /// These are the chunks known to exist in the data source (e.g. remote server).
    ///
    /// The chunk store may split large chunks and merge (compact) small ones,
    /// so what's in the chunk store can differ significantally.
    remote_chunks: HashMap<ChunkId, ChunkInfo>,

    /// The chunk store may split large chunks and merge (compact) small ones.
    /// When we later drop a chunk, we need to know which other chunks to invalidate.
    parents: HashMap<ChunkId, HashSet<ChunkId>>,

    /// Have we ever deleted a chunk?
    ///
    /// If so, we have run some GC and should not show progress bar.
    has_deleted: bool,

    /// Full time range per timeline
    pub timelines: BTreeMap<TimelineName, AbsoluteTimeRange>,

    pub times_per_timeline: TimesPerTimeline,

    native_temporal_map: re_log_encoding::NativeTemporalMap,
}

impl RrdManifestIndex {
    pub fn append(&mut self, manifest: RrdManifest) -> CodecResult<()> {
        re_tracing::profile_function!();

        self.native_temporal_map = manifest.to_native_temporal()?;

        for timelines in self.native_temporal_map.values() {
            for (timeline, comps) in timelines {
                let mut timeline_range = self
                    .timelines
                    .get(timeline.name())
                    .copied()
                    .unwrap_or(AbsoluteTimeRange::EMPTY);

                for chunks in comps.values() {
                    for entry in chunks.values() {
                        let NativeTemporalMapEntry {
                            time_range: chunk_range,
                            num_rows, // TODO: Emil, wanna do something with this?
                        } = entry;

                        timeline_range = timeline_range.union(*chunk_range);

                        // TODO: this is a bad idea
                        let timeline_stats = self
                            .times_per_timeline
                            .0
                            .entry(*timeline.name())
                            .or_insert_with(|| TimelineStats::new(*timeline));
                        *timeline_stats.per_time.entry(chunk_range.min).or_default() += 1;
                        *timeline_stats.per_time.entry(chunk_range.max).or_default() += 1;
                        timeline_stats.total_count += 2;
                    }
                }

                if timeline_range != AbsoluteTimeRange::EMPTY {
                    self.timelines.insert(*timeline.name(), timeline_range);
                }
            }
        }

        for chunk_id in manifest.col_chunk_id()? {
            self.remote_chunks.entry(chunk_id).or_default();
            // TODO(RR-2999): update chunk info?
        }
        self.manifest = Some(manifest);
        Ok(())
    }

    /// False for recordings streamed from SDK via proxy
    pub fn has_manifest(&self) -> bool {
        self.manifest.is_some()
    }

    /// The full manifest, if known.
    pub fn manifest(&self) -> Option<&RrdManifest> {
        self.manifest.as_ref()
    }

    pub fn native_temporal_map(&self) -> &re_log_encoding::NativeTemporalMap {
        &self.native_temporal_map
    }

    /// [0, 1], how many chunks have been loaded?
    ///
    /// Returns `None` if we have already started garbage-collecting some chunks.
    pub fn progress(&self) -> Option<f32> {
        #[expect(clippy::question_mark)]
        if self.manifest.is_none() {
            return None;
        }

        let num_remote_chunks = self.remote_chunks.len();

        if self.has_deleted {
            None
        } else if num_remote_chunks == 0 {
            Some(1.0)
        } else {
            let num_loaded = self
                .remote_chunks
                .values()
                .filter(|c| *c.state.lock() == LoadState::Loaded)
                .count();
            Some(num_loaded as f32 / num_remote_chunks as f32)
        }
    }

    pub fn mark_as_loaded(&mut self, chunk_id: ChunkId) {
        let chunk_info = self.remote_chunks.entry(chunk_id).or_default();
        *chunk_info.state.lock() = LoadState::Loaded;
    }

    pub fn on_events(&mut self, store_events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        if self.manifest.is_none() {
            return;
        }

        for event in store_events {
            let store_kind = event.store_id.kind();
            let chunk_id = event.chunk.id();
            match event.kind {
                re_chunk_store::ChunkStoreDiffKind::Addition => {
                    if let Some(chunk_info) = self.remote_chunks.get_mut(&chunk_id) {
                        *chunk_info.state.lock() = LoadState::Loaded;
                    } else if let Some(source) = event.split_source {
                        // The added chunk was the result of splitting another chunk:
                        self.parents.entry(chunk_id).or_default().insert(source);
                    } else {
                        warn_when_editing_recording(
                            store_kind,
                            "Added chunk that was not part of the chunk index",
                        );
                    }
                }
                re_chunk_store::ChunkStoreDiffKind::Deletion => {
                    self.mark_deleted(store_kind, &chunk_id);
                }
            }
        }
    }

    fn mark_deleted(&mut self, store_kind: StoreKind, chunk_id: &ChunkId) {
        self.has_deleted = true;

        if let Some(chunk_info) = self.remote_chunks.get_mut(chunk_id) {
            *chunk_info.state.lock() = LoadState::Unloaded;
        } else if let Some(parents) = self.parents.remove(chunk_id) {
            // Mark all ancestors as not being fully loaded:

            let mut ancestors = parents.into_iter().collect_vec();
            while let Some(chunk_id) = ancestors.pop() {
                if let Some(chunk_info) = self.remote_chunks.get_mut(&chunk_id) {
                    *chunk_info.state.lock() = LoadState::Unloaded;
                } else if let Some(grandparents) = self.parents.get(&chunk_id) {
                    ancestors.extend(grandparents);
                } else {
                    warn_when_editing_recording(
                        store_kind,
                        "Removed ancestor chunk that was not part of the index",
                    );
                }
            }
        } else {
            warn_when_editing_recording(store_kind, "Removed chunk that was not part of the index");
        }
    }

    /// When do we have data on this timeline?
    pub fn timeline_range(&self, timeline: &TimelineName) -> Option<AbsoluteTimeRange> {
        self.timelines.get(timeline).copied()
    }

    /// Returns the yet-to-be-loaded chunks
    pub fn time_range_missing_chunks(
        &self,
        timeline: &Timeline,
        query_range: AbsoluteTimeRange,
    ) -> anyhow::Result<RecordBatch> {
        re_tracing::profile_function!();
        // Find the indices of all chunks that overlaps the query, then select those rows of the record batch.

        let Some(manifest) = self.manifest.as_ref() else {
            anyhow::bail!("No manifest");
        };
        let record_batch = &manifest.data;

        let mut indices = vec![];

        let chunk_id = manifest.col_chunk_id()?;
        let chunk_is_static = manifest.col_chunk_is_static()?;
        let (_, start_column) = TimeType::from_arrow_array(
            record_batch.try_get_column(RrdManifest::field_index_start(timeline, None).name())?,
        )?;
        let (_, end_column) = TimeType::from_arrow_array(
            record_batch.try_get_column(RrdManifest::field_index_end(timeline, None).name())?,
        )?;

        for (row_idx, (chunk_id, chunk_is_static, start_time, end_time)) in
            izip!(chunk_id, chunk_is_static, start_column, end_column).enumerate()
        {
            let chunk_range = AbsoluteTimeRange::new(*start_time, *end_time);
            let include = chunk_is_static || chunk_range.intersects(query_range);
            if include {
                if let Some(chunk_info) = self.remote_chunks.get(&chunk_id) {
                    if *chunk_info.state.lock() == LoadState::Unloaded {
                        *chunk_info.state.lock() = LoadState::InTransit;
                        indices.push(row_idx as i32);
                    }
                }
            }
        }

        Ok(take_record_batch(
            &manifest.data,
            &Int32Array::from(indices),
        )?)
    }

    #[must_use]
    pub fn time_ranges_all_chunks(
        &self,
        timeline: &Timeline,
    ) -> Vec<(LoadState, AbsoluteTimeRange)> {
        let mut time_ranges_all_chunks = Vec::new();

        for timelines in self.native_temporal_map.values() {
            let Some(entity_component_chunks) = timelines.get(timeline) else {
                continue;
            };

            for chunks in entity_component_chunks.values() {
                for (chunk_id, entry) in chunks {
                    let NativeTemporalMapEntry {
                        time_range,
                        num_rows: _, // TODO: Isse, wanna do something with this?
                    } = entry;

                    let Some(info) = self.remote_chunks.get(chunk_id) else {
                        continue;
                    };
                    time_ranges_all_chunks.push((*info.state.lock(), *time_range));
                }
            }
        }

        time_ranges_all_chunks
    }

    pub fn unloaded_time_ranges_for(
        &self,
        timeline: &re_chunk::Timeline,
        entity: &re_chunk::EntityPath,
        component: Option<re_chunk::ComponentIdentifier>,
    ) -> Vec<(AbsoluteTimeRange, u64)> {
        re_tracing::profile_function!();

        let Some(entity_ranges_per_timeline) = self.native_temporal_map.get(entity) else {
            return Vec::new();
        };

        let Some(entity_ranges) = entity_ranges_per_timeline.get(timeline) else {
            return Vec::new();
        };

        let component_ranges = if let Some(component) = component {
            let Some(component_ranges) = entity_ranges.get(&component) else {
                return Vec::new();
            };

            Either::Left(std::iter::once(component_ranges))
        } else {
            Either::Right(entity_ranges.values())
        };

        component_ranges
            .into_iter()
            .flatten()
            .filter(|(chunk, _)| {
                self.remote_chunks
                    .get(chunk)
                    .is_none_or(|c| match *c.state.lock() {
                        LoadState::InTransit | LoadState::Unloaded => true,
                        LoadState::Loaded => false,
                    })
            })
            .map(|(_, entry)| (entry.time_range, 1))
            .collect()
    }
}

#[track_caller]
fn warn_when_editing_recording(store_kind: StoreKind, warning: &str) {
    match store_kind {
        StoreKind::Recording => {
            if cfg!(debug_assertions) {
                re_log::warn_once!("[DEBUG] {warning}");
            } else {
                re_log::debug_once!("{warning}");
            }
        }
        StoreKind::Blueprint => {
            // We edit blueprint by generating new chunks in the viewer.
        }
    }
}
