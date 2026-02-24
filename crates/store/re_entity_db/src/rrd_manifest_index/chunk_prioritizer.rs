use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;

use ahash::{HashMap, HashSet};
use arrow::array::RecordBatch;
use itertools::chain;
use re_byte_size::SizeBytes as _;
use re_chunk::{Chunk, ChunkId, ComponentIdentifier, TimeInt, Timeline, TimelineName};
use re_chunk_store::{ChunkStore, QueriedChunkIdTracker};
use re_log::debug_assert;
use re_log_encoding::RrdManifest;
use re_log_types::{AbsoluteTimeRange, EntityPathHash, TimelinePoint};

use crate::{
    chunk_requests::{ChunkRequests, RequestInfo},
    rrd_manifest_index::{LoadState, RootChunkInfo},
    sorted_range_map::SortedRangeMap,
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
    /// If true, there are no missing chunks.
    pub all_required_are_loaded: bool,
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
    /// Batch together requests until we reach this size.
    pub max_on_wire_bytes_per_batch: u64,

    /// Total budget for all physical chunks.
    pub total_uncompressed_byte_budget: u64,

    /// Maximum number of bytes in transit at once.
    pub max_bytes_on_wire_at_once: u64,
}

impl Default for ChunkPrefetchOptions {
    fn default() -> Self {
        Self {
            total_uncompressed_byte_budget: u64::MAX,

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

impl HighPrioChunks {
    /// All static chunks, plus all temporal chunks on this timeline before the given time.
    /// With chunks closest to the time cursor ordered first.
    fn all_before(&self, timeline_point: TimelinePoint) -> impl Iterator<Item = ChunkId> + '_ {
        self.temporal_chunks
            .get(timeline_point.name())
            .into_iter()
            .flat_map(move |chunks| {
                let idx =
                    chunks.partition_point(|chunk| chunk.time_range.min <= timeline_point.time);

                // Start loading closest to the time cursor.
                chunks[..idx].iter().rev()
            })
            .map(|chunk| chunk.chunk_id)
    }
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
struct ChunkRequestBatcher<'a> {
    manifest: &'a RrdManifest,
    chunk_byte_size_uncompressed: &'a [u64],
    chunk_byte_size: &'a [u64],
    max_on_wire_bytes_per_batch: u64,

    remaining_bytes_in_on_wire_budget: u64,
    current_batch: CurrentBatch,

    // Output
    to_load: Vec<(RecordBatch, RequestInfo)>,
}

impl<'a> ChunkRequestBatcher<'a> {
    fn new(
        manifest: &'a RrdManifest,
        requests: &ChunkRequests,
        options: &ChunkPrefetchOptions,
    ) -> Self {
        Self {
            chunk_byte_size_uncompressed: manifest.col_chunk_byte_size_uncompressed(),
            chunk_byte_size: manifest.col_chunk_byte_size(),
            manifest,
            max_on_wire_bytes_per_batch: options.max_on_wire_bytes_per_batch,

            remaining_bytes_in_on_wire_budget: options
                .max_bytes_on_wire_at_once
                .saturating_sub(requests.num_on_wire_bytes_pending()),
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
            self.manifest.data(),
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
    fn try_fetch(&mut self, chunk_row_idx: usize) -> Result<bool, PrefetchError> {
        if self.remaining_bytes_in_on_wire_budget == 0 {
            return Ok(false);
        }

        let uncompressed_chunk_size = self.chunk_byte_size_uncompressed[chunk_row_idx];
        let on_wire_byte_size = self.chunk_byte_size[chunk_row_idx];

        self.current_batch.row_indices.push(chunk_row_idx);
        self.current_batch.uncompressed_bytes += uncompressed_chunk_size;
        self.current_batch.on_wire_bytes += on_wire_byte_size;

        if self.max_on_wire_bytes_per_batch <= self.current_batch.on_wire_bytes {
            self.finish_batch()?;
        }
        self.remaining_bytes_in_on_wire_budget = self
            .remaining_bytes_in_on_wire_budget
            .saturating_sub(on_wire_byte_size);
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

struct RemainingByteBudget {
    remaining_bytes: u64,
}

impl RemainingByteBudget {
    /// Try to fit `bytes` into the remaining budget.
    ///
    /// Returns `true` if it fits (even partially), `false` if the budget is exhausted.
    fn try_fit_into_budget(&mut self, bytes: u64, required: bool) -> bool {
        self.remaining_bytes = self.remaining_bytes.saturating_sub(bytes);

        if self.remaining_bytes == 0 {
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
struct PrioritizedRootChunk {
    /// If this chunk came from `used_physical` or `missing_virtual` it's required
    /// and we log a warning if we can't fit it.
    required: bool,

    root_chunk_id: ChunkId,
}

impl PrioritizedRootChunk {
    fn required(root_chunk_id: ChunkId) -> Self {
        Self {
            required: true,
            root_chunk_id,
        }
    }

    fn optional(chunk_id: ChunkId) -> Self {
        Self {
            required: false,
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

    /// Result of the latest call to [`Self::prioritize_and_prefetch`].
    latest_result: Option<PrioritizationState>,

    /// Chunks that are in the progress of being downloaded.
    chunk_requests: ChunkRequests,

    /// Intervals of all root chunks in the rrd manifest per timeline.
    root_chunk_intervals: BTreeMap<Timeline, SortedRangeMap<TimeInt, ChunkId>>,

    /// All static root chunks in the rrd manifest.
    static_chunk_ids: HashSet<ChunkId>,

    /// Chunks that should be downloaded before any else.
    high_priority_chunks: HighPrioChunks,

    pub component_paths_from_root_id: HashMap<ChunkId, Vec<ComponentPathKey>>,

    /// Component paths that were reported either as being used or missing.
    pub components_of_interest: HashSet<ComponentPathKey>,
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
        } = self;

        protected_chunks.heap_size_bytes()
            + virtual_chunk_intervals.heap_size_bytes()
            + static_chunk_ids.heap_size_bytes()
            + high_priority_chunks.heap_size_bytes()
            + component_paths_from_root_id.heap_size_bytes()
            + components_of_interest.heap_size_bytes()
    }
}

impl ChunkPrioritizer {
    pub fn on_rrd_manifest(&mut self, manifest: &RrdManifest) {
        self.update_static_chunks(manifest);
        self.update_chunk_intervals(manifest);
        self.update_high_priority_chunks(manifest);

        self.component_paths_from_root_id.clear();
        for (entity, per_component) in manifest.static_map() {
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

        for (entity, per_timeline) in manifest.temporal_map() {
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

    /// Result of the latest call to [`Self::prioritize_and_prefetch`].
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
        self.high_priority_chunks = Self::find_chunks_with_component_prefix(
            manifest,
            "Transform3D:", // Hard-coding this here is VERY hacky, but I want to ship MVP
        );
    }

    fn update_static_chunks(&mut self, manifest: &RrdManifest) {
        for entity_chunks in manifest.static_map().values() {
            for &chunk_id in entity_chunks.values() {
                self.static_chunk_ids.insert(chunk_id);
            }
        }
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

        self.root_chunk_intervals.clear();
        for (timeline, chunks) in per_timeline_chunks {
            self.root_chunk_intervals
                .insert(timeline, SortedRangeMap::new(chunks));
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

    /// An iterator over root chunks in priority order.
    ///
    /// May return duplicates!
    ///
    /// See [`Self::prioritize_and_prefetch`] for more details.
    #[expect(clippy::too_many_arguments)] // TODO(emilk): refactor to simplify
    fn root_chunks_in_priority<'a>(
        components_of_interest: &'a HashSet<ComponentPathKey>,
        component_paths_from_root_id: &'a HashMap<ChunkId, Vec<ComponentPathKey>>,
        static_chunk_ids: &'a HashSet<ChunkId>,
        high_priority_chunks: &'a HighPrioChunks,
        store: &'a ChunkStore,
        used_and_missing: &QueriedChunkIdTracker,
        time_cursor: Option<TimelinePoint>,
        root_chunks: &'a HashMap<ChunkId, RootChunkInfo>,
        root_chunks_on_timeline: Option<&'a SortedRangeMap<TimeInt, ChunkId>>,
    ) -> impl Iterator<Item = PrioritizedRootChunk> + use<'a> {
        re_tracing::profile_function!();

        let mut missing_roots = Vec::new();
        for missing_virtual_chunk_id in &used_and_missing.missing_virtual {
            store.collect_root_ids(missing_virtual_chunk_id, &mut missing_roots);
        }
        missing_roots.sort();
        missing_roots.dedup();

        let chunks_ids_after_time_cursor = move || {
            time_cursor
                .zip(root_chunks_on_timeline)
                .map(|(time_cursor, root_chunks_on_timeline)| {
                    root_chunks_on_timeline
                        .query(time_cursor.time..=TimeInt::MAX)
                        .map(|(_, chunk_id)| *chunk_id)
                })
                .into_iter()
                .flatten()
        };
        let chunks_ids_before_time_cursor = move || {
            time_cursor
                .zip(root_chunks_on_timeline)
                .map(|(time_cursor, root_chunks_on_timeline)| {
                    root_chunks_on_timeline
                        .query(TimeInt::MIN..=time_cursor.time.saturating_sub(1))
                        .map(|(_, chunk_id)| *chunk_id)
                })
                .into_iter()
                .flatten()
        };

        // Note: we do NOT take `components_of_interest` for high-priority transform chunks,
        // because that seems to cause bugs for unknown reasons.
        let high_prio_chunks_before_time_cursor = time_cursor
            .map(|time_cursor| high_priority_chunks.all_before(time_cursor))
            .into_iter()
            .flatten();

        // Chunks that are required for the current view.
        let required_chunks = chain!(
            missing_roots,
            static_chunk_ids.iter().copied(),
            high_prio_chunks_before_time_cursor,
        );

        // Chunks that aren't currently required. Pure prefetching:
        let optional_chunks = {
            // Chunks for components we are interested in.
            let is_interesting_chunk = |chunk_id: &ChunkId| {
                component_paths_from_root_id[chunk_id]
                    .iter()
                    .any(|path| components_of_interest.contains(path))
            };
            let is_uninteresting_chunk = |chunk_id: &ChunkId| {
                !component_paths_from_root_id[chunk_id]
                    .iter()
                    .any(|path| components_of_interest.contains(path))
            };

            // Extra chunks we try to prefetch, that may _soon_ be needed:
            let optional_interesting_chunks = chain!(
                std::iter::once_with(chunks_ids_after_time_cursor).flatten(),
                std::iter::once_with(chunks_ids_before_time_cursor).flatten(),
            )
            .filter(is_interesting_chunk);

            // Extra chunks at the current time (or after), that the user is not _currently_
            // looking at, but they may switch views.
            let optional_uninteresting_chunks = std::iter::once_with(chunks_ids_after_time_cursor)
                .flatten()
                .filter(is_uninteresting_chunk);

            // Finally: backfill with ALL unloaded chunks.
            // If we have the memory budget for it, we always want to load the full recording:
            let all_chunks = root_chunks.keys().copied();

            chain!(
                optional_interesting_chunks,
                optional_uninteresting_chunks,
                all_chunks,
            )
        };

        chain!(
            required_chunks.map(PrioritizedRootChunk::required),
            optional_chunks.map(PrioritizedRootChunk::optional),
        )
    }

    /// Prioritize which chunk (loaded & unloaded) we want to fit in the
    /// current memory budget. And prefetch some amount of those chunks.
    ///
    /// This prioritizes chunks in the order of:
    /// - Physical chunks that were used since last time this was ran.
    /// - Virtual chunks that would've been hit by queries since last time
    ///   this was ran.
    /// - Static chunks.
    /// - Chunks after the time cursor in rising temporal order.
    /// - Chunks before the time cursor in rising temporal order.
    ///
    /// We go through these chunks until we hit [`ChunkPrefetchOptions::total_uncompressed_byte_budget`]
    /// and prefetch missing chunks until we hit [`ChunkPrefetchOptions::max_bytes_on_wire_at_once`].
    /// Returns all batches that should be loaded.
    #[must_use = "Load the returned batches"]
    pub fn prioritize_and_prefetch(
        &mut self,
        store: &ChunkStore,
        used_and_missing: &QueriedChunkIdTracker,
        options: &ChunkPrefetchOptions,
        time_cursor: Option<TimelinePoint>,
        manifest: &RrdManifest,
        root_chunks: &HashMap<ChunkId, RootChunkInfo>,
    ) -> Result<Vec<(RecordBatch, RequestInfo)>, PrefetchError> {
        re_tracing::profile_function!();

        let mut chunk_batcher = ChunkRequestBatcher::new(manifest, &self.chunk_requests, options);

        if let Some(latest_result) = &mut self.latest_result
            && chunk_batcher.remaining_bytes_in_on_wire_budget == 0
        {
            // Early-out: too many bytes already in-transit.

            if !used_and_missing.missing_virtual.is_empty() {
                latest_result.all_required_are_loaded = false;
            }

            self.protect_used_and_missing(store, used_and_missing);
            return Ok(vec![]);
        }

        self.update_components_of_interest(store, used_and_missing);

        // We will re-calculate these:
        self.protected_chunks.roots.clear();
        self.protected_chunks.physical.clear(); // <- Things we put in here will also be subtracted from remaining_byte_budget

        self.protect_used_and_missing(store, used_and_missing);

        let mut remaining_byte_budget = RemainingByteBudget {
            remaining_bytes: options.total_uncompressed_byte_budget,
        };

        // Start by going through the actually used physical chunks:
        for &physical_chunk_id in &used_and_missing.used_physical {
            debug_assert!(
                self.protected_chunks.physical.contains(&physical_chunk_id),
                "We added it earlier"
            );

            if let Some(chunk) = store.physical_chunk(&physical_chunk_id) {
                let required = true;
                remaining_byte_budget
                    .try_fit_into_budget(Chunk::total_size_bytes(chunk.as_ref()), required);
            } else {
                re_log::debug_warn_once!("Couldn't get physical chunk from chunk store");
            }
        }

        let root_chunks_on_timeline = time_cursor
            .and_then(|time_cursor| self.root_chunk_intervals.get(&time_cursor.timeline()));

        let root_chunk_ids_in_priority_order = Self::root_chunks_in_priority(
            &self.components_of_interest,
            &self.component_paths_from_root_id,
            &self.static_chunk_ids,
            &self.high_priority_chunks,
            store,
            used_and_missing,
            time_cursor,
            root_chunks,
            root_chunks_on_timeline,
        );

        let state = Self::fill_byte_budget(
            &mut self.protected_chunks,
            store,
            options,
            manifest,
            root_chunks,
            &mut chunk_batcher,
            &mut remaining_byte_budget,
            root_chunk_ids_in_priority_order,
        )?;
        self.latest_result = Some(state);

        chunk_batcher.finish()
    }

    #[expect(clippy::too_many_arguments)]
    fn fill_byte_budget(
        protected_chunks: &mut ProtectedChunks,
        store: &ChunkStore,
        options: &ChunkPrefetchOptions,
        manifest: &RrdManifest,
        root_chunks: &HashMap<ChunkId, RootChunkInfo>,
        chunk_batcher: &mut ChunkRequestBatcher<'_>,
        remaining_byte_budget: &mut RemainingByteBudget,
        mut root_chunk_ids_in_priority_order: impl Iterator<Item = PrioritizedRootChunk>,
    ) -> Result<PrioritizationState, PrefetchError> {
        re_tracing::profile_function!();

        let entity_paths = manifest.col_chunk_entity_path_raw();

        let mut visited_root_chunks: HashSet<ChunkId> = Default::default();

        let mut physical_chunks_scratch = Vec::new(); // scratch space to save on reallocations

        let mut state = PrioritizationState {
            transit_budget_filled: false,
            memory_budget_filled: false,
            some_chunks_too_big: false,
            all_required_are_loaded: true,
        };

        for next in root_chunk_ids_in_priority_order.by_ref() {
            let PrioritizedRootChunk {
                required,
                root_chunk_id,
            } = next;

            if !visited_root_chunks.insert(root_chunk_id) {
                continue; // We've already handled this chunk earlier in the priority order.
            }

            let Some(root_chunk) = root_chunks.get(&root_chunk_id) else {
                re_log::debug_warn_once!("Missing root chunk");
                continue;
            };

            store.collect_physical_descendents_of(&root_chunk_id, &mut physical_chunks_scratch);

            match root_chunk.state {
                LoadState::Unloaded | LoadState::InTransit => {
                    if required {
                        state.all_required_are_loaded = false;
                    }

                    let row_idx = root_chunk.row_id;

                    // We count only the chunks we are interested in as being part of the memory budget.
                    // The others can/will be evicted as needed.
                    let uncompressed_chunk_size =
                        chunk_batcher.chunk_byte_size_uncompressed[row_idx];

                    if options.total_uncompressed_byte_budget < uncompressed_chunk_size {
                        warn_entity_exceeds_memory(entity_paths.value(row_idx));
                        state.some_chunks_too_big = true;
                        continue;
                    }

                    if !remaining_byte_budget.try_fit_into_budget(uncompressed_chunk_size, required)
                    {
                        state.memory_budget_filled = true;
                        break;
                    }

                    if root_chunk.state == LoadState::Unloaded
                        && !chunk_batcher.try_fetch(row_idx)?
                    {
                        // If we don't have anything more to fetch we stop looking.
                        //
                        // This isn't entirely correct gc wise. But if we evict chunks
                        // we didn't get to because of this break, we won't be fighting
                        // back and forth with gc since there's some unloaded
                        // chunks inbetween we have to download first. After
                        // which we won't stop prioritizing which chunks should
                        // be in memory here.
                        state.transit_budget_filled = true;
                        break;
                    }

                    protected_chunks.roots.insert(root_chunk_id);
                    protected_chunks
                        .physical
                        .extend(physical_chunks_scratch.drain(..));
                }

                LoadState::FullyLoaded => {
                    protected_chunks.roots.insert(root_chunk_id);

                    for chunk_id in physical_chunks_scratch.drain(..) {
                        if protected_chunks.physical.contains(&chunk_id) {
                            continue; // Already counted as part of our byte budget
                        }

                        let Some(chunk) = store.physical_chunk(&chunk_id) else {
                            re_log::debug_warn_once!(
                                "Couldn't get physical chunk from chunk store"
                            );
                            continue;
                        };

                        let bytes = Chunk::total_size_bytes(chunk.as_ref());
                        if !remaining_byte_budget.try_fit_into_budget(bytes, required) {
                            state.memory_budget_filled = true;
                            break;
                        }

                        protected_chunks.physical.insert(chunk_id);
                    }
                }
            }
        }

        if root_chunk_ids_in_priority_order
            .next()
            .is_some_and(|next| next.required)
        {
            state.all_required_are_loaded = false;
        }

        Ok(state)
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
