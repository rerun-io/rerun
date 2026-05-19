use std::{collections::BTreeMap, sync::Arc};

use ahash::HashMap;
use arrow::array::RecordBatch;
use itertools::izip;
use nohash_hasher::IntSet;
use re_byte_size::{MemUsageTree, MemUsageTreeCapture};
use re_chunk::{ChunkId, EntityPath, Timeline, TimelineName};
use re_chunk_store::{ChunkStore, ChunkStoreDiff, ChunkStoreEvent};
use re_log_encoding::{CodecResult, RrdManifest};
use re_log_types::{AbsoluteTimeRange, StoreKind};

pub use crate::chunk_requests::{ChunkPromise, ChunkRequests, RequestInfo};

mod chunk_prioritizer;
mod collapsed_time_ranges;
mod sorted_temporal_chunks;
mod time_range_merger;

pub use chunk_prioritizer::{
    ChunkFetcher, ChunkPrefetchOptions, ChunkPrioritizer, FetchStage, PrefetchError,
    PrefetchTimeCursor, PrioritizationState, ProtectedChunks, RemainingByteBudget,
};
pub use sorted_temporal_chunks::ChunkCountInfo;

use sorted_temporal_chunks::SortedTemporalChunks;

/// Is the following chunk loaded?
///
/// The order here is used for priority to show the state in the ui (lower is more prioritized)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum LoadState {
    /// The chunk is not fully loaded, nor being loaded.
    ///
    /// It may be that we've previously loaded this chunk,
    /// split it on ingestion, removed part of it.
    /// In that case, we consider the chunk [`Self::Unloaded`].
    #[default]
    Unloaded,

    /// We have requested it.
    ///
    /// TODO(emilk): move this state to [`ChunkRequests`]
    InTransit,

    /// We have the whole chunk in memory.
    FullyLoaded,
}

impl LoadState {
    pub fn is_fully_loaded(self) -> bool {
        self == Self::FullyLoaded
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

    pub fn is_fully_loaded(&self) -> bool {
        self.state.is_fully_loaded()
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
/// The manifest may be received in parts and concatenated together.
#[derive(Default)]
#[cfg_attr(feature = "testing", derive(Clone))]
pub struct RrdManifestIndex {
    /// The raw manifest (accumulated from possibly multiple parts).
    ///
    /// This is known ahead-of-time for _some_ data sources.
    manifest: Option<Arc<RrdManifest>>,

    /// True once all parts of the manifest have been received.
    manifest_complete: bool,

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

    /// Cached data time ranges per timeline, used for gap collapsing in the time panel.
    /// Computed from chunk time ranges when the manifest is complete.
    data_time_ranges: BTreeMap<TimelineName, Vec<AbsoluteTimeRange>>,

    entity_has_static_data: IntSet<re_chunk::EntityPath>,

    full_uncompressed_size: u64,
}

impl std::fmt::Debug for RrdManifestIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RrdManifestIndex").finish_non_exhaustive()
    }
}

impl RrdManifestIndex {
    pub fn append(
        &mut self,
        delta: Arc<RrdManifest>,
        entity_tree: &re_chunk_store::EntityTree,
    ) -> CodecResult<()> {
        re_tracing::profile_function!();

        self.update_timeline_stats(&delta);
        self.update_entity_static_data(&delta);
        self.chunk_prioritizer.on_rrd_manifest(&delta);

        self.full_uncompressed_size += delta.col_chunk_byte_size_uncompressed().iter().sum::<u64>();

        self.loaded_ranges = None; // invalidate and recompute

        let row_offset = self
            .manifest
            .as_ref()
            .map_or(0, |manifest| manifest.chunk_fetcher_rb().num_rows());

        for (delta_row_idx, (&root_chunk_id, entity_path)) in
            izip!(delta.col_chunk_ids(), delta.col_chunk_entity_path()).enumerate()
        {
            self.root_chunks.insert(
                root_chunk_id,
                RootChunkInfo::new(entity_path, row_offset + delta_row_idx),
            );
        }

        for timelines in delta.temporal_map().values() {
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

        let new_full_manifest = if let Some(existing) = self.manifest.take() {
            Arc::new(RrdManifest::concat(&[&existing, &delta])?)
        } else {
            delta
        };

        self.sorted_chunks =
            SortedTemporalChunks::new(entity_tree, new_full_manifest.temporal_map());

        self.manifest = Some(new_full_manifest);

        Ok(())
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
                .is_none_or(|c| !c.state.is_fully_loaded())
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

    /// Returns true if an entity has any temporal data on the given timeline.
    ///
    /// This ignores static data.
    pub fn entity_has_temporal_data_on_timeline(
        &self,
        entity: &re_chunk::EntityPath,
        timeline: &TimelineName,
    ) -> bool {
        self.sorted_chunks
            .get(timeline, &entity.hash())
            .is_some_and(|e| e.has_data())
    }

    /// Returns true if an entity has data for the given component on the given timeline at any point in time.
    ///
    /// This ignores static data.
    /// This is a more fine grained version of [`Self::entity_has_temporal_data_on_timeline`].
    pub fn entity_has_temporal_data_on_timeline_for_component(
        &self,
        entity: &re_chunk::EntityPath,
        timeline: &TimelineName,
        component: re_chunk::ComponentIdentifier,
    ) -> bool {
        self.sorted_chunks
            .get(timeline, &entity.hash())
            .is_some_and(|e| !e.component_chunks(&component).is_empty())
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
    ///
    /// This is true as soon as the first piece of the manifest is available.
    pub fn has_manifest(&self) -> bool {
        self.manifest.is_some()
    }

    /// Have all parts of the manifest been received?
    pub fn is_manifest_complete(&self) -> bool {
        self.manifest_complete
    }

    /// Mark the manifest as complete (all parts have been received).
    pub fn set_manifest_complete(&mut self) {
        self.manifest_complete = true;

        let num_root_chunks = self.root_chunks.len();
        if 25_000 < num_root_chunks {
            re_log::debug_warn!(
                "There are {} root chunks in this recording. Consider running `rerun rrd optimize` on the original.",
                re_format::format_uint(num_root_chunks)
            );
        }

        self.data_time_ranges = collapsed_time_ranges::compute_data_time_ranges(&self.root_chunks);
    }

    /// The manifest as it currently stands.
    ///
    /// More pieces of it may still arrive unless [`Self::is_manifest_complete`] is true.
    pub fn manifest(&self) -> Option<&RrdManifest> {
        self.manifest.as_deref()
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
                    self.mark_roots_as(
                        store,
                        &add.chunk_before_processing.id(),
                        LoadState::FullyLoaded,
                    );
                }

                ChunkStoreDiff::Deletion(del) => {
                    // If we deleted part of a root, we mark the whole root as unloaded.
                    self.mark_roots_as(store, &del.chunk.id(), LoadState::Unloaded);
                }

                ChunkStoreDiff::VirtualAddition(_) | ChunkStoreDiff::SchemaAddition(_) => {}
            }
        }
    }

    fn mark_roots_as(&mut self, store: &ChunkStore, chunk_id: &ChunkId, new_state: LoadState) {
        re_tracing::profile_function!();

        let store_kind = store.id().kind();

        let root_chunk_ids = store.find_root_chunks(chunk_id);

        if root_chunk_ids.is_empty() {
            warn_when_editing_recording(
                store_kind,
                "Added chunk that was not part of the chunk index",
            );
        } else {
            // Track which timelines had a large chunk transition to loaded
            let mut timelines_to_recalculate: Vec<TimelineName> = Vec::new();

            for chunk_id in root_chunk_ids {
                if let Some(chunk_info) = self.root_chunks.get_mut(&chunk_id) {
                    let old_state = chunk_info.state;
                    chunk_info.state = new_state;

                    // Only update loaded ranges on actual state transitions to avoid
                    // mismatched increments/decrements.
                    if let Some(loaded_ranges) = &mut self.loaded_ranges
                        && old_state != new_state
                    {
                        match new_state {
                            LoadState::Unloaded => {
                                loaded_ranges.ranges.on_chunk_unloaded(
                                    &chunk_id,
                                    &self.chunk_prioritizer.component_paths_from_root_id,
                                );
                            }
                            LoadState::InTransit => {}
                            LoadState::FullyLoaded => {
                                loaded_ranges.ranges.on_chunk_loaded(
                                    &chunk_id,
                                    &self.chunk_prioritizer.component_paths_from_root_id,
                                );
                            }
                        }
                    }

                    // When a large chunk gets loaded, recalculate data ranges for its timelines
                    if new_state == LoadState::FullyLoaded && old_state != LoadState::FullyLoaded {
                        timelines_to_recalculate.extend(
                            collapsed_time_ranges::should_recalculate_for_chunk(
                                chunk_info,
                                &self.timelines,
                            ),
                        );
                    }
                } else {
                    warn_when_editing_recording(
                        store_kind,
                        "Added chunk that was not part of the chunk index",
                    );
                }
            }

            for timeline_name in timelines_to_recalculate {
                if let Some(ranges) = collapsed_time_ranges::calculate_data_ranges_for_timeline(
                    &self.root_chunks,
                    &self.timelines,
                    store,
                    &timeline_name,
                ) {
                    self.data_time_ranges.insert(timeline_name, ranges);
                }
            }
        }
    }

    /// Data time ranges for the given timeline, used for gap collapsing in the time panel.
    ///
    /// Returns `None` if the manifest is not yet complete or if there are no ranges.
    pub fn data_time_ranges_for(&self, timeline: &TimelineName) -> Option<&[AbsoluteTimeRange]> {
        self.data_time_ranges.get(timeline).map(|v| v.as_slice())
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
        time_cursor: Option<PrefetchTimeCursor>,
        budget: &mut RemainingByteBudget,
        load_chunks: &dyn Fn(RecordBatch) -> ChunkPromise,
    ) -> Result<(), PrefetchError> {
        re_tracing::profile_function!();

        let Some(mut fetcher) = self.prepare_chunk_fetcher(store, options, time_cursor, budget)
        else {
            return Ok(());
        };

        fetcher.fetch(budget, options.max_fetch_stage)?;

        let res = fetcher.finish(load_chunks)?;

        self.handle_fetch_result(res);

        Ok(())
    }

    pub fn handle_fetch_result(&mut self, res: chunk_prioritizer::ChunkFetchResult) {
        for chunk_id in res.new_in_transit_chunks {
            if let Some(chunk) = self.root_chunks.get_mut(&chunk_id) {
                chunk.state = LoadState::InTransit;
            }
        }

        if let Some(time_cursor) = res.time_cursor {
            self.update_loaded_ranges(*time_cursor.timeline().name());
        }
    }

    /// Handle initial chunk prioritization and build a [`ChunkFetcher`].
    ///
    /// This should be called once per frame per recording, because it
    /// clears tracked missing & used chunks from the chunk store, so that can be populated again next frame.
    ///
    /// Subtracts already loaded physical chunks from the memory budget.
    ///
    /// Then call [`ChunkFetcher::fetch`] to actually fetch chunks,
    /// and [`ChunkFetcher::finish`] when done.
    pub fn prepare_chunk_fetcher<'a>(
        &'a mut self,
        store: &'a ChunkStore,
        options: &ChunkPrefetchOptions,
        time_cursor: Option<PrefetchTimeCursor>,
        budget: &mut RemainingByteBudget,
    ) -> Option<ChunkFetcher<'a>> {
        let manifest = self.manifest.as_ref()?;
        Some(self.chunk_prioritizer.prepare_chunk_fetcher(
            store,
            manifest,
            options,
            time_cursor.map(|mut time_cursor| {
                if let Some(loop_range) = time_cursor.loop_range
                    && let Some(timeline_range) = self.timeline_range(time_cursor.name())
                {
                    time_cursor.loop_range = loop_range.intersection(timeline_range);
                }

                time_cursor
            }),
            &self.root_chunks,
            budget,
        ))
    }

    /// True if there are any protected chunks (chunks we're keeping in memory).
    ///
    /// Recordings with protected chunks should not be auto-closed.
    pub fn has_protected_chunks(&self) -> bool {
        !self.chunk_prioritizer.protected_chunks().roots.is_empty()
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

        self.temporal_entries_for(timeline, entity, component)
            .iter()
            .filter(|info| self.is_chunk_unloaded(&info.id))
    }

    /// If `component` is some, this returns all temporal entries for that specific
    /// component on the given timeline.
    ///
    /// If not, this returns all temporal entries for `entity`'s components and its
    /// descendants' unloaded temporal entries.
    pub fn temporal_entries_for(
        &self,
        timeline: &re_chunk::TimelineName,
        entity: &re_chunk::EntityPath,
        component: Option<re_chunk::ComponentIdentifier>,
    ) -> &[ChunkCountInfo] {
        re_tracing::profile_function!();

        let Some(entry) = self.sorted_chunks.get(timeline, &entity.hash()) else {
            return &[];
        };

        if let Some(component) = component {
            entry.component_chunks(&component)
        } else {
            entry.per_entity()
        }
    }

    fn is_chunk_unloaded(&self, chunk_id: &ChunkId) -> bool {
        self.root_chunks
            .get(chunk_id)
            .is_none_or(|c| !c.state.is_fully_loaded())
    }

    /// Expected RAM use of all _chunks_.
    pub fn full_uncompressed_size(&self) -> u64 {
        self.full_uncompressed_size
    }

    /// Have we downloaded the entire recording?
    pub fn is_fully_loaded(&self) -> bool {
        self.root_chunks()
            .all(|chunk| chunk.state == LoadState::FullyLoaded)
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
            manifest,
            sorted_chunks,
            loaded_ranges,
            root_chunks: virtual_chunks,
            chunk_prioritizer,
            timelines,
            data_time_ranges,
            full_uncompressed_size: _,
            manifest_complete: _,
        } = self;

        entity_has_static_data.heap_size_bytes()
            + manifest.heap_size_bytes()
            + sorted_chunks.heap_size_bytes()
            + loaded_ranges.heap_size_bytes()
            + virtual_chunks.heap_size_bytes()
            + chunk_prioritizer.heap_size_bytes()
            + timelines.heap_size_bytes()
            + data_time_ranges.heap_size_bytes()
    }
}

impl MemUsageTreeCapture for RrdManifestIndex {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        re_tracing::profile_function!();

        use re_byte_size::SizeBytes as _;

        let Self {
            entity_has_static_data,
            sorted_chunks,
            loaded_ranges,
            manifest,
            root_chunks: virtual_chunks,
            chunk_prioritizer,
            timelines,
            data_time_ranges: _,
            full_uncompressed_size: _,
            manifest_complete: _,
        } = self;

        let mut node = re_byte_size::MemUsageNode::new();
        node.add("chunk_prioritizer", chunk_prioritizer.total_size_bytes());
        node.add(
            "entity_has_static_data",
            entity_has_static_data.total_size_bytes(),
        );
        node.add("sorted_chunks", sorted_chunks.total_size_bytes());
        node.add("loaded_ranges", loaded_ranges.total_size_bytes());
        node.add("manifest", manifest.total_size_bytes());
        node.add("virtual_chunks", virtual_chunks.total_size_bytes());
        node.add("timelines", timelines.total_size_bytes());

        node.into_tree()
    }
}
