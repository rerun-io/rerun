use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use ahash::{HashMap, HashSet};
use arrow::array::{Int32Array, RecordBatch};
use arrow::compute::take_record_batch;
use itertools::Itertools as _;
use nohash_hasher::{IntMap, IntSet};
use parking_lot::Mutex;
use re_chunk::{Chunk, ChunkId, TimeInt, Timeline, TimelineName};
use re_chunk_store::ChunkStoreEvent;
use re_log_encoding::{CodecResult, RrdManifest, RrdManifestTemporalMapEntry};
use re_log_types::{AbsoluteTimeRange, StoreKind};

use crate::chunk_promise::{ChunkPromise, ChunkPromiseBatch, ChunkPromises};
use crate::sorted_range_map::SortedRangeMap;

mod time_range_merger;

/// Errors that can occur during prefetching.
#[derive(thiserror::Error, Debug)]
pub enum PrefetchError {
    #[error("No manifest available")]
    NoManifest,

    #[error("Unknown timeline: {0:?}")]
    UnknownTimeline(Timeline),

    #[error("Codec: {0}")]
    Codec(#[from] re_log_encoding::CodecError),

    #[error("Arrow: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("Row index too large: {0}")]
    BadIndex(usize),
}

/// Is the following chunk loaded?
///
/// The order here is used for priority to show the state in the ui (lower is more prioritized)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum LoadState {
    /// The chunk is not loaded, nor being loaded.
    #[default]
    Unloaded,

    /// We have requested it.
    ///
    /// TODO(emilk): move this state to [`ChunkPromises`]
    InTransit,

    /// We have the chole chunk in memory.
    Loaded,
}

impl LoadState {
    pub fn is_loaded(&self) -> bool {
        !self.is_unloaded()
    }

    pub fn is_unloaded(&self) -> bool {
        match self {
            Self::Unloaded | Self::InTransit => true,
            Self::Loaded => false,
        }
    }
}

/// How to calculate which chunks to prefetch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkPrefetchOptions {
    pub timeline: Timeline,

    /// Start loading chunks from this time onwards,
    /// before looping back to the start.
    pub start_time: TimeInt,

    /// These chunks are required by one or more queries that are currently in-flight.
    ///
    /// We must try and download them in priority, as there are views actively waiting for them in order to
    /// properly render.
    pub missing_chunk_ids: HashSet<ChunkId>,

    /// Batch together requests until we reach this size).
    pub max_uncompressed_bytes_per_batch: u64,

    /// Total budget for all loaded chunks.
    pub total_uncompressed_byte_budget: u64,

    /// Maximum number of bytes in transit at once.
    pub max_uncompressed_bytes_in_transit: u64,
}

/// Info about a single chunk that we know ahead of loading it.
#[derive(Clone, Debug, Default)]
pub struct ChunkInfo {
    state: LoadState,

    /// None for static chunks
    pub temporal: Option<TemporalChunkInfo>,
}

#[derive(Clone, Copy, Debug)]
pub struct TemporalChunkInfo {
    pub timeline: Timeline,

    /// The time range covered by this entry.
    pub time_range: AbsoluteTimeRange,

    /// The number of rows in the original chunk which are associated with this entry.
    ///
    /// At most, this is the same as the number of rows in the chunk as a whole. For a specific
    /// entry it might be less, since chunks allow sparse components.
    pub num_rows: u64,
}

/// A secondary index that keeps track of which chunks have been loaded into memory.
///
/// This is constructed from an [`RrdManifest`], which is what
/// the server sends to the client/viewer.
#[derive(Default)]
#[cfg_attr(feature = "testing", derive(Clone))]
pub struct RrdManifestIndex {
    /// The raw manifest.
    ///
    /// This is known ahead-of-time for _some_ data sources.
    manifest: Option<RrdManifest>,

    /// These are the chunks known to exist in the data source (e.g. remote server).
    ///
    /// The chunk store may split large chunks and merge (compact) small ones,
    /// so what's in the chunk store can differ significantly.
    remote_chunks: HashMap<ChunkId, ChunkInfo>,

    chunk_promises: ChunkPromises,

    /// The chunk store may split large chunks and merge (compact) small ones.
    /// When we later drop a chunk, we need to know which other chunks to invalidate.
    parents: HashMap<ChunkId, HashSet<ChunkId>>,

    /// Full time range per timeline
    timelines: BTreeMap<TimelineName, AbsoluteTimeRange>,

    pub entity_tree: crate::EntityTree,
    entity_has_temporal_data_on_timeline: IntMap<re_chunk::EntityPath, IntSet<TimelineName>>,
    entity_has_static_data: IntSet<re_chunk::EntityPath>,

    static_chunk_ids: HashSet<ChunkId>,

    native_static_map: re_log_encoding::RrdManifestStaticMap,
    native_temporal_map: re_log_encoding::RrdManifestTemporalMap,

    chunk_intervals: BTreeMap<Timeline, SortedRangeMap<TimeInt, ChunkId>>,

    manifest_row_from_chunk_id: BTreeMap<ChunkId, usize>,
}

impl std::fmt::Debug for RrdManifestIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RrdManifestIndex").finish_non_exhaustive()
    }
}

impl RrdManifestIndex {
    pub fn append(&mut self, manifest: RrdManifest) -> CodecResult<()> {
        re_tracing::profile_function!();

        self.native_static_map = manifest.get_static_data_as_a_map()?;
        self.native_temporal_map = manifest.get_temporal_data_as_a_map()?;

        for entity_chunks in self.native_static_map.values() {
            for &chunk_id in entity_chunks.values() {
                self.static_chunk_ids.insert(chunk_id);
            }
        }

        self.update_timeline_stats();
        self.update_entity_tree();
        self.update_entity_temporal_data();
        self.update_entity_static_data();
        self.update_chunk_intervals();

        for chunk_id in manifest.col_chunk_id()? {
            self.remote_chunks.entry(chunk_id).or_default();
        }

        for timelines in self.native_temporal_map.values() {
            for (&timeline, comps) in timelines {
                for chunks in comps.values() {
                    for (&chunk_id, entry) in chunks {
                        let chunk_info = self.remote_chunks.entry(chunk_id).or_default();
                        chunk_info.temporal = Some(TemporalChunkInfo {
                            timeline,
                            time_range: entry.time_range,
                            num_rows: entry.num_rows,
                        });
                    }
                }
            }
        }

        if self.manifest.is_some() {
            re_log::warn!(
                "Received a second RRD manifest schema for the same recording. This is not yet supported."
            );
        }

        self.manifest_row_from_chunk_id.clear();
        let chunk_id = manifest.col_chunk_id()?;
        for (row_idx, chunk_id) in chunk_id.enumerate() {
            self.manifest_row_from_chunk_id.insert(chunk_id, row_idx);
        }

        self.manifest = Some(manifest);

        Ok(())
    }

    /// Info about a chunk that is in the manifest
    pub fn remote_chunk_info(&self, chunk_id: &ChunkId) -> Option<&ChunkInfo> {
        self.remote_chunks.get(chunk_id)
    }

    fn update_timeline_stats(&mut self) {
        for timelines in self.native_temporal_map.values() {
            for (timeline, comps) in timelines {
                let mut timeline_range = self
                    .timelines
                    .get(timeline.name())
                    .copied()
                    .unwrap_or(AbsoluteTimeRange::EMPTY);

                for chunks in comps.values() {
                    for entry in chunks.values() {
                        timeline_range = timeline_range.union(entry.time_range);
                    }
                }

                if timeline_range != AbsoluteTimeRange::EMPTY {
                    self.timelines.insert(*timeline.name(), timeline_range);
                }
            }
        }
    }

    fn update_entity_tree(&mut self) {
        for entity in self
            .native_static_map
            .keys()
            .chain(self.native_temporal_map.keys())
        {
            self.entity_tree.on_new_entity(entity);
        }
    }

    fn update_entity_temporal_data(&mut self) {
        for (entity, timelines) in &self.native_temporal_map {
            self.entity_has_temporal_data_on_timeline
                .entry(entity.clone())
                .or_default()
                .extend(timelines.keys().map(|t| *t.name()));
        }
    }

    fn update_entity_static_data(&mut self) {
        for entity in self.native_static_map.keys() {
            self.entity_has_static_data.insert(entity.clone());
        }
    }

    fn update_chunk_intervals(&mut self) {
        let mut per_timeline_chunks: BTreeMap<Timeline, Vec<(RangeInclusive<TimeInt>, ChunkId)>> =
            BTreeMap::default();

        for timelines in self.native_temporal_map.values() {
            for (timeline, components) in timelines {
                let timeline_chunks = per_timeline_chunks.entry(*timeline).or_default();
                for chunks in components.values() {
                    for (chunk_id, entry) in chunks {
                        timeline_chunks.push((entry.time_range.into(), *chunk_id));
                    }
                }
            }
        }

        self.chunk_intervals.clear();
        for (timeline, chunks) in per_timeline_chunks {
            self.chunk_intervals
                .insert(timeline, SortedRangeMap::new(chunks));
        }
    }

    pub fn entity_has_temporal_data_on_timeline(
        &self,
        entity: &re_chunk::EntityPath,
        timeline: &TimelineName,
    ) -> bool {
        self.entity_has_temporal_data_on_timeline
            .get(entity)
            .is_some_and(|timelines| timelines.contains(timeline))
    }

    pub fn entity_has_static_data(&self, entity: &re_chunk::EntityPath) -> bool {
        self.entity_has_static_data.contains(entity)
    }

    pub fn entity_has_data_on_timeline(
        &self,
        entity: &re_chunk::EntityPath,
        timeline: &TimelineName,
    ) -> bool {
        self.entity_has_static_data(entity)
            || self.entity_has_temporal_data_on_timeline(entity, timeline)
    }

    /// False for recordings streamed from SDK via proxy
    pub fn has_manifest(&self) -> bool {
        self.manifest.is_some()
    }

    /// The full manifest, if known.
    pub fn manifest(&self) -> Option<&RrdManifest> {
        self.manifest.as_ref()
    }

    pub fn native_temporal_map(&self) -> &re_log_encoding::RrdManifestTemporalMap {
        &self.native_temporal_map
    }

    pub fn mark_as_loaded(&mut self, chunk_id: ChunkId) {
        let chunk_info = self.remote_chunks.entry(chunk_id).or_default();
        chunk_info.state = LoadState::Loaded;
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
                        chunk_info.state = LoadState::Loaded;
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
        if let Some(chunk_info) = self.remote_chunks.get_mut(chunk_id) {
            chunk_info.state = LoadState::Unloaded;
        } else if let Some(parents) = self.parents.remove(chunk_id) {
            // Mark all ancestors as not being fully loaded:

            let mut ancestors = parents.into_iter().collect_vec();
            while let Some(chunk_id) = ancestors.pop() {
                if let Some(chunk_info) = self.remote_chunks.get_mut(&chunk_id) {
                    chunk_info.state = LoadState::Unloaded;
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

    /// See if we have received any new chunks since last call.
    pub fn resolve_pending_promises(&mut self) -> Vec<Chunk> {
        self.chunk_promises.resolve_pending()
    }

    pub fn has_pending_promises(&self) -> bool {
        self.chunk_promises.has_pending()
    }

    /// Find the next candidates for prefetching.
    pub fn prefetch_chunks(
        &mut self,
        options: ChunkPrefetchOptions,
        load_chunks: &dyn Fn(RecordBatch) -> ChunkPromise,
    ) -> Result<(), PrefetchError> {
        re_tracing::profile_function!();

        let ChunkPrefetchOptions {
            timeline,
            missing_chunk_ids,
            start_time,
            max_uncompressed_bytes_per_batch,
            total_uncompressed_byte_budget,
            mut max_uncompressed_bytes_in_transit,
        } = options;

        let Some(manifest) = self.manifest.as_ref() else {
            return Err(PrefetchError::NoManifest);
        };

        let Some(chunks) = self.chunk_intervals.get(&timeline) else {
            return Err(PrefetchError::UnknownTimeline(timeline));
        };

        let mut remaining_uncompressed_byte_budget = total_uncompressed_byte_budget;

        max_uncompressed_bytes_in_transit = max_uncompressed_bytes_in_transit
            .saturating_sub(self.chunk_promises.num_uncompressed_bytes_pending());

        if max_uncompressed_bytes_in_transit == 0 {
            return Ok(());
        }

        let chunk_byte_size_uncompressed_raw: &[u64] =
            manifest.col_chunk_byte_size_uncompressed_raw()?.values();

        let mut uncompressed_bytes_in_batch: u64 = 0;
        let mut indices = vec![];

        let missing_chunk_ids = missing_chunk_ids.into_iter();
        let chunks_ids_after_time_cursor = || {
            chunks
                .query(start_time..=TimeInt::MAX)
                .map(|(_, chunk_id)| *chunk_id)
        };
        let chunks_ids_before_time_cursor = || {
            chunks
                .query(TimeInt::MIN..=start_time.saturating_sub(1))
                .map(|(_, chunk_id)| *chunk_id)
        };
        let chunk_ids_in_priority_order = itertools::chain!(
            self.static_chunk_ids.iter().copied(),
            missing_chunk_ids,
            std::iter::once_with(chunks_ids_after_time_cursor).flatten(),
            std::iter::once_with(chunks_ids_before_time_cursor).flatten(),
        );

        let entity_paths = manifest.col_chunk_entity_path_raw()?;

        // We might reach our budget limits before we enqueue all `missing_chunk_ids`.
        // That's fine: they will still be missing next frame, and therefore will still be reported
        // as such by the store.
        for chunk_id in chunk_ids_in_priority_order {
            let Some(remote_chunk) = self.remote_chunks.get_mut(&chunk_id) else {
                re_log::warn_once!("Chunk {chunk_id:?} not found in RRD manifest index");
                continue;
            };

            let row_idx = self.manifest_row_from_chunk_id[&chunk_id];

            // We count only the chunks we are interested in as being part of the memory budget.
            // The others can/will be evicted as needed.
            let uncompressed_chunk_size = chunk_byte_size_uncompressed_raw[row_idx];

            if total_uncompressed_byte_budget < uncompressed_chunk_size {
                // TODO(RR-3344): improve this error message
                let entity_path = entity_paths.value(row_idx);
                if cfg!(target_arch = "wasm32") {
                    re_log::debug_once!(
                        "Cannot load all of entity '{entity_path}', because its size exceeds the memory budget. Try the native viewer instead, or split up your large assets (e.g. prefer VideoStream over VideoAsset)."
                    );
                } else {
                    re_log::warn_once!(
                        "Cannot load all of entity '{entity_path}', because its size exceeds the memory budget. You should increase the `--memory-limit` or try to split up your large assets (e.g. prefer VideoStream over VideoAsset)."
                    );
                }
                continue;
            }

            {
                // Can we fit this chunk in memory?
                remaining_uncompressed_byte_budget =
                    remaining_uncompressed_byte_budget.saturating_sub(uncompressed_chunk_size);
                if remaining_uncompressed_byte_budget == 0 {
                    break; // We've already loaded too much.
                }
            }

            if remote_chunk.state == LoadState::Unloaded {
                let Ok(row_idx) = i32::try_from(row_idx) else {
                    return Err(PrefetchError::BadIndex(row_idx)); // Very improbable
                };

                indices.push(row_idx);
                uncompressed_bytes_in_batch += uncompressed_chunk_size;
                remote_chunk.state = LoadState::InTransit;

                if max_uncompressed_bytes_per_batch < uncompressed_bytes_in_batch {
                    let rb = take_record_batch(
                        &manifest.data,
                        &Int32Array::from(std::mem::take(&mut indices)),
                    )?;
                    self.chunk_promises.add(ChunkPromiseBatch {
                        promise: Mutex::new(Some(load_chunks(rb))),
                        size_bytes_uncompressed: uncompressed_bytes_in_batch,
                    });
                    uncompressed_bytes_in_batch = 0;
                }

                // We enqueue it first, then decrement the budget, ensuring that we still download
                // big chunks that are outside the `max_uncompressed_bytes_in_transit` limit.
                max_uncompressed_bytes_in_transit =
                    max_uncompressed_bytes_in_transit.saturating_sub(uncompressed_chunk_size);
                if max_uncompressed_bytes_in_transit == 0 {
                    break; // We've hit our budget.
                }
            }
        }

        if !indices.is_empty() {
            let rb = take_record_batch(&manifest.data, &Int32Array::from(indices))?;
            self.chunk_promises.add(ChunkPromiseBatch {
                promise: Mutex::new(Some(load_chunks(rb))),
                size_bytes_uncompressed: uncompressed_bytes_in_batch,
            });
        }

        Ok(())
    }

    /// Creates an iterator of time ranges which are loaded on a specific timeline.
    ///
    /// The ranges are guaranteed to be ordered and non-overlapping.
    pub fn loaded_ranges_on_timeline(
        &self,
        timeline: &Timeline,
    ) -> impl Iterator<Item = AbsoluteTimeRange> {
        re_tracing::profile_function!();

        let mut scratch = Vec::new();
        let mut ranges = Vec::new();

        // First we merge ranges for individual components, since chunks' time ranges
        // often have gaps which we don't want to display other components' chunks
        // loaded state in.
        for timelines in self.native_temporal_map.values() {
            let Some(data) = timelines.get(timeline) else {
                continue;
            };

            re_tracing::profile_scope!("timeline", timeline.name().as_str());

            for chunks in data.values() {
                scratch.extend(chunks.iter().filter_map(|(c, range)| {
                    let state = self.remote_chunk_info(c)?.state;

                    Some(time_range_merger::TimeRange {
                        range: range.time_range,
                        loaded: state.is_loaded(),
                    })
                }));

                ranges.extend(time_range_merger::merge_ranges(&scratch));

                scratch.clear();
            }
        }

        time_range_merger::merge_ranges(&ranges)
            .into_iter()
            .filter(|r| r.loaded)
            .map(|r| r.range)
    }

    /// If `component` is some, this returns all unloaded temporal entries for that specific
    /// component on the given timeline.
    ///
    /// If not, this returns all unloaded temporal entries for `entity`'s components and its
    /// descendants' unloaded temporal entries.
    pub fn unloaded_temporal_entries_for(
        &self,
        timeline: &re_chunk::Timeline,
        entity: &re_chunk::EntityPath,
        component: Option<re_chunk::ComponentIdentifier>,
    ) -> Vec<RrdManifestTemporalMapEntry> {
        re_tracing::profile_function!();

        if let Some(component) = component {
            let Some(entity_ranges_per_timeline) = self.native_temporal_map.get(entity) else {
                return Vec::new();
            };

            let Some(entity_ranges) = entity_ranges_per_timeline.get(timeline) else {
                return Vec::new();
            };

            let Some(component_ranges) = entity_ranges.get(&component) else {
                return Vec::new();
            };

            component_ranges
                .iter()
                .filter(|(chunk, _)| self.is_chunk_unloaded(chunk))
                .map(|(_, entry)| *entry)
                .collect()
        } else {
            // If we don't have a specific component we want to include the entity's children
            let mut res = Vec::new();

            if let Some(tree) = self.entity_tree.subtree(entity) {
                tree.visit_children_recursively(|child| {
                    self.unloaded_temporal_entries_for_entity(&mut res, timeline, child);
                });
            } else {
                #[cfg(debug_assertions)]
                re_log::warn_once!(
                    "[DEBUG] Missing entity tree for {entity} while fetching temporal entities"
                );

                self.unloaded_temporal_entries_for_entity(&mut res, timeline, entity);
            }

            res
        }
    }

    /// Fills `ranges` with unloaded temporal entries for this exact entity (descendants aren't included).
    fn unloaded_temporal_entries_for_entity(
        &self,
        ranges: &mut Vec<RrdManifestTemporalMapEntry>,
        timeline: &re_chunk::Timeline,
        entity: &re_chunk::EntityPath,
    ) {
        re_tracing::profile_function!();

        if let Some(entity_ranges_per_timeline) = self.native_temporal_map.get(entity)
            && let Some(entity_ranges) = entity_ranges_per_timeline.get(timeline)
        {
            for (_, entry) in entity_ranges
                .values()
                .flatten()
                .filter(|(chunk, _)| self.is_chunk_unloaded(chunk))
            {
                ranges.push(*entry);
            }
        }
    }

    fn is_chunk_unloaded(&self, chunk_id: &ChunkId) -> bool {
        self.remote_chunks
            .get(chunk_id)
            .is_none_or(|c| c.state.is_unloaded())
    }

    pub fn full_uncompressed_size(&self) -> Option<u64> {
        re_tracing::profile_function!();
        Some(
            self.manifest()?
                .col_chunk_byte_size_uncompressed_raw()
                .ok()?
                .values()
                .iter()
                .sum(),
        )
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
