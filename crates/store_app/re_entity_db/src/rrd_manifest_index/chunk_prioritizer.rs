use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;
use std::time::Duration;

use ahash::{HashMap, HashSet};
use arrow::array::RecordBatch;
use itertools::chain;
use re_byte_size::SizeBytes as _;
use re_chunk::{Chunk, ChunkId, ComponentIdentifier, TimeInt, Timeline, TimelineName};
use re_chunk_index::RrdManifest;
use re_chunk_store::{ChunkStore, QueriedChunkIdTracker};
use re_int::SaturatingInto as _;
use re_log::debug_assert;
use re_log_types::{AbsoluteTimeRange, EntityPathHash, TimelinePoint};
use re_mutex::Mutex;

use crate::{
    chunk_requests::{ChunkRequests, RequestInfo},
    rrd_manifest_index::{LoadState, RootChunkInfo},
    sorted_range_map::{OverlapIterState, SortedRangeMap},
};

#[derive(Clone, Copy, Default, re_byte_size::SizeBytes)]
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
#[derive(Clone, Default, re_byte_size::SizeBytes)]
struct HighPrioChunks {
    /// Sorted by time range min.
    temporal_chunks: BTreeMap<TimelineName, Vec<HighPrioChunk>>,
}

#[derive(Clone, re_byte_size::SizeBytes)]
struct HighPrioChunk {
    chunk_id: ChunkId,
    time_range: AbsoluteTimeRange,
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

    /// Add a chunk to be fetched, and subtract its size from the on-wire budget.
    fn fetch(
        &mut self,
        chunk_row_idx: usize,
        budget: &mut RemainingByteBudget,
    ) -> Result<(), PrefetchError> {
        let on_wire_byte_size = self.chunk_byte_size[chunk_row_idx];

        budget.remaining_bytes_on_wire = budget
            .remaining_bytes_on_wire
            .saturating_sub_unsigned(on_wire_byte_size);

        let uncompressed_chunk_size = self.chunk_byte_size_uncompressed[chunk_row_idx];

        self.current_batch.row_indices.push(chunk_row_idx);
        self.current_batch.uncompressed_bytes += uncompressed_chunk_size;
        self.current_batch.on_wire_bytes += on_wire_byte_size;

        if self.max_on_wire_bytes_per_batch <= self.current_batch.on_wire_bytes {
            self.finish_batch()?;
        }

        Ok(())
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

    /// The amount of bytes left to download on wire.
    ///
    /// This is allowed to go in the negatives, since we allow downloading
    /// chunks larger than the budget. But if it's 0 or less, no more chunks
    /// will be requested.
    pub remaining_bytes_on_wire: i64,
}

impl RemainingByteBudget {
    /// If either the wire budget, or memory budget is filled.
    pub fn full(&self) -> bool {
        self.remaining_bytes_in_memory == 0 || self.on_wire_full()
    }

    /// If no more chunks can be requested.
    fn on_wire_full(&self) -> bool {
        self.remaining_bytes_on_wire <= 0
    }

    /// If no more chunks up to `stage` can be requested.
    ///
    /// `Required` chunks are requested even once the memory budget is used up.
    fn full_for_stage(&self, stage: FetchStage) -> bool {
        if stage.is_required() {
            self.on_wire_full()
        } else {
            self.full()
        }
    }

    /// Create a new budget with the given memory and on-wire limits.
    pub fn new(total_bytes_in_memory: u64, max_bytes_on_wire: u64) -> Self {
        Self {
            total_bytes_in_memory,
            remaining_bytes_in_memory: total_bytes_in_memory,
            remaining_bytes_on_wire: max_bytes_on_wire.saturating_into::<i64>(),
        }
    }

    /// Whether [`Self::try_fit_in_memory`] would return `true` for `bytes`.
    ///
    /// Leaves the remaining memory budget unchanged.
    fn fits_in_memory(&self, bytes: u64, required: bool) -> bool {
        required || bytes < self.remaining_bytes_in_memory
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

    fn indicated(root_chunk_id: ChunkId) -> Self {
        Self {
            stage: FetchStage::Indicated,
            root_chunk_id,
        }
    }

    fn similar(chunk_id: ChunkId, time_cursor_offset: Option<Duration>) -> Self {
        Self {
            stage: FetchStage::Similar(time_cursor_offset),
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

#[derive(Copy, Clone, PartialEq, Eq, Hash, re_byte_size::SizeBytes)]
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
            component: ComponentIdentifier::from("test"),
        }
    }
}

#[derive(Clone, Default, re_byte_size::SizeBytes)]
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

#[derive(Default, re_byte_size::SizeBytes)]
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

#[derive(Clone, Copy)]
pub struct PrefetchTimeCursor {
    pub time_cursor: TimelinePoint,

    /// How fast the time cursor would move in `TimeInt / real second` if
    /// not paused.
    pub speed_if_unpaused: f64,

    /// If the time playing is looped this defines what range is looped.
    pub loop_range: Option<AbsoluteTimeRange>,
}

impl std::ops::Deref for PrefetchTimeCursor {
    type Target = TimelinePoint;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.time_cursor
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

    /// Find all chunk IDs that contain components of one of the given archetypes.
    fn find_chunks_with_archetypes(manifest: &RrdManifest, prefixes: &[&str]) -> HighPrioChunks {
        let mut temporal_chunks: BTreeMap<TimelineName, Vec<HighPrioChunk>> = Default::default();

        // We intentionally ignore static chunks, because we already prioritize ALL static chunks.

        for timelines in manifest.temporal_map().values() {
            for (timeline, components) in timelines {
                for (component, chunks) in components {
                    if prefixes
                        .iter()
                        .any(|prefix| component.as_str().starts_with(prefix))
                    {
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
        let new_chunks = Self::find_chunks_with_archetypes(
            manifest,
            // TODO(RR-5745): Don't hardcode this.
            &["Transform3D:", "Pinhole:"],
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
        time_cursor: Option<PrefetchTimeCursor>,
        root_chunks: &'a HashMap<ChunkId, RootChunkInfo>,
        budget: &mut RemainingByteBudget,
    ) -> ChunkFetcher<'a> {
        let chunk_usage_tracker = store.take_tracked_chunk_ids();

        self.frame_visited.clear();
        self.update_components_of_interest(store, &chunk_usage_tracker);
        self.protected_chunks.roots.clear();
        self.protected_chunks.physical.clear();
        self.protect_used_and_missing(store, &chunk_usage_tracker);

        for &physical_chunk_id in &chunk_usage_tracker.used_physical {
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

        let unvisited_in_transit_roots = self.chunk_requests.pending_root_chunk_ids().collect();

        ChunkFetcher {
            unvisited_in_transit_roots,
            visited_root_chunks: std::mem::take(&mut self.frame_visited),
            chunk_id_scratch: Vec::new(),
            state: PrioritizationState::default(),
            prioritizer: self,
            root_chunks,
            time_cursor,
            store,
            next_chunk: None,
            fetch_stage: ChunkPriorityStage::Start,
            tracker: chunk_usage_tracker,
            protect_skipped_unloaded: false,

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
            transient_used_physical: _,
            missing_virtual,
            transient_missing_virtual: _,
            indicated_virtual,
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
        for missing_virtual_chunk_id in chain!(missing_virtual.keys(), indicated_virtual) {
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
            transient_used_physical,
            missing_virtual,
            transient_missing_virtual,
            indicated_virtual,
        } = used_and_missing;

        for physical_chunk_id in std::iter::chain(used_physical.iter(), transient_used_physical) {
            // We don't need to add the root(s) of this to the `protected_root_chunks`.
            // It is fine to cancel the download of the root(s),
            // as long as we don't GC this particular physical chunk.
            self.protected_chunks.physical.insert(*physical_chunk_id);
        }

        for chunk_id in chain!(
            missing_virtual.keys(),
            transient_missing_virtual,
            indicated_virtual,
        ) {
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
#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
#[repr(u32)]
pub enum FetchStage {
    /// Fetch all required chunks, which includes:
    /// - Missing chunks.
    /// - High-prio chunks before the time cursor (e.g Transform ones).
    Required = 0,

    /// Includes chunks that were indicated as required soon, which includes:
    /// - Chunks indicated intentionally as indicated in various visualizers like video.
    /// - Some future high prio chunks.
    /// - Static chunks.
    Indicated = 1,

    /// Fetches all chunks on the component paths of chunks that were reported
    /// as used or missing within the given time range.
    ///
    /// This is in number of seconds ahead if the timeline were to be played.
    Similar(Option<Duration>) = 2,

    /// Fetches everything. Starting at the time cursor.
    Everything = 3,
}

impl PartialOrd for FetchStage {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FetchStage {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (self, other) {
            (Self::Required, Self::Required)
            | (Self::Indicated, Self::Indicated)
            | (Self::Everything, Self::Everything) => Ordering::Equal,

            (Self::Similar(a), Self::Similar(b)) => match (a, b) {
                (Some(a), Some(b)) => a.cmp(b),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            },

            (Self::Required, _) | (_, Self::Everything) => Ordering::Less,
            (_, Self::Required) | (Self::Everything, _) => Ordering::Greater,

            (Self::Indicated, _) => Ordering::Less,
            (_, Self::Indicated) => Ordering::Greater,
        }
    }
}

impl Default for FetchStage {
    fn default() -> Self {
        Self::Similar(Some(Duration::from_secs(30)))
    }
}

impl FetchStage {
    /// The highest stage preview recordings fetch.
    pub const MAX_PREVIEW: Self = Self::Similar(Some(Duration::from_secs(10)));

    pub fn is_required(&self) -> bool {
        match self {
            Self::Required => true,
            Self::Indicated | Self::Similar(_) | Self::Everything => false,
        }
    }

    pub fn is_everything(&self) -> bool {
        match self {
            Self::Required | Self::Indicated | Self::Similar(_) => false,
            Self::Everything => true,
        }
    }
}

enum IterState {
    Uninited,
    Idx(usize),
    Done,
}

#[derive(Clone, Copy)]
enum TimeRangeStage {
    AfterCursor,
    BeforeCursor,
    AfterCursorOutsideLoop,
    BeforeCursorOutsideLoop,
}

impl TimeRangeStage {
    fn next(&self) -> Option<Self> {
        match self {
            Self::AfterCursor => Some(Self::BeforeCursor),
            Self::BeforeCursor => Some(Self::AfterCursorOutsideLoop),
            Self::AfterCursorOutsideLoop => Some(Self::BeforeCursorOutsideLoop),
            Self::BeforeCursorOutsideLoop => None,
        }
    }
}

/// Chunk fetching stages, defined in the order they're done.
enum ChunkPriorityStage<'a> {
    /// Initial state.
    Start,

    /// Fetches all missing chunks.
    Missing(std::vec::IntoIter<ChunkId>),

    /// Fetches high prio chunks before the time cursor in reverse order.
    HighPrioBefore(IterState),

    /// Fetches all chunks indicated as needed soon.
    Indicated(std::vec::IntoIter<ChunkId>),

    HighPrioAfter {
        idx: IterState,
        buffer_time: f64,
    },

    /// Fetches all static chunks.
    Static(usize),

    /// Fetches chunks in temporal order within a specific range.
    ///
    /// If `interesting` is true, this only fetches chunks if they contain a component path
    /// that has been marked as used/missing.
    TimeQuery {
        stage: TimeRangeStage,
        iter_state: Option<OverlapIterState>,
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
    time_cursor: Option<PrefetchTimeCursor>,
    visited_root_chunks: HashSet<ChunkId>,

    /// Root chunks of the downloads in progress that no pass has visited yet.
    unvisited_in_transit_roots: HashSet<ChunkId>,

    chunk_id_scratch: Vec<ChunkId>,
    pub state: PrioritizationState,

    store: &'a ChunkStore,
    prioritizer: &'a mut ChunkPrioritizer,
    root_chunks: &'a HashMap<ChunkId, RootChunkInfo>,

    next_chunk: Option<PrioritizedRootChunk>,
    fetch_stage: ChunkPriorityStage<'a>,
    tracker: QueriedChunkIdTracker,

    /// A protect pass skipped an unloaded chunk. That chunk counts as visited, so a later
    /// [`Self::fetch`] doesn't request it.
    protect_skipped_unloaded: bool,

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
                ChunkPriorityStage::Start => {
                    let mut missing_roots = Vec::new();
                    for missing_virtual_chunk_id in self.tracker.missing_virtual.keys() {
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
                    }
                    self.fetch_stage = ChunkPriorityStage::HighPrioBefore(IterState::Uninited);
                }
                ChunkPriorityStage::HighPrioBefore(_) => {
                    if let Some(prioritized) = self.high_prio_chunk_stage() {
                        return Some(prioritized);
                    }

                    let mut indicated_roots = Vec::new();
                    for missing_virtual_chunk_id in std::iter::chain(
                        self.tracker.transient_missing_virtual.iter(),
                        &self.tracker.indicated_virtual,
                    ) {
                        self.store
                            .collect_root_ids(missing_virtual_chunk_id, &mut indicated_roots);
                    }
                    indicated_roots.sort();
                    indicated_roots.dedup();

                    self.fetch_stage = ChunkPriorityStage::Indicated(indicated_roots.into_iter());
                }

                ChunkPriorityStage::Indicated(indicated) => {
                    if let Some(indicated) = indicated.next() {
                        return Some(PrioritizedRootChunk::indicated(indicated));
                    }
                    self.fetch_stage = ChunkPriorityStage::HighPrioAfter {
                        idx: IterState::Uninited,
                        // Load 5 wall-clock seconds ahead of high prio chunks.
                        buffer_time: 5.0,
                    };
                }
                ChunkPriorityStage::HighPrioAfter { .. } => {
                    if let Some(prioritized) = self.high_prio_chunk_stage() {
                        return Some(prioritized);
                    }
                    self.fetch_stage = ChunkPriorityStage::Static(0);
                }
                ChunkPriorityStage::Static(idx) => {
                    if let Some(c) = self.prioritizer.static_chunk_ids.get(*idx) {
                        *idx += 1;

                        return Some(PrioritizedRootChunk::indicated(*c));
                    }
                    self.fetch_stage = ChunkPriorityStage::TimeQuery {
                        stage: TimeRangeStage::AfterCursor,
                        iter_state: None,
                        interesting: true,
                    };
                }

                ChunkPriorityStage::TimeQuery {
                    stage,
                    iter_state,
                    interesting,
                } => {
                    let stage = *stage;
                    let interesting = *interesting;
                    let mut iter_state = *iter_state;
                    if let Some(chunk) =
                        self.next_in_time_query(stage, &mut iter_state, interesting)
                    {
                        self.fetch_stage = ChunkPriorityStage::TimeQuery {
                            stage,
                            iter_state,
                            interesting,
                        };

                        return Some(chunk);
                    } else if let Some(stage) = stage.next() {
                        self.fetch_stage = ChunkPriorityStage::TimeQuery {
                            stage,
                            iter_state: None,
                            interesting,
                        };
                    } else if interesting {
                        self.fetch_stage = ChunkPriorityStage::TimeQuery {
                            stage: TimeRangeStage::AfterCursor,
                            iter_state: None,
                            interesting: false,
                        };
                    } else {
                        self.fetch_stage = ChunkPriorityStage::Everything(self.root_chunks.keys());
                    }
                }
                ChunkPriorityStage::Everything(chunks) => {
                    if let Some(chunk_id) = chunks.next() {
                        return Some(PrioritizedRootChunk::everything(*chunk_id));
                    }
                    self.fetch_stage = ChunkPriorityStage::Done;
                }
                ChunkPriorityStage::Done => return None,
            }
        }
    }

    fn high_prio_chunk_stage(&mut self) -> Option<PrioritizedRootChunk> {
        let (idx, before, buffer_time) = match &mut self.fetch_stage {
            ChunkPriorityStage::HighPrioBefore(idx) => (idx, true, 0.0),
            ChunkPriorityStage::HighPrioAfter {
                idx,
                buffer_time: time,
            } => (idx, false, *time),
            _ => return None,
        };

        if let Some(time_cursor) = self.time_cursor
            && let Some(chunks_on_timeline) = self
                .prioritizer
                .high_priority_chunks
                .temporal_chunks
                .get(time_cursor.timeline().name())
            && let Some(current_idx) = match idx {
                IterState::Uninited => {
                    let partition_point = chunks_on_timeline
                        .partition_point(|c| c.time_range.min <= time_cursor.time);
                    let start_idx = if before {
                        partition_point.checked_sub(1)
                    } else {
                        Some(partition_point)
                    };
                    let (new_idx, res) = if let Some(idx) = start_idx {
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
            && if before {
                true
            } else {
                // After load `buffer_time` wall-clock seconds into the future
                (c.time_range.min - time_cursor.time_cursor.time).as_f64()
                    / time_cursor.speed_if_unpaused
                    < buffer_time
            }
        {
            *idx = if let Some(idx) = if before {
                current_idx.checked_sub(1)
            } else {
                current_idx.checked_add(1)
            } {
                IterState::Idx(idx)
            } else {
                IterState::Done
            };

            return Some(if before {
                PrioritizedRootChunk::required(c.chunk_id)
            } else {
                PrioritizedRootChunk::indicated(c.chunk_id)
            });
        }

        None
    }

    fn next_in_time_query(
        &self,
        stage: TimeRangeStage,
        cursor: &mut Option<OverlapIterState>,
        interesting: bool,
    ) -> Option<PrioritizedRootChunk> {
        let time_cursor = self.time_cursor?;
        let query = match stage {
            TimeRangeStage::AfterCursor => {
                let loop_range = time_cursor.loop_range?;
                AbsoluteTimeRange::new(loop_range.min.max(time_cursor.time), loop_range.max)
            }
            TimeRangeStage::BeforeCursor => {
                let loop_range = time_cursor.loop_range?;
                AbsoluteTimeRange::new(
                    loop_range.min,
                    loop_range.max.min(time_cursor.time.saturating_sub(1)),
                )
            }
            TimeRangeStage::AfterCursorOutsideLoop => AbsoluteTimeRange::new(
                time_cursor
                    .loop_range
                    .map(|r| r.max + TimeInt::new_temporal(1))
                    .unwrap_or(time_cursor.time),
                TimeInt::MAX,
            ),
            TimeRangeStage::BeforeCursorOutsideLoop => AbsoluteTimeRange::new(
                TimeInt::MIN,
                time_cursor
                    .loop_range
                    .map(|r| r.min.saturating_sub(1))
                    .unwrap_or_else(|| time_cursor.time.saturating_sub(1)),
            ),
        };

        if query.is_empty() {
            return None;
        }

        let map = self
            .prioritizer
            .root_chunk_intervals
            .get(&time_cursor.timeline())?;

        let mut iter = match *cursor {
            Some(c) => map.resume_query(query.min..=query.max, c),
            None => map.query(query.min..=query.max),
        };

        // Skip chunks that don't match the current interest filter.
        let chunk = iter.find(|(_, c)| {
            let is_interesting = self
                .prioritizer
                .component_paths_from_root_id
                .get(c)
                .is_some_and(|k| {
                    k.iter()
                        .any(|k| self.prioritizer.components_of_interest.contains(k))
                });

            is_interesting == interesting
        });

        *cursor = Some(iter.cursor());

        let (range, chunk_id) = chunk?;
        let range = AbsoluteTimeRange::new(*range.start(), *range.end());

        let chunk = if interesting {
            let after = Duration::try_from_secs_f64(
                (range.min - time_cursor.time).max(TimeInt::ZERO).as_f64()
                    / time_cursor.speed_if_unpaused,
            )
            .ok();

            // The time it would take (in real time), for the time cursor to get to this chunk.
            //
            // `None` if it would never reach, if for example outside of the current loop section.
            let real_time_offset = match stage {
                TimeRangeStage::AfterCursor => after,
                TimeRangeStage::BeforeCursor => time_cursor.loop_range.and_then(|loop_range| {
                    Duration::try_from_secs_f64(
                        ((loop_range.max - time_cursor.time).max(TimeInt::ZERO)
                            + (range.min - loop_range.min).max(TimeInt::ZERO))
                        .as_f64()
                            / time_cursor.speed_if_unpaused,
                    )
                    .ok()
                }),
                TimeRangeStage::AfterCursorOutsideLoop => {
                    if time_cursor.loop_range.is_some() {
                        None
                    } else {
                        after
                    }
                }
                TimeRangeStage::BeforeCursorOutsideLoop => None,
            };

            PrioritizedRootChunk::similar(*chunk_id, real_time_offset)
        } else {
            PrioritizedRootChunk::everything(*chunk_id)
        };

        Some(chunk)
    }

    /// Iterate through prioritized chunks, consuming budget.
    ///
    /// `to_state` determines how many chunks we process before stopping (within budget).
    ///
    /// Stops at the first unloaded chunk once the on-wire budget is used up.
    /// The next call starts at that chunk.
    pub fn fetch(
        &mut self,
        budget: &mut RemainingByteBudget,
        to_state: FetchStage,
    ) -> Result<(), PrefetchError> {
        self.run_pass(budget, to_state, PassMode::Request)
    }

    /// Continue iterating through prioritized chunks up to `to_state`, without requesting any,
    /// until every chunk in transit has been visited.
    ///
    /// Chunks in transit and the loaded chunks before them stay protected as long as the
    /// memory budget allows.
    /// A later [`Self::fetch`] doesn't request the unloaded chunks this skips, so call this
    /// once the on-wire budget is used up or after the last [`Self::fetch`] up to `to_state`.
    pub(super) fn protect_in_transit(
        &mut self,
        budget: &mut RemainingByteBudget,
        to_state: FetchStage,
    ) -> Result<(), PrefetchError> {
        self.run_pass(budget, to_state, PassMode::ProtectInTransit)
    }

    fn run_pass(
        &mut self,
        budget: &mut RemainingByteBudget,
        to_state: FetchStage,
        mode: PassMode,
    ) -> Result<(), PrefetchError> {
        if mode == PassMode::Request {
            debug_assert!(
                !self.protect_skipped_unloaded,
                "`ChunkFetcher::fetch` called after a protect pass skipped an unloaded chunk"
            );
        }

        let Some(mut batcher) = self.request_batcher.take() else {
            return Ok(());
        };

        let res = self.run_pass_inner(&mut batcher, budget, to_state, mode);

        self.request_batcher = Some(batcher);

        res
    }

    fn run_pass_inner(
        &mut self,
        batcher: &mut ChunkRequestBatcher<'_>,
        budget: &mut RemainingByteBudget,
        to_state: FetchStage,
        mode: PassMode,
    ) -> Result<(), PrefetchError> {
        if self.state.all_required_are_loaded.is_none() {
            self.state.all_required_are_loaded = Some(true);
        }

        let entity_paths = batcher.manifest.col_chunk_entity_path();

        loop {
            if mode == PassMode::ProtectInTransit && self.unvisited_in_transit_roots.is_empty() {
                break;
            }

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
            self.unvisited_in_transit_roots.remove(&root_chunk_id);

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

                    if root_chunk.state == LoadState::Unloaded
                        && (mode != PassMode::Request || budget.on_wire_full())
                    {
                        self.state.transit_budget_filled |= budget.on_wire_full();
                        self.chunk_id_scratch.clear();

                        match mode {
                            PassMode::Request => {
                                // The next `fetch` starts at this chunk.
                                self.visited_root_chunks.remove(&root_chunk_id);
                                self.next_chunk = Some(PrioritizedRootChunk {
                                    stage,
                                    root_chunk_id,
                                });
                                break;
                            }
                            PassMode::ProtectOnly | PassMode::ProtectInTransit => {
                                // The skipped chunk uses none of the memory budget.
                                if !budget
                                    .fits_in_memory(uncompressed_chunk_size, stage.is_required())
                                {
                                    self.state.memory_budget_filled = true;
                                }
                                self.protect_skipped_unloaded = true;
                                continue;
                            }
                        }
                    }

                    if !budget.try_fit_in_memory(uncompressed_chunk_size, stage.is_required()) {
                        self.state.memory_budget_filled = true;
                        self.chunk_id_scratch.clear();
                        break;
                    }

                    if root_chunk.state == LoadState::Unloaded {
                        batcher.fetch(row_idx, budget)?;
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
            time_cursor: self.time_cursor.as_deref().copied(),
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PassMode {
    /// Request unloaded chunks until the on-wire budget is used up.
    Request,

    /// Skip unloaded chunks, and protect chunks in transit and loaded chunks.
    ProtectOnly,

    /// Like `ProtectOnly`, but stop once every chunk in transit has been visited.
    ProtectInTransit,
}

impl ChunkFetcher<'_> {
    /// Fetch chunks for several recordings that share one budget.
    ///
    /// Fetches all stages in a specific order:
    /// 1. `Required` for active recordings.
    /// 2. `Required` for preview recordings.
    /// 3. [`FetchStage::MAX_PREVIEW`] for active recordings.
    /// 4. [`FetchStage::MAX_PREVIEW`] for preview recordings.
    /// 5. `max_fetch_stage` for active recordings.
    /// 6. If `max_fetch_stage == Everything`, `Everything` for background recordings.
    ///
    /// Preview recordings never fetch above [`FetchStage::MAX_PREVIEW`].
    ///
    /// Stages above `max_fetch_stage` are skipped entirely.
    ///
    /// Once the on-wire budget is used up, no more chunks are requested.
    /// Chunks in transit and loaded chunks stay protected in the same stage order as fetching,
    /// up to [`FetchStage::MAX_PREVIEW`].
    /// Above it, only chunks in transit and the loaded chunks before them stay protected.
    /// Once the memory budget is used up, only `Required` chunks are requested or protected.
    pub fn fetch_for_recordings(
        active: &mut [&mut ChunkFetcher<'_>],
        previews: &mut [&mut ChunkFetcher<'_>],
        background: &mut [&mut ChunkFetcher<'_>],
        budget: &mut RemainingByteBudget,
        max_fetch_stage: FetchStage,
    ) {
        re_tracing::profile_function!();

        let preview_stage = FetchStage::MAX_PREVIEW.min(max_fetch_stage);

        for stage in [FetchStage::Required, preview_stage] {
            Self::fetch_up_to_stage(active, budget, stage, PassMode::ProtectOnly);
            Self::fetch_up_to_stage(previews, budget, stage, PassMode::ProtectOnly);
        }

        Self::fetch_up_to_stage(active, budget, max_fetch_stage, PassMode::ProtectInTransit);

        if max_fetch_stage == FetchStage::Everything {
            Self::fetch_up_to_stage(
                background,
                budget,
                FetchStage::Everything,
                PassMode::ProtectInTransit,
            );
        }
    }

    /// Requests chunks up to `stage` for every fetcher.
    ///
    /// Splits the on-wire budget evenly between the fetchers, and each fetcher requests chunks
    /// until its own share is used up. A fetcher that used up its share gets a share of what is
    /// left in the next round, until the budget is used up or every fetcher has reached the end
    /// of `stage`.
    ///
    /// Then runs another pass up to `stage` with `protect_mode` for every fetcher. That pass
    /// protects chunks in transit and loaded chunks, and requests none.
    fn fetch_up_to_stage(
        fetchers: &mut [&mut ChunkFetcher<'_>],
        budget: &mut RemainingByteBudget,
        stage: FetchStage,
        protect_mode: PassMode,
    ) {
        if fetchers.is_empty() {
            return;
        }

        let mut requesting: Vec<_> = fetchers.iter_mut().collect();

        while !requesting.is_empty() && !budget.full_for_stage(stage) {
            // Evenly distribute bytes on wire between the fetchers.
            // Every fetcher gets at least one byte, so it can request a chunk.
            let on_wire_bytes_per_fetcher =
                (budget.remaining_bytes_on_wire / requesting.len().cast_signed() as i64).max(1);

            requesting.retain_mut(|fetcher| {
                // The fetcher sees only its own share as the on-wire budget.
                // What it used is subtracted from the total afterwards.
                let on_wire_bytes_of_others = budget
                    .remaining_bytes_on_wire
                    .saturating_sub(on_wire_bytes_per_fetcher);
                budget.remaining_bytes_on_wire = on_wire_bytes_per_fetcher;

                let has_more_to_request = if budget.full_for_stage(stage) {
                    true
                } else {
                    if let Err(err) = fetcher.fetch(budget, stage) {
                        re_log::warn_once!("prefetch_chunks failed: {err}");
                    }

                    // If we've used the on wire budget, there are possibly more
                    // chunks to fetch in this fetch stage, so keep it for the
                    // next iteration.
                    budget.on_wire_full()
                };

                budget.remaining_bytes_on_wire = budget
                    .remaining_bytes_on_wire
                    .saturating_add(on_wire_bytes_of_others);

                has_more_to_request
            });
        }

        for fetcher in fetchers {
            if let Err(err) = fetcher.run_pass(budget, stage, protect_mode) {
                re_log::warn_once!("prefetch_chunks failed: {err}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use re_chunk::{EntityPath, RowId};
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{StoreId, StoreKind, TimePoint};

    use crate::ChunkPromise;
    use crate::rrd_manifest_index::RrdManifestIndex;

    use super::*;

    fn setup_test_recording(chunks: &[Arc<Chunk>]) -> (ChunkStore, RrdManifestIndex) {
        let store_id = StoreId::random(StoreKind::Recording, "test");
        let manifest =
            RrdManifest::build_in_memory_from_chunks(store_id.clone(), chunks.iter().map(|c| &**c))
                .unwrap();

        let mut store = ChunkStore::new(store_id, Default::default());
        let _events = store.insert_rrd_manifest(manifest.clone());

        let mut manifest_index = RrdManifestIndex::default();
        manifest_index
            .append(manifest, store.entity_tree())
            .unwrap();

        (store, manifest_index)
    }

    fn build_temporal_chunk(entity: &str, timeline: Timeline, time: i64) -> Arc<Chunk> {
        let point = MyPoint::new(1.0, 1.0);
        Arc::new(
            Chunk::builder(EntityPath::from(entity))
                .with_component_batch(
                    RowId::new(),
                    TimePoint::from_iter([(timeline, time)]),
                    (MyPoints::descriptor_points(), &[point] as _),
                )
                .build()
                .unwrap(),
        )
    }

    fn build_static_chunk(entity: &str) -> Arc<Chunk> {
        let point = MyPoint::new(1.0, 1.0);
        Arc::new(
            Chunk::builder(EntityPath::from(entity))
                .with_component_batch(
                    RowId::new(),
                    TimePoint::STATIC,
                    (MyPoints::descriptor_points(), &[point] as _),
                )
                .build()
                .unwrap(),
        )
    }

    /// Chunk IDs in a batch passed to the load callback, in the order the manifest gave us.
    fn chunk_ids_in_batch(rb: &RecordBatch) -> Vec<ChunkId> {
        RrdManifest::COLUMN_CHUNK_ID
            .extract(rb)
            .expect("chunk_id should decode")
            .to_vec()
    }

    /// Load callback that records every chunk ID the prioritizer asked to load.
    ///
    /// Returns the callback together with the shared buffer it writes into, so the
    /// caller can assert exactly which chunks were requested after running the fetch.
    fn recording_load_fn() -> (
        impl Fn(RecordBatch) -> ChunkPromise,
        Arc<re_mutex::Mutex<Vec<ChunkId>>>,
    ) {
        let requested = Arc::new(re_mutex::Mutex::new(Vec::<ChunkId>::new()));
        let out = Arc::clone(&requested);
        let load = move |rb: RecordBatch| {
            out.lock().extend(chunk_ids_in_batch(&rb));
            poll_promise::Promise::from_ready(Ok(vec![]))
        };
        (load, requested)
    }

    /// A time cursor at the start of `timeline` that moves 100 time units per second.
    fn time_cursor_at_zero(timeline: Timeline) -> PrefetchTimeCursor {
        PrefetchTimeCursor {
            time_cursor: (timeline, TimeInt::new_temporal(0)).into(),
            speed_if_unpaused: 100.0,
            loop_range: None,
        }
    }

    /// Finishes `fetchers` in order and returns the chunk ids they requested.
    fn finish_and_take_requested<const N: usize>(fetchers: [ChunkFetcher<'_>; N]) -> Vec<ChunkId> {
        let (load, requested) = recording_load_fn();
        for fetcher in fetchers {
            let _result = fetcher.finish(&load).unwrap();
        }
        std::mem::take(&mut *requested.lock())
    }

    struct FetchOutcome {
        requested: Vec<ChunkId>,
        result: ChunkFetchResult,
        state: PrioritizationState,
    }

    /// Run one full prioritizer pass and collect what happened: which chunks got asked
    /// for, the end-of-fetch prioritization state, and the resulting [`ChunkFetchResult`].
    fn run_fetch(
        manifest_index: &mut RrdManifestIndex,
        store: &ChunkStore,
        budget: &mut RemainingByteBudget,
        stage: FetchStage,
    ) -> FetchOutcome {
        let options = ChunkPrefetchOptions::default();
        let mut fetcher = manifest_index
            .prepare_chunk_fetcher(store, &options, None, budget)
            .expect("should create fetcher");
        fetcher.fetch(budget, stage).unwrap();
        let state = fetcher.state;
        let (load, requested) = recording_load_fn();
        let result = fetcher.finish(&load).unwrap();
        let requested = std::mem::take(&mut *requested.lock());
        FetchOutcome {
            requested,
            result,
            state,
        }
    }

    /// When the memory budget cannot hold every `FullyLoaded` chunk, only the ones
    /// that fit stay protected. This protects the rest from being pinned in memory
    /// and lets garbage collection evict them.
    #[test]
    fn fully_loaded_chunks_drop_protection_when_memory_budget_is_exhausted() {
        let tl = Timeline::new_sequence("frame");
        // Different entity paths keep the chunks from getting merged.
        let chunks: Vec<Arc<Chunk>> = (0..5)
            .map(|i| build_temporal_chunk(&format!("/entity_{i}"), tl, (i as i64 + 1) * 100))
            .collect();

        let (mut store, mut manifest_index) = setup_test_recording(&chunks);

        for chunk in &chunks {
            let events = store.insert_chunk(chunk).unwrap();
            manifest_index.on_events(&store, &events);
        }

        let total_physical_bytes: u64 = store
            .iter_physical_chunks()
            .map(|c| Chunk::total_size_bytes(c.as_ref()))
            .sum();
        assert!(total_physical_bytes > 0);
        let budget_bytes = total_physical_bytes / 2;

        let mut budget = RemainingByteBudget::new(budget_bytes, u64::MAX);
        let outcome = run_fetch(
            &mut manifest_index,
            &store,
            &mut budget,
            FetchStage::Everything,
        );

        assert!(
            outcome.state.memory_budget_filled,
            "budget should be exhausted with budget {budget_bytes} for {total_physical_bytes} total bytes"
        );
        assert!(
            outcome.requested.is_empty(),
            "fully-loaded chunks should never go through the load callback"
        );
        assert!(
            outcome.result.new_in_transit_chunks.is_empty(),
            "fully-loaded chunks should not transition to InTransit"
        );

        let protected = manifest_index.chunk_prioritizer().protected_chunks();
        assert!(
            protected.roots.len() < chunks.len(),
            "budget fit {} out of {} root chunks; expected fewer than all",
            protected.roots.len(),
            chunks.len()
        );
        assert!(
            !protected.roots.is_empty(),
            "at least one chunk must fit in half the total budget"
        );

        for root_id in &protected.roots {
            assert!(
                chunks.iter().any(|c| c.id() == *root_id),
                "protected root {root_id:?} should be one of the chunks we inserted"
            );
        }
    }

    /// The static chunk is fetched in the `Indicated` pass rather than the `Required` pass,
    /// so it never blocks playback. The remaining temporal chunks stay untouched until an
    /// `Everything` pass runs.
    #[test]
    fn indicated_pass_fetches_static_then_everything_fetches_the_rest() {
        let tl = Timeline::new_sequence("frame");
        let static_chunk = build_static_chunk("/static_entity");
        let temporal_chunks: Vec<Arc<Chunk>> = (0..3)
            .map(|i| build_temporal_chunk("/temporal_entity", tl, (i + 1) * 100))
            .collect();

        let mut all_chunks = vec![Arc::clone(&static_chunk)];
        all_chunks.extend(temporal_chunks.iter().cloned());

        let (store, mut manifest_index) = setup_test_recording(&all_chunks);

        let mut budget = RemainingByteBudget::new(u64::MAX, u64::MAX);
        let required = run_fetch(
            &mut manifest_index,
            &store,
            &mut budget,
            FetchStage::Required,
        );
        assert!(
            required.requested.is_empty(),
            "Required pass should not load the static chunk"
        );

        let indicated = run_fetch(
            &mut manifest_index,
            &store,
            &mut budget,
            FetchStage::Indicated,
        );
        assert_eq!(
            indicated.requested,
            vec![static_chunk.id()],
            "Indicated pass should load the static chunk"
        );
        assert_eq!(
            indicated.result.new_in_transit_chunks,
            vec![static_chunk.id()],
            "only the static chunk should transition to InTransit"
        );

        manifest_index.handle_fetch_result(indicated.result);

        let everything = run_fetch(
            &mut manifest_index,
            &store,
            &mut budget,
            FetchStage::Everything,
        );

        let requested: HashSet<ChunkId> = everything.requested.iter().copied().collect();
        let expected: HashSet<ChunkId> = temporal_chunks.iter().map(|c| c.id()).collect();
        assert_eq!(
            requested, expected,
            "Everything pass should load exactly the remaining temporal chunks"
        );
        assert_eq!(
            everything
                .result
                .new_in_transit_chunks
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
            expected,
        );
    }

    /// With a memory budget that only fits a couple of chunks, the fetcher stops
    /// once the budget is full and the load callback sees only that subset, all drawn
    /// from the input.
    #[test]
    fn memory_budget_caps_how_many_chunks_get_loaded() {
        let tl = Timeline::new_sequence("frame");
        let chunks: Vec<Arc<Chunk>> = (0..5)
            .map(|i| build_temporal_chunk("/e", tl, (i + 1) * 100))
            .collect();

        let (store, mut manifest_index) = setup_test_recording(&chunks);

        let manifest = manifest_index.manifest().unwrap();
        let one_chunk_size = manifest.col_chunk_byte_size_uncompressed()[0];
        assert!(
            one_chunk_size > 0,
            "manifest should report nonzero chunk size"
        );
        let budget_bytes = one_chunk_size * 2 + one_chunk_size / 2;

        let mut budget = RemainingByteBudget::new(budget_bytes, u64::MAX);
        let outcome = run_fetch(
            &mut manifest_index,
            &store,
            &mut budget,
            FetchStage::Everything,
        );

        assert!(
            outcome.state.memory_budget_filled,
            "memory budget should report as filled"
        );
        assert!(
            outcome.requested.len() < chunks.len(),
            "budget should cap the load: got {} out of {}",
            outcome.requested.len(),
            chunks.len()
        );
        assert!(
            !outcome.requested.is_empty(),
            "budget should allow at least one chunk"
        );

        assert_eq!(
            outcome
                .result
                .new_in_transit_chunks
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
            outcome.requested.iter().copied().collect::<HashSet<_>>(),
            "new_in_transit_chunks should match the IDs passed to the load callback"
        );
    }

    /// A chunk is in transit when another chunk earlier in the priority order goes missing
    /// and the on-wire budget has no room for it. The missing chunk isn't requested, but the
    /// chunk in transit stays protected and its download isn't canceled.
    #[test]
    fn in_transit_chunk_stays_protected_when_on_wire_budget_is_full() {
        let tl = Timeline::new_sequence("frame");
        let chunks: Vec<Arc<Chunk>> = (0..3)
            .map(|i| build_temporal_chunk("/entity", tl, (i + 1) * 100))
            .collect();

        let (store, mut manifest_index) = setup_test_recording(&chunks);
        let time_cursor = time_cursor_at_zero(tl);
        let options = ChunkPrefetchOptions {
            max_fetch_stage: FetchStage::Similar(None),
            ..Default::default()
        };
        let (load, requested) = recording_load_fn();

        // Room for a single chunk on the wire.
        store.report_missing_virtual_chunk_id(chunks[0].id());
        manifest_index
            .prefetch_chunks(
                &store,
                &options,
                Some(time_cursor),
                &mut RemainingByteBudget::new(u64::MAX, 1),
                &load,
            )
            .unwrap();
        assert_eq!(*requested.lock(), vec![chunks[0].id()]);
        requested.lock().clear();

        // No room on the wire.
        store.report_missing_virtual_chunk_id(chunks[2].id());
        manifest_index
            .prefetch_chunks(
                &store,
                &options,
                Some(time_cursor),
                &mut RemainingByteBudget::new(u64::MAX, 0),
                &load,
            )
            .unwrap();
        assert!(
            requested.lock().is_empty(),
            "nothing should be requested without on-wire budget"
        );

        assert_eq!(
            manifest_index.chunk_prioritizer().protected_chunks().roots,
            [chunks[0].id(), chunks[2].id()]
                .into_iter()
                .collect::<ahash::HashSet<_>>(),
            "the chunk in transit and the missing chunk should be protected"
        );

        manifest_index.cancel_outdated_requests(0.0);
        assert_eq!(
            manifest_index
                .root_chunk_info(&chunks[0].id())
                .map(|info| info.state),
            Some(LoadState::InTransit),
            "the chunk in transit should not be canceled"
        );
    }

    /// A chunk is in transit, and a loaded chunk comes after it in the priority order. Once the
    /// on-wire budget is used up, the pass stops after the chunk in transit, so the loaded
    /// chunk isn't protected.
    #[test]
    fn protect_stops_after_last_chunk_in_transit() {
        let tl = Timeline::new_sequence("frame");
        let chunks: Vec<Arc<Chunk>> = (0..3)
            .map(|i| build_temporal_chunk("/entity", tl, (i + 1) * 100))
            .collect();

        let (mut store, mut manifest_index) = setup_test_recording(&chunks);
        let events = store.insert_chunk(&chunks[2]).unwrap();
        manifest_index.on_events(&store, &events);

        let time_cursor = time_cursor_at_zero(tl);
        let options = ChunkPrefetchOptions {
            max_fetch_stage: FetchStage::Everything,
            ..Default::default()
        };
        let (load, requested) = recording_load_fn();

        // Room for a single chunk on the wire.
        store.report_missing_virtual_chunk_id(chunks[0].id());
        manifest_index
            .prefetch_chunks(
                &store,
                &options,
                Some(time_cursor),
                &mut RemainingByteBudget::new(u64::MAX, 1),
                &load,
            )
            .unwrap();
        assert_eq!(*requested.lock(), vec![chunks[0].id()]);

        // No room on the wire.
        store.report_missing_virtual_chunk_id(chunks[0].id());
        manifest_index
            .prefetch_chunks(
                &store,
                &options,
                Some(time_cursor),
                &mut RemainingByteBudget::new(u64::MAX, 0),
                &load,
            )
            .unwrap();

        let protected = manifest_index.chunk_prioritizer().protected_chunks();
        assert_eq!(
            protected.roots,
            std::iter::once(chunks[0].id()).collect::<ahash::HashSet<_>>(),
            "only the chunk in transit should be protected"
        );
        assert!(
            !protected.physical.contains(&chunks[2].id()),
            "the loaded chunk after the chunk in transit should not be protected"
        );
    }

    /// A preview recording misses several chunks and comes before a preview recording that
    /// is fully loaded. The on-wire budget fits all missing chunks, but an even share of it
    /// doesn't. The loaded preview's unused share goes to the other preview, which then
    /// requests all its missing chunks.
    #[test]
    fn unused_on_wire_share_goes_to_preview_with_missing_chunks() {
        let tl = Timeline::new_sequence("frame");
        let build_chunks = || -> Vec<Arc<Chunk>> {
            (0..3)
                .map(|i| build_temporal_chunk("/entity", tl, (i + 1) * 100))
                .collect()
        };
        let missing_chunks = build_chunks();
        let loaded_chunks = build_chunks();

        let (missing_store, mut missing_index) = setup_test_recording(&missing_chunks);
        let (mut loaded_store, mut loaded_index) = setup_test_recording(&loaded_chunks);

        for chunk in &missing_chunks {
            missing_store.report_missing_virtual_chunk_id(chunk.id());
        }
        for chunk in &loaded_chunks {
            let events = loaded_store.insert_chunk(chunk).unwrap();
            loaded_index.on_events(&loaded_store, &events);
        }

        let chunk_byte_sizes = missing_index.manifest().unwrap().col_chunk_byte_size();
        let on_wire_bytes: u64 = chunk_byte_sizes.iter().sum();
        let largest_chunk_bytes = chunk_byte_sizes.iter().copied().max().unwrap();
        assert!(
            on_wire_bytes / 2 <= on_wire_bytes - largest_chunk_bytes,
            "an even share of the on-wire budget should not fit every missing chunk"
        );
        let mut budget = RemainingByteBudget::new(u64::MAX, on_wire_bytes);
        let options = ChunkPrefetchOptions::default();

        let mut missing = missing_index
            .prepare_chunk_fetcher(&missing_store, &options, None, &mut budget)
            .unwrap();
        let mut loaded = loaded_index
            .prepare_chunk_fetcher(&loaded_store, &options, None, &mut budget)
            .unwrap();

        ChunkFetcher::fetch_for_recordings(
            &mut [],
            &mut [&mut missing, &mut loaded],
            &mut [],
            &mut budget,
            options.max_fetch_stage,
        );

        let mut requested = finish_and_take_requested([missing, loaded]);
        requested.sort();
        let mut expected: Vec<ChunkId> = missing_chunks.iter().map(|c| c.id()).collect();
        expected.sort();
        assert_eq!(
            requested, expected,
            "every missing chunk should be requested exactly once"
        );
    }

    /// Two preview recordings each miss one chunk. The on-wire budget is smaller than a chunk,
    /// so an even share of it is smaller than a chunk too. Each preview still requests its
    /// missing chunk.
    #[test]
    fn each_preview_requests_a_chunk_when_its_share_is_smaller_than_a_chunk() {
        let tl = Timeline::new_sequence("frame");
        let first_chunk = build_temporal_chunk("/entity", tl, 100);
        let second_chunk = build_temporal_chunk("/entity", tl, 100);

        let (first_store, mut first_index) =
            setup_test_recording(std::slice::from_ref(&first_chunk));
        let (second_store, mut second_index) =
            setup_test_recording(std::slice::from_ref(&second_chunk));
        first_store.report_missing_virtual_chunk_id(first_chunk.id());
        second_store.report_missing_virtual_chunk_id(second_chunk.id());

        let mut budget = RemainingByteBudget::new(u64::MAX, 2);
        let options = ChunkPrefetchOptions::default();

        let mut first = first_index
            .prepare_chunk_fetcher(&first_store, &options, None, &mut budget)
            .unwrap();
        let mut second = second_index
            .prepare_chunk_fetcher(&second_store, &options, None, &mut budget)
            .unwrap();

        ChunkFetcher::fetch_for_recordings(
            &mut [],
            &mut [&mut first, &mut second],
            &mut [],
            &mut budget,
            options.max_fetch_stage,
        );

        assert_eq!(
            finish_and_take_requested([first, second]),
            vec![first_chunk.id(), second_chunk.id()],
            "each preview should request its missing chunk"
        );
    }

    /// An active recording and a preview recording each miss a required chunk. The active
    /// recording's chunk uses up the memory budget. The preview's required chunk is still
    /// requested. The preview's unloaded chunk above the `Required` stage isn't requested,
    /// and the preview's state says the memory budget is filled.
    #[test]
    fn preview_requests_required_chunks_once_memory_budget_is_used_up() {
        let tl = Timeline::new_sequence("frame");
        let active_chunk = build_temporal_chunk("/entity", tl, 100);
        let preview_chunk = build_temporal_chunk("/entity", tl, 100);

        // Static chunks are in the `Indicated` stage, above `Required`.
        let preview_static_chunk = build_static_chunk("/static");

        let (active_store, mut active_index) =
            setup_test_recording(std::slice::from_ref(&active_chunk));
        let (preview_store, mut preview_index) = setup_test_recording(&[
            Arc::clone(&preview_chunk),
            Arc::clone(&preview_static_chunk),
        ]);
        active_store.report_missing_virtual_chunk_id(active_chunk.id());
        preview_store.report_missing_virtual_chunk_id(preview_chunk.id());

        let active_chunk_bytes = active_index
            .manifest()
            .unwrap()
            .col_chunk_byte_size_uncompressed()
            .iter()
            .sum();
        let mut budget = RemainingByteBudget::new(active_chunk_bytes, u64::MAX);
        let options = ChunkPrefetchOptions::default();

        let mut active = active_index
            .prepare_chunk_fetcher(&active_store, &options, None, &mut budget)
            .unwrap();
        let mut preview = preview_index
            .prepare_chunk_fetcher(&preview_store, &options, None, &mut budget)
            .unwrap();

        ChunkFetcher::fetch_for_recordings(
            &mut [&mut active],
            &mut [&mut preview],
            &mut [],
            &mut budget,
            options.max_fetch_stage,
        );

        let preview_state = preview.state;

        assert_eq!(
            finish_and_take_requested([active, preview]),
            vec![active_chunk.id(), preview_chunk.id()],
            "only the two required chunks should be requested"
        );
        assert!(
            preview_state.memory_budget_filled,
            "the preview's state should say the memory budget is filled"
        );
    }

    /// A recording has an unloaded chunk that is bigger than the whole memory budget, and the
    /// on-wire budget has no room. The chunk isn't requested, and the state says some chunks
    /// are too big.
    #[test]
    fn some_chunks_too_big_is_set_when_on_wire_budget_is_full() {
        let chunk = build_static_chunk("/entity");
        let (store, mut manifest_index) = setup_test_recording(std::slice::from_ref(&chunk));

        let mut budget = RemainingByteBudget::new(1, 0);
        let outcome = run_fetch(
            &mut manifest_index,
            &store,
            &mut budget,
            FetchStage::Everything,
        );

        assert!(
            outcome.requested.is_empty(),
            "a chunk bigger than the memory budget should not be requested"
        );
        assert!(
            outcome.state.some_chunks_too_big,
            "the state should say some chunks are too big"
        );
    }

    /// With a duration cap on `Similar`, only chunks the time cursor would reach
    /// within that duration get fetched. Chunks further ahead on the timeline
    /// stay untouched until a later pass asks for them.
    #[test]
    fn similar_stage_skips_chunks_beyond_reach_time() {
        let tl = Timeline::new_sequence("frame");
        // Same entity on every chunk so reporting one missing seeds interest for
        // all of them. The time query only classifies interesting chunks as
        // `Similar`.
        let chunks: Vec<Arc<Chunk>> = (0..5)
            .map(|i| build_temporal_chunk("/entity", tl, (i + 1) * 100))
            .collect();

        let (store, mut manifest_index) = setup_test_recording(&chunks);
        store.report_missing_virtual_chunk_id(chunks[0].id());

        // Chunks are spaced so that reach time equals `speed * frame`, i.e.
        // cap of 3s keeps chunks 0..=2 and drops chunks 3..=4.
        let time_cursor = PrefetchTimeCursor {
            time_cursor: (tl, TimeInt::new_temporal(0)).into(),
            speed_if_unpaused: 100.0,
            loop_range: None,
        };

        let options = ChunkPrefetchOptions::default();
        let mut budget = RemainingByteBudget::new(u64::MAX, u64::MAX);
        let mut fetcher = manifest_index
            .prepare_chunk_fetcher(&store, &options, Some(time_cursor), &mut budget)
            .expect("should create fetcher");

        fetcher
            .fetch(
                &mut budget,
                FetchStage::Similar(Some(Duration::from_secs(3))),
            )
            .unwrap();
        let (load, requested) = recording_load_fn();
        let _result = fetcher.finish(&load).unwrap();
        let requested: HashSet<ChunkId> = requested.lock().iter().copied().collect();

        let within_cap: HashSet<ChunkId> = chunks[..=2].iter().map(|c| c.id()).collect();
        let beyond_cap: HashSet<ChunkId> = chunks[3..].iter().map(|c| c.id()).collect();
        assert!(within_cap.is_subset(&requested));
        assert!(beyond_cap.is_disjoint(&requested));
    }

    /// Without a duration cap, `Similar(None)` fetches every chunk the time
    /// query classifies as similar, regardless of where they sit on the timeline.
    #[test]
    fn similar_stage_without_time_cap_fetches_all_reachable_chunks() {
        let tl = Timeline::new_sequence("frame");
        let chunks: Vec<Arc<Chunk>> = (0..5)
            .map(|i| build_temporal_chunk("/entity", tl, (i + 1) * 100))
            .collect();

        let (store, mut manifest_index) = setup_test_recording(&chunks);
        store.report_missing_virtual_chunk_id(chunks[0].id());

        let time_cursor = PrefetchTimeCursor {
            time_cursor: (tl, TimeInt::new_temporal(0)).into(),
            speed_if_unpaused: 100.0,
            loop_range: None,
        };

        let options = ChunkPrefetchOptions::default();
        let mut budget = RemainingByteBudget::new(u64::MAX, u64::MAX);
        let mut fetcher = manifest_index
            .prepare_chunk_fetcher(&store, &options, Some(time_cursor), &mut budget)
            .expect("should create fetcher");

        fetcher
            .fetch(&mut budget, FetchStage::Similar(None))
            .unwrap();
        let (load, requested) = recording_load_fn();
        let _result = fetcher.finish(&load).unwrap();
        let requested: HashSet<ChunkId> = requested.lock().iter().copied().collect();

        let expected: HashSet<ChunkId> = chunks.iter().map(|c| c.id()).collect();
        assert_eq!(requested, expected);
    }

    /// During loop playback, reach time for a chunk before the cursor is the
    /// wrap-around time through the loop. Chunks outside the loop are
    /// considered unreachable and are skipped whenever the cap is finite.
    #[test]
    fn similar_stage_honours_loop_wrap_around_and_excludes_chunks_outside_loop() {
        let tl = Timeline::new_sequence("frame");
        // The chunks span every `TimeRangeStage` relative to the cursor and the
        // loop. First a chunk in `BeforeCursorOutsideLoop`, then one in
        // `BeforeCursor` whose reach time wraps around through the loop, then
        // three in `AfterCursor` covering the cursor, mid-loop, and the loop
        // end, then a final one in `AfterCursorOutsideLoop`.
        let chunks: Vec<Arc<Chunk>> = [100, 450, 500, 600, 700, 900]
            .into_iter()
            .map(|t| build_temporal_chunk("/entity", tl, t))
            .collect();

        let (store, mut manifest_index) = setup_test_recording(&chunks);
        // Report a chunk that sits in `BeforeCursorOutsideLoop` as missing so
        // the `Required` pass picks it up and also seeds interest for the rest.
        store.report_missing_virtual_chunk_id(chunks[0].id());

        let time_cursor = PrefetchTimeCursor {
            time_cursor: (tl, TimeInt::new_temporal(500)).into(),
            speed_if_unpaused: 100.0,
            loop_range: Some(AbsoluteTimeRange::new(
                TimeInt::new_temporal(400),
                TimeInt::new_temporal(700),
            )),
        };

        let options = ChunkPrefetchOptions::default();
        let mut budget = RemainingByteBudget::new(u64::MAX, u64::MAX);
        let mut fetcher = manifest_index
            .prepare_chunk_fetcher(&store, &options, Some(time_cursor), &mut budget)
            .expect("should create fetcher");

        // Cap picked so the wrap-around chunk fits but chunks outside the loop
        // do not. Their reach time is unreachable under loop playback.
        fetcher
            .fetch(
                &mut budget,
                FetchStage::Similar(Some(Duration::from_secs(3))),
            )
            .unwrap();
        let (load, requested) = recording_load_fn();
        let _result = fetcher.finish(&load).unwrap();
        let requested: HashSet<ChunkId> = requested.lock().iter().copied().collect();

        // The `Required` chunk and every chunk inside the loop should be
        // fetched. The chunk past the loop end in `AfterCursorOutsideLoop`
        // should not.
        let inside_loop_and_required: HashSet<ChunkId> =
            chunks[..5].iter().map(|c| c.id()).collect();
        assert!(inside_loop_and_required.is_subset(&requested));
        assert!(!requested.contains(&chunks[5].id()));
    }

    /// An active recording misses a required chunk that uses up the on-wire budget. In a stage
    /// above the preview stage it has a loaded chunk ahead of the time cursor, and a chunk in
    /// transit after that. A preview recording has loaded chunks within the preview stage. The
    /// memory budget doesn't fit all loaded chunks, so the preview's chunks stay protected and
    /// the active recording's chunk ahead of the time cursor doesn't.
    #[test]
    fn preview_stage_is_protected_before_later_active_stages() {
        let tl = Timeline::new_sequence("frame");

        // All three chunks are on the same entity, so a missing chunk makes the other two part
        // of a `Similar` stage. The loaded chunk and the chunk in transit are above
        // `FetchStage::MAX_PREVIEW`, and the loaded chunk comes first in the priority order.
        let missing_chunk = build_temporal_chunk("/entity", tl, 100);
        let ahead_chunk = build_temporal_chunk("/entity", tl, 2000);
        let in_transit_chunk = build_temporal_chunk("/entity", tl, 2500);
        let (mut active_store, mut active_index) = setup_test_recording(&[
            Arc::clone(&missing_chunk),
            Arc::clone(&ahead_chunk),
            Arc::clone(&in_transit_chunk),
        ]);
        let events = active_store.insert_chunk(&ahead_chunk).unwrap();
        active_index.on_events(&active_store, &events);

        let options = ChunkPrefetchOptions::default();
        let time_cursor = time_cursor_at_zero(tl);
        let (load, requested) = recording_load_fn();

        // Room for a single chunk on the wire.
        active_store.report_missing_virtual_chunk_id(in_transit_chunk.id());
        active_index
            .prefetch_chunks(
                &active_store,
                &options,
                Some(time_cursor),
                &mut RemainingByteBudget::new(u64::MAX, 1),
                &load,
            )
            .unwrap();
        assert_eq!(*requested.lock(), vec![in_transit_chunk.id()]);
        requested.lock().clear();

        active_store.report_missing_virtual_chunk_id(missing_chunk.id());

        // Static chunks are in the `Indicated` stage, below `FetchStage::MAX_PREVIEW`.
        // Different entity paths keep the chunks from getting merged.
        let preview_chunks: Vec<Arc<Chunk>> = (0..2)
            .map(|i| build_static_chunk(&format!("/static_{i}")))
            .collect();
        let (mut preview_store, mut preview_index) = setup_test_recording(&preview_chunks);
        for chunk in &preview_chunks {
            let events = preview_store.insert_chunk(chunk).unwrap();
            preview_index.on_events(&preview_store, &events);
        }

        let manifest = active_index.manifest().unwrap();
        let missing_row = manifest
            .col_chunk_ids()
            .iter()
            .position(|id| *id == missing_chunk.id())
            .expect("missing chunk should be in the manifest");
        let missing_chunk_bytes = manifest.col_chunk_byte_size_uncompressed()[missing_row];
        let ahead_chunk_bytes: u64 = active_store
            .iter_physical_chunks()
            .map(|c| Chunk::total_size_bytes(c.as_ref()))
            .sum();
        let preview_bytes: u64 = preview_store
            .iter_physical_chunks()
            .map(|c| Chunk::total_size_bytes(c.as_ref()))
            .sum();
        assert!(
            ahead_chunk_bytes <= preview_bytes,
            "the memory budget should fit the missing chunk and the chunk ahead of the time cursor"
        );

        // Room for a single chunk on the wire. The memory budget fits the missing chunk and
        // the preview's chunks with one byte to spare.
        let mut budget = RemainingByteBudget::new(missing_chunk_bytes + preview_bytes + 1, 1);

        let mut active = active_index
            .prepare_chunk_fetcher(&active_store, &options, Some(time_cursor), &mut budget)
            .unwrap();
        let mut preview = preview_index
            .prepare_chunk_fetcher(&preview_store, &options, None, &mut budget)
            .unwrap();

        ChunkFetcher::fetch_for_recordings(
            &mut [&mut active],
            &mut [&mut preview],
            &mut [],
            &mut budget,
            options.max_fetch_stage,
        );

        assert_eq!(
            finish_and_take_requested([active, preview]),
            vec![missing_chunk.id()],
            "only the missing chunk should be requested"
        );

        let expected_preview: ahash::HashSet<ChunkId> =
            preview_chunks.iter().map(|c| c.id()).collect();
        assert_eq!(
            preview_index
                .chunk_prioritizer()
                .protected_chunks()
                .physical,
            expected_preview,
            "every preview chunk should be protected"
        );
        assert_eq!(
            active_index.chunk_prioritizer().protected_chunks().physical,
            ahash::HashSet::default(),
            "the active recording's chunk ahead of the time cursor should not be protected"
        );
    }
}
