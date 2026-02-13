use std::{collections::BTreeMap, sync::Arc};

use ahash::HashMap;
use arrow::array::RecordBatch;
use itertools::izip;
use nohash_hasher::IntSet;
use re_byte_size::{MemUsageTree, MemUsageTreeCapture};
use re_chunk::{ChunkId, EntityPath, Timeline, TimelineName};
use re_chunk_store::{ChunkStore, ChunkStoreDiff, ChunkStoreEvent};
use re_log_encoding::RrdManifest;
use re_log_types::{AbsoluteTimeRange, StoreKind, TimelinePoint};
use re_mutex::Mutex;

use crate::chunk_requests::ChunkBatchRequest;
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
#[derive(Clone, Debug)]
pub struct RootChunkInfo {
    pub entity_path: EntityPath,

    /// What row in the source RRD manifest is this chunk in?
    pub row_id: usize,

    state: LoadState,

    /// Empty for static chunks
    pub temporals: HashMap<TimelineName, TemporalChunkInfo>,
}

impl RootChunkInfo {
    fn new(entity_path: EntityPath, row_idx: usize) -> Self {
        Self {
            entity_path,
            state: LoadState::Unloaded,
            row_id: row_idx,
            temporals: Default::default(),
        }
    }
}

impl re_byte_size::SizeBytes for RootChunkInfo {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            entity_path,
            state: _,
            row_id: _,
            temporals,
        } = self;
        entity_path.heap_size_bytes() + temporals.heap_size_bytes()
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

/// A cache used to calculate which ranges are loaded from a latest at perspective.
#[derive(Clone)]
struct LoadedRanges {
    ranges: time_range_merger::MergedRanges,

    /// The timeline this is cached for.
    timeline: TimelineName,
}

impl re_byte_size::SizeBytes for LoadedRanges {
    fn heap_size_bytes(&self) -> u64 {
        let Self { ranges, timeline } = self;

        ranges.heap_size_bytes() + timeline.heap_size_bytes()
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
    /// The chunk store may split and/or merge root chunks, producing _derived_ chunks.
    root_chunks: HashMap<ChunkId, RootChunkInfo>,

    chunk_prioritizer: ChunkPrioritizer,

    /// Keeps track of temporal chunks that are related to a specific entity.
    ///
    /// Used for displaying chunks as unloaded in the density graph in the time panel.
    sorted_chunks: SortedTemporalChunks,

    /// Keeps track of what chunks need to be loaded for a time range to be displayed
    /// as loaded.
    ///
    /// Used for displaying the top loaded indicator in the time panel.
    loaded_ranges: Option<LoadedRanges>,

    /// Full time range per timeline
    timelines: BTreeMap<TimelineName, AbsoluteTimeRange>,

    pub entity_tree: crate::EntityTree,
    entity_has_static_data: IntSet<re_chunk::EntityPath>,

    full_uncompressed_size: u64,
}

impl std::fmt::Debug for RrdManifestIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RrdManifestIndex").finish_non_exhaustive()
    }
}

impl RrdManifestIndex {
    pub fn append(&mut self, manifest: Arc<RrdManifest>) {
        re_tracing::profile_function!();

        if self.manifest.is_some() {
            re_log::warn!(
                "Received a second RRD manifest schema for the same recording. This is not yet supported."
            );
        }

        self.full_uncompressed_size = manifest.col_chunk_byte_size_uncompressed().iter().sum();

        self.update_timeline_stats(&manifest);
        self.update_entity_tree(&manifest);
        self.update_entity_static_data(&manifest);
        self.chunk_prioritizer.on_rrd_manifest(&manifest);

        self.sorted_chunks
            .update(&self.entity_tree, manifest.temporal_map());

        for (row_idx, (&root_chunk_id, entity_path)) in
            izip!(manifest.col_chunk_ids(), manifest.col_chunk_entity_path()).enumerate()
        {
            self.root_chunks
                .insert(root_chunk_id, RootChunkInfo::new(entity_path, row_idx));
        }

        for timelines in manifest.temporal_map().values() {
            for (&timeline, comps) in timelines {
                for chunks in comps.values() {
                    for (&chunk_id, entry) in chunks {
                        let chunk_info = self
                            .root_chunks
                            .get_mut(&chunk_id)
                            .expect("Bug in RRD manifest");
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

        self.manifest = Some(manifest);
    }

    /// Iterate over all chunks in the manifest.
    pub fn root_chunks(&self) -> impl Iterator<Item = &RootChunkInfo> {
        self.root_chunks.values()
    }

    /// Info about a chunk that is in the manifest
    pub fn root_chunk_info(&self, chunk_id: &ChunkId) -> Option<&RootChunkInfo> {
        self.root_chunks.get(chunk_id)
    }

    fn update_timeline_stats(&mut self, manifest: &RrdManifest) {
        for timelines in manifest.temporal_map().values() {
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

    fn update_entity_tree(&mut self, manifest: &RrdManifest) {
        for entity in manifest
            .static_map()
            .keys()
            .chain(manifest.temporal_map().keys())
        {
            self.entity_tree.on_new_entity(entity);
        }
    }

    fn update_entity_static_data(&mut self, manifest: &RrdManifest) {
        for entity in manifest.static_map().keys() {
            self.entity_has_static_data.insert(entity.clone());
        }
    }

    fn update_loaded_ranges(&mut self, current_timeline: TimelineName) {
        re_tracing::profile_function!();

        let is_unloaded = |id| {
            self.root_chunks
                .get(&id)
                .is_none_or(|c| c.state.is_unloaded())
        };

        // Skip fully updating if timeline didn't change.
        if let Some(loaded_ranges) = &mut self.loaded_ranges
            && loaded_ranges.timeline == current_timeline
        {
            loaded_ranges.ranges.update_components_of_interest(
                &self.chunk_prioritizer.components_of_interest,
                &self.chunk_prioritizer.component_paths_from_root_id,
                is_unloaded,
            );
            return;
        }

        let Some(timeline_range) = self.timeline_range(&current_timeline) else {
            return;
        };

        let mut ranges = Vec::new();

        // First we merge ranges for individual components, since chunks' time ranges
        // often have gaps which we don't want to display other components' chunks
        // loaded state in.
        for chunks in self
            .sorted_chunks
            .iter_all_component_chunks_on_timeline(current_timeline)
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

        self.loaded_ranges = Some(LoadedRanges {
            ranges: time_range_merger::MergedRanges::new(
                time_range_merger::merge_ranges(ranges.drain(..)),
                &self.chunk_prioritizer.components_of_interest,
                &self.chunk_prioritizer.component_paths_from_root_id,
                is_unloaded,
            ),
            timeline: current_timeline,
        });
    }

    pub fn entity_has_temporal_data_on_timeline(
        &self,
        entity: &re_chunk::EntityPath,
        timeline: &TimelineName,
    ) -> bool {
        self.sorted_chunks
            .get(timeline, &entity.hash())
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

    pub fn mark_as_loaded(&mut self, chunk_id: ChunkId) {
        if let Some(root_info) = self.root_chunks.get_mut(&chunk_id) {
            root_info.state = LoadState::Loaded;
        }
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
            if let Some(loaded_ranges) = loaded_ranges {
                match state {
                    LoadState::Unloaded => {
                        loaded_ranges.ranges.on_chunk_unloaded(
                            &chunk_id,
                            &self.chunk_prioritizer.component_paths_from_root_id,
                        );
                    }
                    LoadState::InTransit => {}
                    LoadState::Loaded => {
                        loaded_ranges.ranges.on_chunk_loaded(
                            &chunk_id,
                            &self.chunk_prioritizer.component_paths_from_root_id,
                        );
                    }
                }
            }
        };

        if let Some(chunk_info) = self.root_chunks.get_mut(chunk_id) {
            chunk_info.state = state;
            update_ranges(*chunk_id);
        } else {
            let root_chunk_ids = store.find_root_chunks(chunk_id);
            if root_chunk_ids.is_empty() {
                warn_when_editing_recording(
                    store_kind,
                    "Added chunk that was not part of the chunk index",
                );
            } else {
                for chunk_id in root_chunk_ids {
                    if let Some(chunk_info) = self.root_chunks.get_mut(&chunk_id) {
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

    /// Cancel all fetches of things that are not currently needed.
    pub fn cancel_outdated_requests(&mut self, egui_now_time: f64) {
        if self.has_manifest() {
            let cancelred_chunks = self
                .chunk_prioritizer
                .cancel_outdated_requests(egui_now_time);
            for chunk_id in cancelred_chunks {
                if let Some(chunk_info) = self.root_chunks.get_mut(&chunk_id) {
                    chunk_info.state = LoadState::Unloaded;
                } else {
                    re_log::warn!(
                        "Canceled chunk fetch that was not part of the chunk index. This is unexpected and may indicate a bug in the RRD manifest or chunk store."
                    );
                }
            }
        }
    }

    /// Find the next candidates for prefetching.
    ///
    /// This will also clear the tracked missing/used chunks ids in the store.
    pub fn prefetch_chunks(
        &mut self,
        store: &ChunkStore,
        options: &ChunkPrefetchOptions,
        time_cursor: TimelinePoint,
        load_chunks: &dyn Fn(RecordBatch) -> ChunkPromise,
    ) -> Result<(), PrefetchError> {
        re_tracing::profile_function!();

        let used_and_missing = store.take_tracked_chunk_ids(); // Note: this mutates the store (kind of).

        if let Some(manifest) = &self.manifest {
            let to_load = self.chunk_prioritizer.prioritize_and_prefetch(
                store,
                used_and_missing,
                options,
                time_cursor,
                manifest,
                &self.root_chunks,
            )?;

            // Start loading all batches we prepared:
            for (rb, batch_info) in to_load {
                for root_chunk_id in &batch_info.root_chunk_ids {
                    if let Some(root_chunk) = self.root_chunks.get_mut(root_chunk_id) {
                        root_chunk.state = LoadState::InTransit;
                    }
                }

                let promise = load_chunks(rb);
                let batch = ChunkBatchRequest {
                    promise: Mutex::new(Some(promise)),
                    info: batch_info.into(),
                };
                self.chunk_prioritizer.chunk_requests_mut().add(batch);
            }

            self.update_loaded_ranges(time_cursor.name);

            Ok(())
        } else {
            Err(PrefetchError::NoManifest)
        }
    }

    /// Creates an iterator of time ranges which are loaded on a specific timeline.
    ///
    /// The ranges are guaranteed to be ordered and non-overlapping.
    pub fn loaded_ranges_on_timeline(&self, timeline: &TimelineName) -> Vec<AbsoluteTimeRange> {
        re_tracing::profile_function!();

        let Some(loaded_ranges) = self
            .loaded_ranges
            .as_ref()
            .filter(|l| l.timeline == *timeline)
        else {
            return Vec::new();
        };

        loaded_ranges.ranges.loaded_ranges()
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

        let Some(entry) = self.sorted_chunks.get(timeline, &entity.hash()) else {
            return iterate_unloaded(self, &[]);
        };

        if let Some(component) = component {
            iterate_unloaded(self, entry.component_chunks(&component))
        } else {
            iterate_unloaded(self, entry.per_entity())
        }
    }

    fn is_chunk_unloaded(&self, chunk_id: &ChunkId) -> bool {
        self.root_chunks
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
            re_log::debug_warn_once!("{warning}");
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
            root_chunks: virtual_chunks,
            chunk_prioritizer,
            timelines,
            full_uncompressed_size: _,
        } = self;

        entity_has_static_data.heap_size_bytes()
            + entity_tree.heap_size_bytes()
            + manifest.heap_size_bytes()
            + sorted_chunks.heap_size_bytes()
            + loaded_ranges.heap_size_bytes()
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
            root_chunks: virtual_chunks,
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
        node.add("virtual_chunks", virtual_chunks.total_size_bytes());
        node.add("timelines", timelines.total_size_bytes());

        node.into_tree()
    }
}
