use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use ahash::{HashMap, HashSet};
use arrow::array::RecordBatch;
use arrow::compute::take_record_batch;
use itertools::Itertools as _;
use nohash_hasher::{IntMap, IntSet};
use re_chunk::{ChunkId, TimeInt, Timeline, TimelineName};
use re_chunk_store::ChunkStoreEvent;
use re_log_encoding::{CodecResult, RrdManifest, RrdManifestTemporalMapEntry};
use re_log_types::{AbsoluteTimeRange, StoreKind};

use crate::sorted_range_map::SortedRangeMap;

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
pub enum LoadState {
    /// The chunk is not loaded, nor being loaded.
    #[default]
    Unloaded,

    /// We have requested it.
    InTransit,

    /// We have the chole chunk in memory.
    Loaded,
}

/// How to calculate which chunks to prefetch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkPrefetchOptions {
    pub timeline: Timeline,

    /// Only consider chunks overlapping this range on [`Self::timeline`].
    pub desired_range: AbsoluteTimeRange,

    /// Total budget for all loaded chunks.
    pub total_byte_budget: u64,

    /// Budget for this specific prefetch request.
    pub delta_byte_budget: u64,
}

/// Info about a single chunk that we know ahead of loading it.
#[derive(Clone, Debug, Default)]
pub struct ChunkInfo {
    pub state: LoadState,

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
    timelines: BTreeMap<TimelineName, AbsoluteTimeRange>,

    pub entity_tree: crate::EntityTree,
    entity_has_temporal_data_on_timeline: IntMap<re_chunk::EntityPath, IntSet<TimelineName>>,
    entity_has_static_data: IntSet<re_chunk::EntityPath>,

    native_static_map: re_log_encoding::RrdManifestStaticMap,
    native_temporal_map: re_log_encoding::RrdManifestTemporalMap,

    chunk_intervals: BTreeMap<Timeline, SortedRangeMap<TimeInt, ChunkId>>,

    manifest_row_from_chunk_id: BTreeMap<ChunkId, usize>,
}

impl RrdManifestIndex {
    pub fn append(&mut self, manifest: RrdManifest) -> CodecResult<()> {
        re_tracing::profile_function!();

        self.native_static_map = manifest.get_static_data_as_a_map()?;
        self.native_temporal_map = manifest.get_temporal_data_as_a_map()?;

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
        self.has_deleted = true;

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

    /// Find the next candidates for prefetching.
    pub fn prefetch_chunks(
        &mut self,
        options: &ChunkPrefetchOptions,
    ) -> Result<RecordBatch, PrefetchError> {
        re_tracing::profile_function!();

        let ChunkPrefetchOptions {
            timeline,
            desired_range,
            mut total_byte_budget,
            mut delta_byte_budget,
        } = *options;

        let Some(manifest) = self.manifest.as_ref() else {
            return Err(PrefetchError::NoManifest);
        };

        let Some(chunks) = self.chunk_intervals.get(&timeline) else {
            return Err(PrefetchError::UnknownTimeline(timeline));
        };

        let chunk_byte_size_uncompressed_raw: &[u64] =
            manifest.col_chunk_byte_size_uncompressed_raw()?.values();
        let mut indices: Vec<u32> = vec![];

        for (_, chunk_id) in chunks.query(desired_range.into()) {
            let Some(remote_chunk) = self.remote_chunks.get_mut(chunk_id) else {
                re_log::warn_once!("Chunk {chunk_id:?} not found in RRD manifest index");
                continue;
            };

            let row_idx = self.manifest_row_from_chunk_id[chunk_id];

            let chunk_size = chunk_byte_size_uncompressed_raw[row_idx];
            total_byte_budget = total_byte_budget.saturating_sub(chunk_size);
            if total_byte_budget == 0 {
                break; // We've already loaded too much.
            }

            if remote_chunk.state == LoadState::Unloaded {
                remote_chunk.state = LoadState::InTransit;

                if let Ok(row_idx) = u32::try_from(row_idx) {
                    indices.push(row_idx);
                } else {
                    // Improbable
                    return Err(PrefetchError::BadIndex(row_idx));
                }

                delta_byte_budget = delta_byte_budget.saturating_sub(chunk_size);
                if delta_byte_budget == 0 {
                    break; // We aren't allowed to prefetch more than this in one go.
                }
            }
        }

        Ok(take_record_batch(
            &manifest.data,
            &arrow::array::UInt32Array::from(indices),
        )?)
    }

    #[must_use]
    pub fn time_ranges_all_chunks(
        &self,
        timeline: &Timeline,
    ) -> Vec<(LoadState, AbsoluteTimeRange)> {
        re_tracing::profile_function!();

        let mut time_ranges_all_chunks = Vec::new();

        for timelines in self.native_temporal_map.values() {
            let Some(entity_component_chunks) = timelines.get(timeline) else {
                continue;
            };

            for chunks in entity_component_chunks.values() {
                for (chunk_id, entry) in chunks {
                    let RrdManifestTemporalMapEntry { time_range, .. } = entry;

                    let Some(info) = self.remote_chunks.get(chunk_id) else {
                        continue;
                    };
                    debug_assert!(
                        time_range.min <= time_range.max,
                        "Unexpected negative time range in RRD manifest"
                    );
                    time_ranges_all_chunks.push((info.state, *time_range));
                }
            }
        }

        time_ranges_all_chunks
    }

    pub fn loaded_ranges_on_timeline(
        &self,
        timeline: &Timeline,
    ) -> impl Iterator<Item = AbsoluteTimeRange> {
        fn merge_ranges(
            ranges: &mut Vec<(bool, AbsoluteTimeRange)>,
        ) -> Vec<(bool, AbsoluteTimeRange)> {
            ranges.sort_by_key(|(_, r)| r.min);
            let mut new_ranges = Vec::new();
            let mut delayed_ranges = Vec::<(bool, AbsoluteTimeRange)>::new();
            let mut add_range =
                |loaded: bool,
                 mut range: AbsoluteTimeRange,
                 delayed_ranges: &mut Vec<(bool, AbsoluteTimeRange)>| {
                    let Some((last_loaded, last_range)) = new_ranges.last_mut() else {
                        new_ranges.push((loaded, range));
                        return;
                    };

                    match (*last_loaded).cmp(&loaded) {
                        // Equal states for both ranges, combine them.
                        std::cmp::Ordering::Equal => {
                            last_range.max = last_range.max.max(range.max);
                        }
                        // The last state should be prioritized
                        std::cmp::Ordering::Less => {
                            if last_range.max <= range.min {
                                // To not leave any gaps between states, expand the prioritized last state
                                last_range.max = range.min;
                                new_ranges.push((loaded, range));
                            } else if last_range.max < range.max {
                                // To not have overlapping states, start the current state at the end of the prioritized last state
                                range.min = last_range.max;
                                delayed_ranges.push((loaded, range));
                            }
                        }
                        // The current state should be prioritized
                        std::cmp::Ordering::Greater => {
                            if range.min <= last_range.max {
                                // To not have overlapping states, start the last state at the end of the prioritized current state
                                if range.max < last_range.max {
                                    delayed_ranges.push((
                                        *last_loaded,
                                        AbsoluteTimeRange::new(range.max, last_range.max),
                                    ));
                                }

                                if last_range.min == range.min {
                                    // We can replace the last here since we don't want overlapping states
                                    *last_range = range;
                                    *last_loaded = loaded;
                                } else {
                                    last_range.max = range.min;

                                    new_ranges.push((loaded, range));
                                }
                            } else {
                                // To not leave any gaps between states, expand the prioritized current state
                                // to start at the end of the last state
                                range.min = last_range.max;
                                new_ranges.push((loaded, range));
                            }
                        }
                    }
                };

            let rev_cmp = |(_, r): &(bool, AbsoluteTimeRange)| -r.min.as_i64();

            for (loaded, range) in ranges {
                debug_assert!(range.min <= range.max, "Negative time-range");

                while delayed_ranges
                    .last()
                    .is_some_and(|(_, r)| r.min <= range.min)
                    && let Some((state, range)) = delayed_ranges.pop()
                {
                    add_range(state, range, &mut delayed_ranges);
                    delayed_ranges.sort_by_key(rev_cmp);
                }
                add_range(*loaded, *range, &mut delayed_ranges);
                delayed_ranges.sort_by_key(rev_cmp);
            }

            while let Some((loaded, range)) = delayed_ranges.pop() {
                add_range(loaded, range, &mut delayed_ranges);
                delayed_ranges.sort_by_key(rev_cmp);
            }

            new_ranges
        }

        let mut scratch = Vec::new();
        let mut ranges = Vec::new();

        for timelines in self.native_temporal_map.values() {
            let Some(data) = timelines.get(timeline) else {
                continue;
            };

            for chunks in data.values() {
                scratch.extend(chunks.iter().filter_map(|(c, range)| {
                    let state = self.remote_chunk_info(c)?.state;
                    let loaded = match state {
                        LoadState::Unloaded | LoadState::InTransit => false,
                        LoadState::Loaded => true,
                    };

                    Some((loaded, range.time_range))
                }));

                ranges.extend(merge_ranges(&mut scratch));

                scratch.clear();
            }
        }

        merge_ranges(&mut ranges)
            .into_iter()
            .filter(|(loaded, _)| *loaded)
            .map(|(_, range)| range)
    }

    pub fn unloaded_time_ranges_for(
        &self,
        timeline: &re_chunk::Timeline,
        entity: &re_chunk::EntityPath,
        component: Option<re_chunk::ComponentIdentifier>,
    ) -> Vec<(AbsoluteTimeRange, u64)> {
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
                .filter(|(chunk, _)| {
                    self.remote_chunks.get(chunk).is_none_or(|c| match c.state {
                        LoadState::InTransit | LoadState::Unloaded => true,
                        LoadState::Loaded => false,
                    })
                })
                .map(|(_, entry)| (entry.time_range, entry.num_rows))
                .collect()
        } else {
            // If we don't have a specific component we want to include the entity's children
            let mut res = Vec::new();

            if let Some(tree) = self.entity_tree.subtree(entity) {
                tree.visit_children_recursively(|child| {
                    self.unloaded_time_ranges_for_entity(&mut res, timeline, child);
                });
            } else {
                re_log::warn_once!("Missing tree for {entity}");
                self.unloaded_time_ranges_for_entity(&mut res, timeline, entity);
            }

            res
        }
    }

    fn unloaded_time_ranges_for_entity(
        &self,
        ranges: &mut Vec<(AbsoluteTimeRange, u64)>,
        timeline: &re_chunk::Timeline,
        entity: &re_chunk::EntityPath,
    ) {
        re_tracing::profile_function!();

        if let Some(entity_ranges_per_timeline) = self.native_temporal_map.get(entity)
            && let Some(entity_ranges) = entity_ranges_per_timeline.get(timeline)
        {
            for (_, entry) in entity_ranges.values().flatten().filter(|(chunk, _)| {
                self.remote_chunks.get(chunk).is_none_or(|c| match c.state {
                    LoadState::InTransit | LoadState::Unloaded => true,
                    LoadState::Loaded => false,
                })
            }) {
                ranges.push((entry.time_range, entry.num_rows));
            }
        }
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
