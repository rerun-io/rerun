use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;

use ahash::{HashMap, HashSet};
use arrow::array::RecordBatch;
use re_byte_size::SizeBytes as _;
use re_chunk::{Chunk, ChunkId, ComponentIdentifier, TimeInt, Timeline, TimelineName};
use re_chunk_store::{ChunkStore, QueriedChunkIdTracker};
use re_log::debug_assert;
use re_log_encoding::RrdManifest;
use re_log_types::{AbsoluteTimeRange, EntityPathHash, TimelinePoint};
use re_mutex::Mutex;

use crate::{
    chunk_requests::{ChunkRequests, RequestInfo},
    rrd_manifest_index::{LoadState, RootChunkInfo},
    sorted_range_map::{OverlapCursor, SortedRangeMap},
};

#[derive(Clone, Copy, Default)]
pub struct PrioritizationState {
    /// We're not allowed to have more things in-transit (on-wire)
    /// right now.
    pub transit_budget_filled: bool,

    /// We cannot fit the whole recording into memory.
    pub memory_budget_filled: bool,

    /// Some individual chunks exceed the total memory budget.
    pub some_chunks_too_big: bool,

    /// Are all required chunks fully loaded?
    ///
    /// `None` means we haven't run a fetch yet, so we don't know.
    /// `Some(true)` means no required chunk was found to be missing.
    /// `Some(false)` means at least one required chunk is missing or in transit.
    pub all_required_are_loaded: Option<bool>,
}

impl PrioritizationState {
    /// The whole recording fits in memory,
    /// and the full download of it has started.
    pub fn all_chunks_loaded_or_in_transit(&self) -> bool {
        !self.transit_budget_filled && !self.memory_budget_filled && !self.some_chunks_too_big
    }
}

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
}

/// How to calculate which chunks to prefetch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkPrefetchOptions {
    /// Only prefetch chunks up to (and including) this stage.
    ///
    /// Useful for debugging and for users who want to limit how aggressively
    /// we prefetch data ahead of what is strictly needed.
    pub max_fetch_stage: FetchStage,

    /// Batch together requests until we reach this size.
    pub max_on_wire_bytes_per_batch: u64,

    /// Maximum number of bytes in transit at once.
    pub max_bytes_on_wire_at_once: u64,
}

impl Default for ChunkPrefetchOptions {
    fn default() -> Self {
        Self {
            max_fetch_stage: FetchStage::default(),

            // Batch small chunks together.
            max_on_wire_bytes_per_batch: 256 * 1024,

            // A high value -> better theoretical bandwidth
            // Low value -> better responsiveness (e.g. when moving time cursor).
            // In practice, this is a limit on how many bytes we can download _every frame_.
            max_bytes_on_wire_at_once: 4_000_000,
        }
    }
}

/// Special chunks for which we need the entire history, not just the latest-at value.
///
/// Right now, this is only used for transform-related chunks.
#[derive(Clone, Default)]
struct HighPrioChunks {
    /// Sorted by time range min.
    temporal_chunks: BTreeMap<TimelineName, Vec<HighPrioChunk>>,
}

impl re_byte_size::SizeBytes for HighPrioChunks {
    fn heap_size_bytes(&self) -> u64 {
        let Self { temporal_chunks } = self;
        temporal_chunks.heap_size_bytes()
    }
}

#[derive(Clone)]
struct HighPrioChunk {
    chunk_id: ChunkId,
    time_range: AbsoluteTimeRange,
}

impl re_byte_size::SizeBytes for HighPrioChunk {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            chunk_id: _,
            time_range: _,
        } = self;
        0
    }

    fn is_pod() -> bool {
        true
    }
}

#[derive(Default)]
struct CurrentBatch {
    row_indices: Vec<usize>,
    uncompressed_bytes: u64,
    on_wire_bytes: u64,
}

impl CurrentBatch {
    fn reset(&mut self) {
        let Self {
            row_indices,
            uncompressed_bytes,
            on_wire_bytes,
        } = self;
        row_indices.clear();
        *uncompressed_bytes = 0;
        *on_wire_bytes = 0;
    }
}

/// Helper struct responsible for batching requests and creating
/// promises for missing chunks.
pub(crate) struct ChunkRequestBatcher<'a> {
    manifest: &'a RrdManifest,
    chunk_byte_size_uncompressed: &'a [u64],
    chunk_byte_size: &'a [u64],
    max_on_wire_bytes_per_batch: u64,

    current_batch: CurrentBatch,

    // Output
    to_load: Vec<(RecordBatch, RequestInfo)>,
}

impl<'a> ChunkRequestBatcher<'a> {
    pub(crate) fn new(manifest: &'a RrdManifest, options: &ChunkPrefetchOptions) -> Self {
        Self {
            chunk_byte_size_uncompressed: manifest.col_chunk_byte_size_uncompressed(),
            chunk_byte_size: manifest.col_chunk_byte_size(),
            manifest,
            max_on_wire_bytes_per_batch: options.max_on_wire_bytes_per_batch,

            current_batch: Default::default(),

            to_load: Vec::new(),
        }
    }

    fn finish_batch(&mut self) -> Result<(), PrefetchError> {
        if self.current_batch.row_indices.is_empty() {
            return Ok(());
        }

        let row_indices: BTreeSet<usize> = self.current_batch.row_indices.iter().copied().collect();

        let col_chunk_ids: &[ChunkId] = self.manifest.col_chunk_ids();

        let mut root_chunk_ids = ahash::HashSet::default();
        for &row_idx in &row_indices {
            root_chunk_ids.insert(col_chunk_ids[row_idx]);
        }

        let rb = re_arrow_util::take_record_batch(
            self.manifest.chunk_fetcher_rb(),
            &std::mem::take(&mut self.current_batch.row_indices),
        )?;
        self.to_load.push((
            rb,
            RequestInfo {
                root_chunk_ids,
                row_indices,
                size_bytes_uncompressed: self.current_batch.uncompressed_bytes,
                size_bytes_on_wire: self.current_batch.on_wire_bytes,
            },
        ));
        self.current_batch.reset();
        Ok(())
    }

    /// Add a chunk to be fetched.
    fn try_fetch(
        &mut self,
        chunk_row_idx: usize,
        budget: &mut RemainingByteBudget,
    ) -> Result<bool, PrefetchError> {
        let on_wire_byte_size = self.chunk_byte_size[chunk_row_idx];

        if !budget.try_fit_on_wire(on_wire_byte_size) {
            return Ok(false);
        }

        let uncompressed_chunk_size = self.chunk_byte_size_uncompressed[chunk_row_idx];

        self.current_batch.row_indices.push(chunk_row_idx);
        self.current_batch.uncompressed_bytes += uncompressed_chunk_size;
        self.current_batch.on_wire_bytes += on_wire_byte_size;

        if self.max_on_wire_bytes_per_batch <= self.current_batch.on_wire_bytes {
            self.finish_batch()?;
        }

        Ok(true)
    }

    /// Returns all batches that should be loaded
    #[must_use = "Load the returned batches"]
    pub fn finish(mut self) -> Result<Vec<(RecordBatch, RequestInfo)>, PrefetchError> {
        self.finish_batch()?;
        Ok(self.to_load)
    }
}

fn warn_entity_exceeds_memory(entity_path: &str) {
    // TODO(RR-3344): improve this error message
    if cfg!(target_arch = "wasm32") {
        re_log::debug_once!(
            "Cannot load all of entity '{entity_path}', because its size exceeds the memory budget. Try the native viewer instead, or split up your large assets (e.g. prefer VideoStream over VideoAsset)."
        );
    } else {
        re_log::warn_once!(
            "Cannot load all of entity '{entity_path}', because its size exceeds the memory budget. You should increase the `--memory-limit` or try to split up your large assets (e.g. prefer VideoStream over VideoAsset)."
        );
    }
}

pub struct RemainingByteBudget {
    /// Fixed total — used to check if a single chunk is too large to ever fit.
    pub total_bytes_in_memory: u64,
    remaining_bytes_in_memory: u64,
    remaining_bytes_on_wire: u64,
}

impl RemainingByteBudget {
    /// If either the wire budget, or memory budget is filled.
    pub fn full(&self) -> bool {
        self.remaining_bytes_in_memory == 0 || self.remaining_bytes_on_wire == 0
    }

    /// Create a new budget with the given memory and on-wire limits.
    pub fn new(total_bytes_in_memory: u64, max_bytes_on_wire: u64) -> Self {
        Self {
            total_bytes_in_memory,
            remaining_bytes_in_memory: total_bytes_in_memory,
            remaining_bytes_on_wire: max_bytes_on_wire,
        }
    }

    /// Try to fit `bytes` into the remaining memory budget.
    ///
    /// Returns `true` if it fits (even partially), `false` if the budget is exhausted.
    fn try_fit_in_memory(&mut self, bytes: u64, required: bool) -> bool {
        self.remaining_bytes_in_memory = self.remaining_bytes_in_memory.saturating_sub(bytes);

        if self.remaining_bytes_in_memory == 0 {
            if required {
                if cfg!(target_arch = "wasm32") {
                    re_log::warn_once!(
                        "This recording is very memory intense, and the Wasm32 build only has 4GiB of memory. Consider using the native viewer to use all of your RAM."
                    );
                } else {
                    re_log::warn_once!(
                        "The current recording may use more data than your current memory budget."
                    );
                }
                true // Risk it! We are conservative in our budgeting
            } else {
                false
            }
        } else {
            true
        }
    }

    /// Try to fit `bytes` into the remaining on-wire budget.
    ///
    /// Returns `true` if it fits (even partially), `false` if the budget is exhausted.
    fn try_fit_on_wire(&mut self, bytes: u64) -> bool {
        let fit_on_wire = self.remaining_bytes_on_wire > 0;

        self.remaining_bytes_on_wire = self.remaining_bytes_on_wire.saturating_sub(bytes);

        fit_on_wire
    }
}

/// Chunk that we've prioritized in `chunks_in_priority`.
#[derive(Clone, Copy)]
pub struct PrioritizedRootChunk {
    /// If this chunk came from `used_physical` or `missing_virtual` it's required
    /// and we log a warning if we can't fit it.
    stage: FetchStage,

    root_chunk_id: ChunkId,
}

impl PrioritizedRootChunk {
    fn required(root_chunk_id: ChunkId) -> Self {
        Self {
            stage: FetchStage::Required,
            root_chunk_id,
        }
    }

    fn similar(chunk_id: ChunkId) -> Self {
        Self {
            stage: FetchStage::Similar,
            root_chunk_id: chunk_id,
        }
    }

    fn everything(chunk_id: ChunkId) -> Self {
        Self {
            stage: FetchStage::Everything,
            root_chunk_id: chunk_id,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct ComponentPathKey {
    entity_path: EntityPathHash,
    component: ComponentIdentifier,
}

#[cfg(test)]
impl ComponentPathKey {
    /// Creates a dummy key for use in tests where the specific entity/component doesn't matter.
    pub fn dummy() -> Self {
        Self {
            entity_path: EntityPathHash::NONE,
            component: ComponentIdentifier::new("test"),
        }
    }
}

impl re_byte_size::SizeBytes for ComponentPathKey {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            entity_path: _,
            component: _,
        } = self;

        0
    }

    fn is_pod() -> bool {
        true
    }
}

#[derive(Clone, Default)]
pub struct ProtectedChunks {
    /// All root chunks that we have an interest in having loaded,
    /// (or at least a part of them).
    ///
    /// These chunks are protected from _canceling_,
    /// i.e. we won't cancel the download of these chunks.
    pub roots: HashSet<ChunkId>,

    /// All physical chunks that are 'in' the memory limit.
    ///
    /// These chunks are protected from being gc'd.
    pub physical: HashSet<ChunkId>,
}

impl re_byte_size::SizeBytes for ProtectedChunks {
    fn heap_size_bytes(&self) -> u64 {
        let Self { roots, physical } = self;
        roots.heap_size_bytes() + physical.heap_size_bytes()
    }
}

#[derive(Default)]
#[cfg_attr(feature = "testing", derive(Clone))]
pub struct ChunkPrioritizer {
    protected_chunks: ProtectedChunks,

    /// Result of the latest fetch pass (set by [`ChunkFetcher::finish`]).
    latest_result: Option<PrioritizationState>,

    /// Chunks that are in the progress of being downloaded.
    chunk_requests: ChunkRequests,

    /// Intervals of all root chunks in the rrd manifest per timeline.
    root_chunk_intervals: BTreeMap<Timeline, SortedRangeMap<TimeInt, ChunkId>>,

    /// All static root chunks in the rrd manifest.
    static_chunk_ids: Vec<ChunkId>,

    /// Chunks that should be downloaded before any else.
    high_priority_chunks: HighPrioChunks,

    pub component_paths_from_root_id: HashMap<ChunkId, Vec<ComponentPathKey>>,

    /// Component paths that were reported either as being used or missing.
    pub components_of_interest: HashSet<ComponentPathKey>,

    /// Root chunks visited during the required pass of the current frame.
    ///
    /// Carried into the optional pass so those chunks are skipped (not double-counted).
    /// Reset at the start of each required pass.
    frame_visited: HashSet<ChunkId>,
}

impl re_byte_size::SizeBytes for ChunkPrioritizer {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            protected_chunks,
            latest_result: _,
            chunk_requests: _, // not yet implemented
            root_chunk_intervals: virtual_chunk_intervals,
            static_chunk_ids,
            high_priority_chunks,
            component_paths_from_root_id,
            components_of_interest,
            frame_visited,
        } = self;

        protected_chunks.heap_size_bytes()
            + virtual_chunk_intervals.heap_size_bytes()
            + static_chunk_ids.heap_size_bytes()
            + high_priority_chunks.heap_size_bytes()
            + component_paths_from_root_id.heap_size_bytes()
            + components_of_interest.heap_size_bytes()
            + frame_visited.heap_size_bytes()
    }
}

impl ChunkPrioritizer {
    pub fn on_rrd_manifest(&mut self, delta: &RrdManifest) {
        self.update_static_chunks(delta);
        self.update_chunk_intervals(delta);
        self.update_high_priority_chunks(delta);

        for (entity, per_component) in delta.static_map() {
            for (component, chunk) in per_component {
                self.component_paths_from_root_id
                    .entry(*chunk)
                    .or_default()
                    .push(ComponentPathKey {
                        entity_path: entity.hash(),
                        component: *component,
                    });
            }
        }

        for (entity, per_timeline) in delta.temporal_map() {
            for per_component in per_timeline.values() {
                for (component, chunks) in per_component {
                    for chunk in chunks.keys() {
                        self.component_paths_from_root_id
                            .entry(*chunk)
                            .or_default()
                            .push(ComponentPathKey {
                                entity_path: entity.hash(),
                                component: *component,
                            });
                    }
                }
            }
        }
    }

    /// Result of the latest fetch pass (set by [`ChunkFetcher::finish`]).
    pub fn latest_result(&self) -> Option<PrioritizationState> {
        self.latest_result
    }

    /// Find all chunk IDs that contain components with the given prefix.
    fn find_chunks_with_component_prefix(manifest: &RrdManifest, prefix: &str) -> HighPrioChunks {
        let mut temporal_chunks: BTreeMap<TimelineName, Vec<HighPrioChunk>> = Default::default();

        // We intentionally ignore static chunks, because we already prioritize ALL static chunks.

        for timelines in manifest.temporal_map().values() {
            for (timeline, components) in timelines {
                for (component, chunks) in components {
                    if component.as_str().starts_with(prefix) {
                        for (chunk_id, entry) in chunks {
                            temporal_chunks.entry(*timeline.name()).or_default().push(
                                HighPrioChunk {
                                    chunk_id: *chunk_id,
                                    time_range: entry.time_range,
                                },
                            );
                        }
                    }
                }
            }
        }

        for chunks in temporal_chunks.values_mut() {
            chunks.sort_by_key(|chunk| chunk.time_range.min);
        }

        HighPrioChunks { temporal_chunks }
    }

    fn update_high_priority_chunks(&mut self, manifest: &RrdManifest) {
        // Find chunks containing transform-related components.
        // We need to download _all_ of them because any one of them could
        // contain a crucial part of the transform hierarchy.
        // Latest-at fails, because a single entity can define the transform of multiple
        // parts of a hierarchy, and not all of the transform are required to be
        // available at each time point.
        // More here: https://linear.app/rerun/issue/RR-3441/required-transform-frames-arent-always-loaded
        let new_chunks = Self::find_chunks_with_component_prefix(
            manifest,
            "Transform3D:", // Hard-coding this here is VERY hacky, but I want to ship MVP
        );
        for (timeline, mut chunks) in new_chunks.temporal_chunks {
            let existing = self
                .high_priority_chunks
                .temporal_chunks
                .entry(timeline)
                .or_default();
            existing.append(&mut chunks);
            existing.sort_by_key(|chunk| chunk.time_range.min);
        }
    }

    fn update_static_chunks(&mut self, manifest: &RrdManifest) {
        for entity_chunks in manifest.static_map().values() {
            self.static_chunk_ids.extend(entity_chunks.values());
        }
        self.static_chunk_ids.sort();
        self.static_chunk_ids.dedup();
    }

    fn update_chunk_intervals(&mut self, manifest: &RrdManifest) {
        let mut per_timeline_chunks: BTreeMap<Timeline, Vec<(RangeInclusive<TimeInt>, ChunkId)>> =
            BTreeMap::default();

        for timelines in manifest.temporal_map().values() {
            for (timeline, components) in timelines {
                let timeline_chunks = per_timeline_chunks.entry(*timeline).or_default();
                for chunks in components.values() {
                    for (chunk_id, entry) in chunks {
                        timeline_chunks.push((entry.time_range.into(), *chunk_id));
                    }
                }
            }
        }

        for (timeline, chunks) in per_timeline_chunks {
            self.root_chunk_intervals
                .entry(timeline)
                .or_default()
                .extend(chunks);
        }
    }

    pub fn chunk_requests(&self) -> &ChunkRequests {
        &self.chunk_requests
    }

    pub fn chunk_requests_mut(&mut self) -> &mut ChunkRequests {
        &mut self.chunk_requests
    }

    pub fn protected_chunks(&self) -> &ProtectedChunks {
        &self.protected_chunks
    }

    /// Handle initial chunk prioritization and build a [`ChunkFetcher`].
    ///
    /// This should be called once per frame per recording, because it
    /// clears tracked missing & used chunks from the chunk store, so that can be populated again next frame.
    ///
    /// Subtracts already loaded physical chunks from the memory budget.
    pub fn prepare_chunk_fetcher<'a>(
        &'a mut self,
        store: &'a ChunkStore,
        manifest: &'a RrdManifest,
        options: &ChunkPrefetchOptions,
        time_cursor: Option<TimelinePoint>,
        root_chunks: &'a HashMap<ChunkId, RootChunkInfo>,
        budget: &mut RemainingByteBudget,
    ) -> ChunkFetcher<'a> {
        let used_and_missing = store.take_tracked_chunk_ids();

        self.frame_visited.clear();
        self.update_components_of_interest(store, &used_and_missing);
        self.protected_chunks.roots.clear();
        self.protected_chunks.physical.clear();
        self.protect_used_and_missing(store, &used_and_missing);

        for &physical_chunk_id in &used_and_missing.used_physical {
            debug_assert!(
                self.protected_chunks.physical.contains(&physical_chunk_id),
                "We added it earlier"
            );
            if let Some(chunk) = store.physical_chunk(&physical_chunk_id) {
                budget.try_fit_in_memory(Chunk::total_size_bytes(chunk.as_ref()), true);
            } else {
                re_log::debug_warn_once!("Couldn't get physical chunk from chunk store");
            }
        }

        ChunkFetcher {
            visited_root_chunks: std::mem::take(&mut self.frame_visited),
            chunk_id_scratch: Vec::new(),
            state: PrioritizationState::default(),
            prioritizer: self,
            root_chunks,
            time_cursor,
            store,
            next_chunk: None,
            fetch_stage: ChunkPriorityStage::Start(used_and_missing),

            request_batcher: Some(ChunkRequestBatcher::new(manifest, options)),
        }
    }

    fn update_components_of_interest(
        &mut self,
        store: &ChunkStore,
        used_and_missing: &QueriedChunkIdTracker,
    ) {
        re_tracing::profile_function!();

        // Basically: what components of which entities are currently being viewed by the user?
        self.components_of_interest.clear();

        let QueriedChunkIdTracker {
            used_physical,
            missing_virtual,
        } = used_and_missing;

        for physical_chunk_id in used_physical {
            if let Some(chunk) = store.physical_chunk(physical_chunk_id) {
                for component in chunk.components_identifiers() {
                    self.components_of_interest.insert(ComponentPathKey {
                        entity_path: chunk.entity_path().hash(),
                        component,
                    });
                }
            }
        }
        for missing_virtual_chunk_id in missing_virtual {
            for root_id in store.find_root_chunks(missing_virtual_chunk_id) {
                if let Some(components) = self.component_paths_from_root_id.get(&root_id) {
                    self.components_of_interest
                        .extend(components.iter().copied());
                }
            }
        }
    }

    /// Prevent these chunks from being canceled or GC:ed.
    fn protect_used_and_missing(
        &mut self,
        store: &ChunkStore,
        used_and_missing: &QueriedChunkIdTracker,
    ) {
        let QueriedChunkIdTracker {
            used_physical,
            missing_virtual,
        } = used_and_missing;

        for physical_chunk_id in used_physical {
            // We don't need to add the root(s) of this to the `protected_root_chunks`.
            // It is fine to cancel the download of the root(s),
            // as long as we don't GC this particular physical chunk.
            self.protected_chunks.physical.insert(*physical_chunk_id);
        }

        for chunk_id in missing_virtual {
            // Do not cancel any downloads of any roots of this missing chunk:
            for root_id in store.find_root_chunks(chunk_id) {
                self.protected_chunks.roots.insert(root_id);
            }
        }
    }

    /// Cancel all fetches of things that are not currently needed.
    #[must_use = "Returns root chunks whose download got cancelled. Mark them as unloaded!"]
    pub fn cancel_outdated_requests(&mut self, egui_now_time: f64) -> Vec<ChunkId> {
        self.chunk_requests
            .cancel_outdated_requests(egui_now_time, &self.protected_chunks.roots)
    }
}

/// How much we should prefetch. A higher stage also includes all lower stages.
#[derive(
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Clone,
    Copy,
    Debug,
    Hash,
    Default,
    serde::Deserialize,
    serde::Serialize,
)]
pub enum FetchStage {
    /// Fetch all required chunks, which includes:
    /// - Static chunks.
    /// - Missing chunks.
    /// - High-prio chunks (e.g Transform ones).
    Required = 0,

    /// Fetches all chunks on the component paths of chunks that were reported
    /// as used or missing.
    #[default]
    Similar = 1,

    /// Fetches everything. Starting at the time cursor.
    Everything = 2,
}

impl FetchStage {
    pub fn is_required(&self) -> bool {
        match self {
            Self::Required => true,
            Self::Similar | Self::Everything => false,
        }
    }

    pub fn is_everything(&self) -> bool {
        match self {
            Self::Required | Self::Similar => false,
            Self::Everything => true,
        }
    }
}

enum IterState {
    Uninited,
    Idx(usize),
    Done,
}

/// Chunk fetching stages, defined in the order they're done.
enum ChunkPriorityStage<'a> {
    /// Initial state.
    Start(QueriedChunkIdTracker),

    /// Fetches all missing chunks.
    Missing(std::vec::IntoIter<ChunkId>),

    /// Fetches all static chunks.
    Static(usize),

    /// Fetches high prio chunks before the time cursor in reverse order.
    HighPrio(IterState),

    /// Fetches chunks in temporal order within a specific range.
    ///
    /// If `interesting` is true, this only fetches chunks if they contain a component path
    /// that has been marked as used/missing.
    TimeQuery {
        query: RangeInclusive<TimeInt>,
        cursor: Option<OverlapCursor>,
        interesting: bool,
    },

    /// All chunks in no particular order.
    ///
    /// This will make sure we fetch chunks that aren't on the current timeline.
    Everything(std::collections::hash_map::Keys<'a, ChunkId, RootChunkInfo>),

    /// No more chunks to check.
    Done,
}

/// Per-recording state for a pre-fetch pass.
///
/// Constructed by calling `ChunkPrioritizer::prepare_chunk_fetcher`, and
/// [`Self::finish`] must be called when completed.
#[must_use]
pub struct ChunkFetcher<'a> {
    time_cursor: Option<TimelinePoint>,
    visited_root_chunks: HashSet<ChunkId>,
    chunk_id_scratch: Vec<ChunkId>,
    pub state: PrioritizationState,

    store: &'a ChunkStore,
    prioritizer: &'a mut ChunkPrioritizer,
    root_chunks: &'a HashMap<ChunkId, RootChunkInfo>,

    next_chunk: Option<PrioritizedRootChunk>,
    fetch_stage: ChunkPriorityStage<'a>,

    request_batcher: Option<ChunkRequestBatcher<'a>>,
}

impl Drop for ChunkFetcher<'_> {
    fn drop(&mut self) {
        if self.request_batcher.is_some() {
            re_log::debug_warn_once!("`ChunkFetcher::finish` not called for `ChunkFetcher`");
        }
    }
}

impl ChunkFetcher<'_> {
    fn peek_chunk(&mut self) -> Option<PrioritizedRootChunk> {
        if self.next_chunk.is_none() {
            self.next_chunk = self.next_chunk();
        }

        self.next_chunk
    }

    /// Get the next root chunk in priority order.
    ///
    /// This may return duplicates!
    fn next_chunk(&mut self) -> Option<PrioritizedRootChunk> {
        if let Some(chunk) = self.next_chunk.take() {
            return Some(chunk);
        }

        loop {
            match &mut self.fetch_stage {
                ChunkPriorityStage::Start(tracker) => {
                    let mut missing_roots = Vec::new();
                    for missing_virtual_chunk_id in &tracker.missing_virtual {
                        self.store
                            .collect_root_ids(missing_virtual_chunk_id, &mut missing_roots);
                    }
                    missing_roots.sort();
                    missing_roots.dedup();

                    self.fetch_stage = ChunkPriorityStage::Missing(missing_roots.into_iter());
                }
                ChunkPriorityStage::Missing(missing) => {
                    if let Some(missing) = missing.next() {
                        return Some(PrioritizedRootChunk::required(missing));
                    } else {
                        self.fetch_stage = ChunkPriorityStage::Static(0);
                    }
                }
                ChunkPriorityStage::Static(idx) => {
                    if let Some(c) = self.prioritizer.static_chunk_ids.get(*idx) {
                        *idx += 1;

                        return Some(PrioritizedRootChunk::required(*c));
                    } else {
                        self.fetch_stage = ChunkPriorityStage::HighPrio(IterState::Uninited);
                    }
                }
                ChunkPriorityStage::HighPrio(idx) => {
                    if let Some(time_cursor) = self.time_cursor
                        && let Some(chunks_on_timeline) = self
                            .prioritizer
                            .high_priority_chunks
                            .temporal_chunks
                            .get(time_cursor.timeline().name())
                        && let Some(current_idx) = match idx {
                            IterState::Uninited => {
                                let (new_idx, res) = if let Some(idx) = chunks_on_timeline
                                    .partition_point(|c| c.time_range.min <= time_cursor.time)
                                    .checked_sub(1)
                                {
                                    (IterState::Idx(idx), Some(idx))
                                } else {
                                    (IterState::Done, None)
                                };

                                *idx = new_idx;

                                res
                            }
                            IterState::Idx(idx) => Some(*idx),
                            IterState::Done => None,
                        }
                        && let Some(c) = chunks_on_timeline.get(current_idx)
                    {
                        *idx = if let Some(idx) = current_idx.checked_sub(1) {
                            IterState::Idx(idx)
                        } else {
                            IterState::Done
                        };

                        return Some(PrioritizedRootChunk::required(c.chunk_id));
                    } else if let Some(time_cursor) = self.time_cursor {
                        self.fetch_stage = ChunkPriorityStage::TimeQuery {
                            query: time_cursor.time..=TimeInt::MAX,
                            cursor: None,
                            interesting: true,
                        };
                    } else {
                        self.fetch_stage = ChunkPriorityStage::Everything(self.root_chunks.keys());
                    }
                }
                ChunkPriorityStage::TimeQuery {
                    query,
                    cursor,
                    interesting,
                } => {
                    if let Some(time_cursor) = self.time_cursor
                        && let Some(map) = self
                            .prioritizer
                            .root_chunk_intervals
                            .get(&time_cursor.timeline())
                        && let Some((_, chunk_id)) = {
                            let mut iter = match *cursor {
                                Some(c) => map.resume_query(query.clone(), c),
                                None => map.query(query.clone()),
                            };

                            // Skip chunks that don't match the current interest filter.
                            let chunk = iter.find(|(_, c)| {
                                let is_interesting = self
                                    .prioritizer
                                    .component_paths_from_root_id
                                    .get(c)
                                    .is_some_and(|k| {
                                        k.iter().any(|k| {
                                            self.prioritizer.components_of_interest.contains(k)
                                        })
                                    });

                                is_interesting == *interesting
                            });

                            *cursor = Some(iter.cursor());

                            chunk
                        }
                    {
                        return Some(if *interesting {
                            PrioritizedRootChunk::similar(*chunk_id)
                        } else {
                            PrioritizedRootChunk::everything(*chunk_id)
                        });
                    } else if let Some(time_cursor) = self.time_cursor {
                        // Go from after time cursor, to before time cursor.
                        if *query.end() == TimeInt::MAX {
                            self.fetch_stage = ChunkPriorityStage::TimeQuery {
                                query: TimeInt::MIN..=time_cursor.time.saturating_sub(1),
                                cursor: None,
                                interesting: *interesting,
                            };
                        }
                        // Go from interesting to uninteresting.
                        else if *interesting {
                            self.fetch_stage = ChunkPriorityStage::TimeQuery {
                                query: time_cursor.time..=TimeInt::MAX,
                                cursor: None,
                                interesting: false,
                            };
                        } else {
                            self.fetch_stage =
                                ChunkPriorityStage::Everything(self.root_chunks.keys());
                        }
                    } else {
                        self.fetch_stage = ChunkPriorityStage::Everything(self.root_chunks.keys());
                    }
                }
                ChunkPriorityStage::Everything(chunks) => {
                    if let Some(chunk_id) = chunks.next() {
                        return Some(PrioritizedRootChunk::everything(*chunk_id));
                    } else {
                        self.fetch_stage = ChunkPriorityStage::Done;
                    }
                }
                ChunkPriorityStage::Done => return None,
            }
        }
    }

    /// Iterate through prioritized chunks, consuming budget.
    ///
    /// `to_state` determines how many chunks we process before stopping (within budget).
    pub fn fetch(
        &mut self,
        budget: &mut RemainingByteBudget,
        to_state: FetchStage,
    ) -> Result<(), PrefetchError> {
        let Some(mut batcher) = self.request_batcher.take() else {
            return Ok(());
        };

        let res = self.fetch_inner(&mut batcher, budget, to_state);

        self.request_batcher = Some(batcher);

        res
    }

    fn fetch_inner(
        &mut self,
        batcher: &mut ChunkRequestBatcher<'_>,
        budget: &mut RemainingByteBudget,
        to_state: FetchStage,
    ) -> Result<(), PrefetchError> {
        if self.state.all_required_are_loaded.is_none() {
            self.state.all_required_are_loaded = Some(true);
        }

        let entity_paths = batcher.manifest.col_chunk_entity_path_raw();

        loop {
            // Peek before consuming so we can stop without eating the first optional
            // chunk when doing the required-only pass.
            if self.peek_chunk().is_some_and(|next| next.stage > to_state) {
                break;
            }

            let Some(PrioritizedRootChunk {
                stage,
                root_chunk_id,
            }) = self.next_chunk()
            else {
                break;
            };

            if !self.visited_root_chunks.insert(root_chunk_id) {
                continue; // Already handled earlier in the priority order.
            }

            let Some(root_chunk) = self.root_chunks.get(&root_chunk_id) else {
                re_log::debug_warn_once!("Missing root chunk");
                continue;
            };

            self.store
                .collect_physical_descendents_of(&root_chunk_id, &mut self.chunk_id_scratch);

            match root_chunk.state {
                LoadState::Unloaded | LoadState::InTransit => {
                    if stage.is_required() {
                        self.state.all_required_are_loaded = Some(false);
                    }

                    let row_idx = root_chunk.row_id;

                    // We count only the chunks we are interested in as being part of the memory budget.
                    // The others can/will be evicted as needed.
                    let uncompressed_chunk_size = batcher.chunk_byte_size_uncompressed[row_idx];

                    if budget.total_bytes_in_memory < uncompressed_chunk_size {
                        warn_entity_exceeds_memory(entity_paths.value(row_idx));
                        self.state.some_chunks_too_big = true;
                        self.chunk_id_scratch.clear();
                        continue;
                    }

                    if !budget.try_fit_in_memory(uncompressed_chunk_size, stage.is_required()) {
                        self.state.memory_budget_filled = true;
                        self.chunk_id_scratch.clear();
                        break;
                    }

                    if root_chunk.state == LoadState::Unloaded
                        && !batcher.try_fetch(row_idx, budget)?
                    {
                        // If we don't have anything more to fetch we stop looking.
                        //
                        // This isn't entirely correct gc wise. But if we evict chunks
                        // we didn't get to because of this break, we won't be fighting
                        // back and forth with gc since there's some unloaded
                        // chunks inbetween we have to download first. After
                        // which we won't stop prioritizing which chunks should
                        // be in memory here.
                        self.state.transit_budget_filled = true;
                        self.chunk_id_scratch.clear();
                        break;
                    }

                    self.prioritizer
                        .protected_chunks
                        .roots
                        .insert(root_chunk_id);
                    self.prioritizer
                        .protected_chunks
                        .physical
                        .extend(self.chunk_id_scratch.drain(..));
                }

                LoadState::FullyLoaded => {
                    self.prioritizer
                        .protected_chunks
                        .roots
                        .insert(root_chunk_id);

                    for chunk_id in self.chunk_id_scratch.drain(..) {
                        if self
                            .prioritizer
                            .protected_chunks
                            .physical
                            .contains(&chunk_id)
                        {
                            continue; // Already counted as part of our byte budget.
                        }

                        let Some(chunk) = self.store.physical_chunk(&chunk_id) else {
                            re_log::debug_warn_once!(
                                "Couldn't get physical chunk from chunk store"
                            );
                            continue;
                        };

                        let bytes = Chunk::total_size_bytes(chunk.as_ref());
                        if !budget.try_fit_in_memory(bytes, stage.is_required()) {
                            self.state.memory_budget_filled = true;
                            break;
                        }

                        self.prioritizer.protected_chunks.physical.insert(chunk_id);
                    }
                    // `drain` drops remaining elements on break, but clear to be explicit.
                    self.chunk_id_scratch.clear();

                    // Don't continue if we already hit the limit with this.
                    if self.state.memory_budget_filled {
                        break;
                    }
                }
            }
        }

        // If budget ran out before all required chunks were seen, flag it.
        if self
            .peek_chunk()
            .is_some_and(|next| next.stage.is_required())
        {
            self.state.all_required_are_loaded = Some(false);
        }

        Ok(())
    }

    /// Handle the result of a [`ChunkFetcher`].
    pub fn finish(
        mut self,
        load_chunks: &dyn Fn(RecordBatch) -> super::ChunkPromise,
    ) -> Result<ChunkFetchResult, PrefetchError> {
        let prioritizer = &mut *self.prioritizer;

        prioritizer.frame_visited = std::mem::take(&mut self.visited_root_chunks);
        let mut state = self.state;
        if state.all_required_are_loaded.is_none() {
            // `fetch` was never called, preserve the previous value.
            state.all_required_are_loaded = prioritizer
                .latest_result
                .as_ref()
                .and_then(|prev| prev.all_required_are_loaded);
        }
        prioritizer.latest_result = Some(state);

        let mut res = ChunkFetchResult {
            new_in_transit_chunks: Vec::new(),
            time_cursor: self.time_cursor,
        };

        if let Some(batcher) = self.request_batcher.take() {
            let to_load = batcher.finish()?;
            for (rb, batch_info) in to_load {
                res.new_in_transit_chunks
                    .extend(batch_info.root_chunk_ids.iter().copied());
                let promise = load_chunks(rb);
                let batch = crate::chunk_requests::ChunkBatchRequest {
                    promise: Mutex::new(Some(promise)),
                    info: batch_info.into(),
                };
                self.prioritizer.chunk_requests_mut().add(batch);
            }
        }

        Ok(res)
    }
}

#[must_use]
pub struct ChunkFetchResult {
    pub(super) new_in_transit_chunks: Vec<ChunkId>,
    pub(super) time_cursor: Option<TimelinePoint>,
}
