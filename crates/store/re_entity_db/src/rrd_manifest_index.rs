use std::{collections::BTreeMap, sync::Arc};

use ahash::HashMap;
use arrow::array::RecordBatch;
use nohash_hasher::IntSet;
use re_byte_size::{MemUsageTree, MemUsageTreeCapture};
use re_chunk::{ChunkId, Timeline, TimelineName};
use re_chunk_store::{ChunkStore, ChunkStoreDiff, ChunkStoreEvent};
use re_log_encoding::{CodecResult, RrdManifest};
use re_log_types::{AbsoluteTimeRange, StoreKind};

pub use crate::chunk_requests::{ChunkPromise, ChunkRequests, RequestInfo};

mod chunk_prioritizer;
mod sorted_temporal_chunks;
mod time_range_merger;

pub use chunk_prioritizer::{ChunkPrefetchOptions, ChunkPrioritizer, PrefetchError};
pub use sorted_temporal_chunks::ChunkCountInfo;

use sorted_temporal_chunks::SortedTemporalChunks;

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
    /// TODO(emilk): move this state to [`ChunkRequests`]
    InTransit,

    /// We have the chole chunk in memory.
    Loaded,
}

impl LoadState {
    pub fn is_unloaded(&self) -> bool {
        match self {
            Self::Unloaded | Self::InTransit => true,
            Self::Loaded => false,
        }
    }
}

/// Info about a single chunk that we know ahead of loading it.
#[derive(Clone, Debug, Default)]
pub struct VirtualChunkInfo {
    state: LoadState,

    /// Empty for static chunks
    pub temporals: HashMap<TimelineName, TemporalChunkInfo>,
}

impl re_byte_size::SizeBytes for VirtualChunkInfo {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            state: _,
            temporals,
        } = self;
        temporals.heap_size_bytes()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TemporalChunkInfo {
    pub timeline: Timeline,

    /// The time range covered by this entry.
    pub time_range: AbsoluteTimeRange,

    /// The total number of events in the original chunk for this time range.
    ///
    /// This accumulates for all entities and all components, but still accounts for sparness.
    pub num_rows_for_all_entities_all_components: u64,
}

impl re_byte_size::SizeBytes for TemporalChunkInfo {
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

/// A secondary index that keeps track of which chunks have been loaded into memory.
///
/// This is constructed from an [`RrdManifest`], which is what the server sends to the client/viewer.
//
// TODO(RR-3383): support multiple manifests per index.
#[derive(Default)]
#[cfg_attr(feature = "testing", derive(Clone))]
pub struct RrdManifestIndex {
    /// The raw manifest.
    ///
    /// This is known ahead-of-time for _some_ data sources.
    manifest: Option<Arc<RrdManifest>>,

    /// These are the chunks known to exist in the data source (e.g. remote server).
    ///
    /// The chunk store may split large chunks and merge (compact) small ones,
    /// so what's in the chunk store can differ significantly.
    virtual_chunks: HashMap<ChunkId, VirtualChunkInfo>,

    chunk_prioritizer: ChunkPrioritizer,

    /// Keeps track of temporal chunks that are related to a specific entity.
    ///
    /// Used for displaying chunks as unloaded in the density graph in the time panel.
    sorted_chunks: SortedTemporalChunks,

    /// Keeps track of what chunks need to be loaded for a time range to be displayed
    /// as loaded.
    ///
    /// Used for displaying the top loaded indicator in the time panel.
    loaded_ranges: BTreeMap<TimelineName, time_range_merger::MergedRanges>,

    /// Full time range per timeline
    timelines: BTreeMap<TimelineName, AbsoluteTimeRange>,

    pub entity_tree: crate::EntityTree,
    entity_has_static_data: IntSet<re_chunk::EntityPath>,

    native_static_map: re_log_encoding::RrdManifestStaticMap,
    native_temporal_map: re_log_encoding::RrdManifestTemporalMap,

    full_uncompressed_size: u64,
}

impl std::fmt::Debug for RrdManifestIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RrdManifestIndex").finish_non_exhaustive()
    }
}

impl RrdManifestIndex {
    pub fn append(&mut self, manifest: Arc<RrdManifest>) -> CodecResult<()> {
        re_tracing::profile_function!();

        self.native_static_map = manifest.get_static_data_as_a_map()?;
        self.native_temporal_map = manifest.get_temporal_data_as_a_map()?;

        self.update_timeline_stats();
        self.update_entity_tree();
        self.update_entity_static_data();
        self.chunk_prioritizer.on_rrd_manifest(
            &manifest,
            &self.native_static_map,
            &self.native_temporal_map,
        );

        self.sorted_chunks
            .update(&self.entity_tree, &self.native_temporal_map);
        self.update_loaded_ranges();

        for &chunk_id in manifest.col_chunk_ids() {
            self.virtual_chunks.insert(chunk_id, Default::default());
        }

        for timelines in self.native_temporal_map.values() {
            for (&timeline, comps) in timelines {
                for chunks in comps.values() {
                    for (&chunk_id, entry) in chunks {
                        let chunk_info = self.virtual_chunks.entry(chunk_id).or_default();
                        chunk_info
                            .temporals
                            .entry(*timeline.name())
                            .and_modify(|info| {
                                info.time_range = entry.time_range.union(info.time_range);
                                info.num_rows_for_all_entities_all_components += entry.num_rows;
                            })
                            .or_insert(TemporalChunkInfo {
                                timeline,
                                time_range: entry.time_range,
                                num_rows_for_all_entities_all_components: entry.num_rows,
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

        self.full_uncompressed_size = manifest.col_chunk_byte_size_uncompressed().iter().sum();

        self.manifest = Some(manifest);

        Ok(())
    }

    /// Iterate over all chunks in the manifest.
    pub fn virtual_chunks(&self) -> impl Iterator<Item = &VirtualChunkInfo> {
        self.virtual_chunks.values()
    }

    /// Info about a chunk that is in the manifest
    pub fn virtual_chunk_info(&self, chunk_id: &ChunkId) -> Option<&VirtualChunkInfo> {
        self.virtual_chunks.get(chunk_id)
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

    fn update_entity_static_data(&mut self) {
        for entity in self.native_static_map.keys() {
            self.entity_has_static_data.insert(entity.clone());
        }
    }

    fn update_loaded_ranges(&mut self) {
        re_tracing::profile_function!();
        let mut ranges = Vec::new();

        // First we merge ranges for individual components, since chunks' time ranges
        // often have gaps which we don't want to display other components' chunks
        // loaded state in.
        for (timeline, timeline_range) in &self.timelines {
            for chunks in self
                .sorted_chunks
                .iter_all_component_chunks_on_timeline(*timeline)
            {
                let mut new_ranges = time_range_merger::merge_ranges(
                    chunks
                        .iter()
                        .map(|info| time_range_merger::TimeRange::new(info.id, info.time_range)),
                );

                // Make sure the last range covers to the end of the timeline.
                if let Some(range) = new_ranges.last_mut() {
                    range.max = timeline_range.max;
                }

                ranges.extend(new_ranges);
            }
            self.loaded_ranges.insert(
                *timeline,
                time_range_merger::MergedRanges::new(time_range_merger::merge_ranges(
                    ranges.drain(..),
                )),
            );
        }
    }

    pub fn entity_has_temporal_data_on_timeline(
        &self,
        entity: &re_chunk::EntityPath,
        timeline: &TimelineName,
    ) -> bool {
        self.sorted_chunks
            .get(timeline, entity)
            .is_some_and(|e| e.has_data())
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
        self.manifest.as_deref()
    }

    pub fn native_temporal_map(&self) -> &re_log_encoding::RrdManifestTemporalMap {
        &self.native_temporal_map
    }

    pub fn mark_as_loaded(&mut self, chunk_id: ChunkId) {
        let chunk_info = self.virtual_chunks.entry(chunk_id).or_default();
        chunk_info.state = LoadState::Loaded;
    }

    pub fn on_events(&mut self, store: &ChunkStore, store_events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        if self.manifest.is_none() {
            return;
        }

        for event in store_events {
            match &event.diff {
                ChunkStoreDiff::Addition(add) => {
                    // This is about marking root-level persistent chunks as either loaded or unloaded.
                    //
                    // It doesn't really matter which chunk we pick because `mark_as` will take care of
                    // walking upwards through the lineage tree until one can be found in any case.
                    //
                    // Picking the unprocessed chunk gives us a better chance of not even needing to walk
                    // back the tree at all.
                    self.mark_as(store, &add.chunk_before_processing.id(), LoadState::Loaded);
                }

                ChunkStoreDiff::Deletion(del) => {
                    self.mark_as(store, &del.chunk.id(), LoadState::Unloaded);
                }

                ChunkStoreDiff::VirtualAddition(_) => {}
            }
        }
    }

    fn mark_as(&mut self, store: &ChunkStore, chunk_id: &ChunkId, state: LoadState) {
        let store_kind = store.id().kind();

        let loaded_ranges = &mut self.loaded_ranges;
        let mut update_ranges = |chunk_id| {
            for ranges in loaded_ranges.values_mut() {
                match state {
                    LoadState::Unloaded => {
                        ranges.on_chunk_unloaded(&chunk_id);
                    }
                    LoadState::InTransit => {}
                    LoadState::Loaded => {
                        ranges.on_chunk_loaded(&chunk_id);
                    }
                }
            }
        };

        if let Some(chunk_info) = self.virtual_chunks.get_mut(chunk_id) {
            chunk_info.state = state;
            update_ranges(*chunk_id);
        } else {
            let root_chunk_ids = store.find_root_rrd_manifests(chunk_id);
            if root_chunk_ids.is_empty() {
                warn_when_editing_recording(
                    store_kind,
                    "Added chunk that was not part of the chunk index",
                );
            } else {
                for (chunk_id, _) in root_chunk_ids {
                    if let Some(chunk_info) = self.virtual_chunks.get_mut(&chunk_id) {
                        update_ranges(chunk_id);
                        chunk_info.state = state;
                    } else {
                        warn_when_editing_recording(
                            store_kind,
                            "Added chunk that was not part of the chunk index",
                        );
                    }
                }
            }
        }
    }

    /// When do we have data on this timeline?
    pub fn timeline_range(&self, timeline: &TimelineName) -> Option<AbsoluteTimeRange> {
        self.timelines.get(timeline).copied()
    }

    pub fn chunk_prioritizer(&self) -> &ChunkPrioritizer {
        &self.chunk_prioritizer
    }

    pub fn chunk_requests(&self) -> &ChunkRequests {
        self.chunk_prioritizer.chunk_requests()
    }

    pub fn chunk_requests_mut(&mut self) -> &mut ChunkRequests {
        self.chunk_prioritizer.chunk_requests_mut()
    }

    /// Find the next candidates for prefetching.
    ///
    /// This will also clear the tracked missing/used chunks ids in the store.
    pub fn prefetch_chunks(
        &mut self,
        store: &ChunkStore,
        options: &ChunkPrefetchOptions,
        load_chunks: &dyn Fn(RecordBatch) -> ChunkPromise,
    ) -> Result<(), PrefetchError> {
        re_tracing::profile_function!();

        let used_and_missing = store.take_tracked_chunk_ids(); // Note: this mutates the store (kind of).

        if let Some(manifest) = &self.manifest {
            self.chunk_prioritizer.prioritize_and_prefetch(
                store,
                used_and_missing,
                options,
                load_chunks,
                manifest,
                &mut self.virtual_chunks,
            )
        } else {
            Err(PrefetchError::NoManifest)
        }
    }

    /// Creates an iterator of time ranges which are loaded on a specific timeline.
    ///
    /// The ranges are guaranteed to be ordered and non-overlapping.
    pub fn loaded_ranges_on_timeline(&self, timeline: &TimelineName) -> Vec<AbsoluteTimeRange> {
        re_tracing::profile_function!();

        let Some(ranges) = self.loaded_ranges.get(timeline) else {
            return Vec::new();
        };

        ranges.loaded_ranges()
    }

    /// If `component` is some, this returns all unloaded temporal entries for that specific
    /// component on the given timeline.
    ///
    /// If not, this returns all unloaded temporal entries for `entity`'s components and its
    /// descendants' unloaded temporal entries.
    pub fn unloaded_temporal_entries_for(
        &self,
        timeline: &re_chunk::TimelineName,
        entity: &re_chunk::EntityPath,
        component: Option<re_chunk::ComponentIdentifier>,
    ) -> impl Iterator<Item = &ChunkCountInfo> {
        re_tracing::profile_function!();

        fn iterate_unloaded<'a>(
            index: &RrdManifestIndex,
            chunks: &'a [ChunkCountInfo],
        ) -> impl Iterator<Item = &'a ChunkCountInfo> {
            chunks
                .iter()
                .filter(|info| index.is_chunk_unloaded(&info.id))
        }

        let Some(entry) = self.sorted_chunks.get(timeline, entity) else {
            return iterate_unloaded(self, &[]);
        };

        if let Some(component) = component {
            iterate_unloaded(self, entry.component_chunks(&component))
        } else {
            iterate_unloaded(self, entry.per_entity())
        }
    }

    fn is_chunk_unloaded(&self, chunk_id: &ChunkId) -> bool {
        self.virtual_chunks
            .get(chunk_id)
            .is_none_or(|c| c.state.is_unloaded())
    }

    pub fn full_uncompressed_size(&self) -> u64 {
        self.full_uncompressed_size
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

impl re_byte_size::SizeBytes for RrdManifestIndex {
    fn heap_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();

        let Self {
            entity_has_static_data,
            entity_tree,
            manifest,
            sorted_chunks,
            loaded_ranges,
            native_static_map,
            native_temporal_map,
            virtual_chunks,
            chunk_prioritizer,
            timelines,
            full_uncompressed_size: _,
        } = self;

        entity_has_static_data.heap_size_bytes()
            + entity_tree.heap_size_bytes()
            + manifest.heap_size_bytes()
            + sorted_chunks.heap_size_bytes()
            + loaded_ranges.heap_size_bytes()
            + native_static_map.heap_size_bytes()
            + native_temporal_map.heap_size_bytes()
            + virtual_chunks.heap_size_bytes()
            + chunk_prioritizer.heap_size_bytes()
            + timelines.heap_size_bytes()
    }
}

impl MemUsageTreeCapture for RrdManifestIndex {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        re_tracing::profile_function!();

        use re_byte_size::SizeBytes as _;

        let Self {
            entity_has_static_data,
            entity_tree,
            sorted_chunks,
            loaded_ranges,
            manifest,
            native_static_map,
            native_temporal_map,
            virtual_chunks,
            chunk_prioritizer,
            timelines,
            full_uncompressed_size: _,
        } = self;

        let mut node = re_byte_size::MemUsageNode::new();
        node.add("chunk_prioritizer", chunk_prioritizer.total_size_bytes());
        node.add(
            "entity_has_static_data",
            entity_has_static_data.total_size_bytes(),
        );
        node.add("entity_tree", entity_tree.total_size_bytes());
        node.add("sorted_chunks", sorted_chunks.total_size_bytes());
        node.add("loaded_ranges", loaded_ranges.total_size_bytes());
        node.add("manifest", manifest.total_size_bytes());
        node.add("native_static_map", native_static_map.total_size_bytes());
        node.add(
            "native_temporal_map",
            native_temporal_map.total_size_bytes(),
        );
        node.add("virtual_chunks", virtual_chunks.total_size_bytes());
        node.add("timelines", timelines.total_size_bytes());

        node.into_tree()
    }
}
