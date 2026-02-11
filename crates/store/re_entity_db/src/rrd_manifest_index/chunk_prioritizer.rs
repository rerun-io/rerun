use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;

use ahash::{HashMap, HashSet};
use arrow::array::RecordBatch;
use itertools::chain;
use re_byte_size::SizeBytes as _;
use re_chunk::{ChunkId, ComponentIdentifier, TimeInt, Timeline, TimelineName};
use re_chunk_store::{ChunkStore, QueriedChunkIdTracker};
use re_log_encoding::RrdManifest;
use re_log_types::{AbsoluteTimeRange, EntityPathHash, TimelinePoint};
use re_tracing::profile_scope;

use crate::{
    chunk_requests::{ChunkRequests, RequestInfo},
    rrd_manifest_index::{LoadState, RootChunkInfo},
    sorted_range_map::SortedRangeMap,
};

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

    /// Total budget for all loaded chunks.
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
    fn all_before(&self, timeline_point: TimelinePoint) -> impl Iterator<Item = ChunkId> + '_ {
        self.temporal_chunks
            .get(timeline_point.name())
            .into_iter()
            .flat_map(|chunks| chunks.iter())
            .filter(move |chunk| chunk.time_range.min <= timeline_point.time)
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

fn warn_entity_exceeds_memory(
    entity_paths: &arrow::array::GenericByteArray<arrow::datatypes::GenericStringType<i32>>,
    row_idx: usize,
) {
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
}

/// Chunk that we've prioritized in `chunks_in_priority`.
struct PrioritizedChunk {
    /// If this chunk came from `used_physical` or `missing_virtual` it's required
    /// and we log a warning if we can't fit it.
    required: bool,
    chunk_id: ChunkId,
}

impl PrioritizedChunk {
    fn required(chunk_id: ChunkId) -> Self {
        Self {
            required: true,
            chunk_id,
        }
    }

    fn optional(chunk_id: ChunkId) -> Self {
        Self {
            required: false,
            chunk_id,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct ComponentPathKey {
    entity_path: EntityPathHash,
    component: ComponentIdentifier,
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

#[derive(Default)]
#[cfg_attr(feature = "testing", derive(Clone))]
pub struct ChunkPrioritizer {
    /// All root chunks that we have an interest in having loaded,
    /// (or at least a part of them).
    desired_root_chunks: HashSet<ChunkId>,

    /// All physical chunks that are 'in' the memory limit.
    ///
    /// These chunks are protected from being gc'd.
    desired_physical_chunks: HashSet<ChunkId>,

    /// Tracks whether the viewer was missing any chunks last time we prioritized chunks.
    any_missing_chunks: bool,

    /// Chunks that are in the progress of being downloaded.
    chunk_requests: ChunkRequests,

    /// Intervals of all root chunks in the rrd manifest per timeline.
    root_chunk_intervals: BTreeMap<Timeline, SortedRangeMap<TimeInt, ChunkId>>,

    /// All static root chunks in the rrd manifest.
    static_chunk_ids: HashSet<ChunkId>,

    /// Chunks that should be downloaded before any else.
    high_priority_chunks: HighPrioChunks,

    component_paths_from_root_id: HashMap<ChunkId, Vec<ComponentPathKey>>,
}

impl re_byte_size::SizeBytes for ChunkPrioritizer {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            desired_root_chunks,
            desired_physical_chunks,
            any_missing_chunks: _,
            chunk_requests: _, // not yet implemented
            root_chunk_intervals: virtual_chunk_intervals,
            static_chunk_ids,
            high_priority_chunks,
            component_paths_from_root_id,
        } = self;

        desired_root_chunks.heap_size_bytes()
            + desired_physical_chunks.heap_size_bytes()
            + virtual_chunk_intervals.heap_size_bytes()
            + static_chunk_ids.heap_size_bytes()
            + high_priority_chunks.heap_size_bytes()
            + component_paths_from_root_id.heap_size_bytes()
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

    /// Returns true if the chunk store had missing chunks last time we prioritized chunks.
    pub fn any_missing_chunks(&self) -> bool {
        self.any_missing_chunks
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

    /// The hashset of chunks that should be protected from being
    /// evicted by gc now.
    pub fn desired_physical_chunks(&self) -> &HashSet<ChunkId> {
        &self.desired_physical_chunks
    }

    /// An iterator over chunks in priority order.
    ///
    /// See [`Self::prioritize_and_prefetch`] for more details.
    #[expect(clippy::too_many_arguments)] // TODO(emilk): refactor to simplify
    fn chunks_in_priority<'a>(
        components_of_interest: &'a HashSet<ComponentPathKey>,
        component_paths_from_root_id: &'a HashMap<ChunkId, Vec<ComponentPathKey>>,
        static_chunk_ids: &'a HashSet<ChunkId>,
        high_priority_chunks: &'a HighPrioChunks,
        store: &'a ChunkStore,
        used_and_missing: QueriedChunkIdTracker,
        time_cursor: TimelinePoint,
        root_chunks_on_timeline: &'a SortedRangeMap<TimeInt, ChunkId>,
    ) -> impl Iterator<Item = PrioritizedChunk> + use<'a> {
        re_tracing::profile_function!();

        let QueriedChunkIdTracker {
            used_physical,
            missing_virtual,
        } = used_and_missing;

        let used_physical = used_physical.into_iter();

        let mut missing_roots = Vec::new();

        for missing_virtual_chunk_id in missing_virtual {
            store.collect_root_ids(&missing_virtual_chunk_id, &mut missing_roots);
        }

        let chunks_ids_after_time_cursor = move || {
            root_chunks_on_timeline
                .query(time_cursor.time..=TimeInt::MAX)
                .map(|(_, chunk_id)| *chunk_id)
        };
        let chunks_ids_before_time_cursor = move || {
            root_chunks_on_timeline
                .query(TimeInt::MIN..=time_cursor.time.saturating_sub(1))
                .map(|(_, chunk_id)| *chunk_id)
        };

        // Note: we do NOT take `components_of_interest` for high-priority transform chunks,
        // because that seems to cause bugs for unknown reasons.
        let high_prio_chunks_before_time_cursor = high_priority_chunks.all_before(time_cursor);

        // Chunks that are required for the current view.
        let required_chunks = chain!(
            used_physical,
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

            // Extra chunks we try to prefetch, that is unlikely to be needed anytime soon,
            // but if we can we still want to load the whole recording:
            let optional_uninteresting_chunks = chain!(
                std::iter::once_with(chunks_ids_after_time_cursor).flatten(),
                std::iter::once_with(chunks_ids_before_time_cursor).flatten(),
            )
            .filter(is_uninteresting_chunk);

            chain!(optional_interesting_chunks, optional_uninteresting_chunks)
        };

        chain!(
            required_chunks.map(PrioritizedChunk::required),
            optional_chunks.map(PrioritizedChunk::optional),
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
    /// Returns all batches that should be loaded
    #[must_use = "Load the returned batches"]
    pub fn prioritize_and_prefetch(
        &mut self,
        store: &ChunkStore,
        used_and_missing: QueriedChunkIdTracker,
        options: &ChunkPrefetchOptions,
        time_cursor: TimelinePoint,
        manifest: &RrdManifest,
        root_chunks: &HashMap<ChunkId, RootChunkInfo>,
    ) -> Result<Vec<(RecordBatch, RequestInfo)>, PrefetchError> {
        let Some(root_chunks_on_timeline) = self.root_chunk_intervals.get(&time_cursor.timeline())
        else {
            return Err(PrefetchError::UnknownTimeline(time_cursor.timeline()));
        };

        self.any_missing_chunks = !used_and_missing.missing_virtual.is_empty();

        let mut remaining_byte_budget = options.total_uncompressed_byte_budget;

        let mut chunk_batcher = ChunkRequestBatcher::new(manifest, &self.chunk_requests, options);

        if chunk_batcher.remaining_bytes_in_on_wire_budget == 0 {
            // Early-out: too many bytes already in-transit.

            // But, make sure we don't GC the chunks that were used this frame:
            let QueriedChunkIdTracker {
                used_physical,
                missing_virtual: _,
            } = used_and_missing;

            for physical_chunk_id in used_physical {
                for root_id in store.find_root_chunks(&physical_chunk_id) {
                    self.desired_root_chunks.insert(root_id);
                }

                self.desired_physical_chunks.insert(physical_chunk_id);
            }

            return Ok(vec![]);
        }

        // Basically: what components of which entities are currently being viewed by the user?
        let mut components_of_interest: HashSet<ComponentPathKey> = Default::default();
        {
            profile_scope!("components_of_interest");

            let QueriedChunkIdTracker {
                used_physical,
                missing_virtual,
            } = &used_and_missing;

            for physical_chunk_id in used_physical {
                if let Some(chunk) = store.physical_chunk(physical_chunk_id) {
                    for component in chunk.components_identifiers() {
                        components_of_interest.insert(ComponentPathKey {
                            entity_path: chunk.entity_path().hash(),
                            component,
                        });
                    }
                }
            }
            for missing_virtual_chunk_id in missing_virtual {
                for root_id in store.find_root_chunks(missing_virtual_chunk_id) {
                    if let Some(components) = self.component_paths_from_root_id.get(&root_id) {
                        components_of_interest.extend(components.iter().copied());
                    }
                }
            }
        }

        // Mixes virtual and physical chunks (!)
        let chunk_ids_in_priority_order = Self::chunks_in_priority(
            &components_of_interest,
            &self.component_paths_from_root_id,
            &self.static_chunk_ids,
            &self.high_priority_chunks,
            store,
            used_and_missing,
            time_cursor,
            root_chunks_on_timeline,
        );

        let entity_paths = manifest.col_chunk_entity_path_raw();

        // Each branch below updates both these two sets:
        self.desired_root_chunks.clear();
        self.desired_physical_chunks.clear();

        // Reuse vecs for less allocations in the loop.
        let mut root_chunks_scratch = Vec::new();
        let mut physical_chunks_scratch = Vec::new();

        'outer: for PrioritizedChunk { required, chunk_id } in chunk_ids_in_priority_order {
            // chunk_id could be a virtual and/or physical chunk.

            // If we've already marked this as to be loaded, ignore it.
            if self.desired_physical_chunks.contains(&chunk_id)
                || self.desired_root_chunks.contains(&chunk_id)
            {
                continue;
            }

            // Can we still fit this much bytes in memory with our new prioritization?
            let mut try_use_uncompressed_bytes = |bytes| {
                remaining_byte_budget = remaining_byte_budget.saturating_sub(bytes);

                if remaining_byte_budget == 0 {
                    if required {
                        if cfg!(target_arch = "wasm32") {
                            re_log::warn_once!(
                                "Viewing the required data would take more memory than the current budget. Use the native viewer for a higher budget."
                            );
                        } else {
                            re_log::warn_once!(
                                "Viewing the required data would take more memory than the current budget. Increase the memory budget to view this recording."
                            );
                        }
                    }

                    false
                } else {
                    true
                }
            };

            match root_chunks.get(&chunk_id) {
                Some(
                    root_chunk @ RootChunkInfo {
                        state: LoadState::Unloaded | LoadState::InTransit,
                        ..
                    },
                ) => {
                    let row_idx = root_chunk.row_id;

                    // We count only the chunks we are interested in as being part of the memory budget.
                    // The others can/will be evicted as needed.
                    let uncompressed_chunk_size =
                        chunk_batcher.chunk_byte_size_uncompressed[row_idx];

                    if options.total_uncompressed_byte_budget < uncompressed_chunk_size {
                        warn_entity_exceeds_memory(entity_paths, row_idx);
                        continue;
                    }

                    if !try_use_uncompressed_bytes(uncompressed_chunk_size) {
                        break 'outer;
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
                        break 'outer;
                    }

                    self.desired_root_chunks.insert(chunk_id);
                    store.collect_physical_descendents_of(&chunk_id, &mut physical_chunks_scratch);
                    self.desired_physical_chunks
                        .extend(physical_chunks_scratch.drain(..));
                }
                Some(RootChunkInfo {
                    state: LoadState::Loaded,
                    ..
                }) => {
                    self.desired_root_chunks.insert(chunk_id);

                    store.collect_physical_descendents_of(&chunk_id, &mut physical_chunks_scratch);
                    for chunk_id in physical_chunks_scratch.drain(..) {
                        if self.desired_physical_chunks.contains(&chunk_id) {
                            continue;
                        }

                        let Some(chunk) = store.physical_chunk(&chunk_id) else {
                            re_log::warn_once!("Couldn't get physical chunk from chunk store");
                            continue;
                        };

                        if !try_use_uncompressed_bytes((**chunk).total_size_bytes()) {
                            break 'outer;
                        }

                        self.desired_physical_chunks.insert(chunk_id);
                    }
                }
                None => {
                    // If it's not in the rrd manifest it should be a physical chunk.
                    let Some(chunk) = store.physical_chunk(&chunk_id) else {
                        re_log::warn_once!("Couldn't get physical chunk from chunk store");
                        continue;
                    };

                    if !try_use_uncompressed_bytes((**chunk).total_size_bytes()) {
                        break 'outer;
                    }

                    // We want to skip trying to load in chunks from the rrd manifest for
                    // physical chunks.
                    //
                    // Either this is a compaction/root chunk and we already have the whole chunk.
                    // Or this is a split, which we only do for large chunks, which we don't want
                    // download unnecessarily. Especially since we only gc these splits if the
                    // memory budget gets hit.
                    //
                    // If these missing splits are missing we can let the `missing` chunk detection
                    // handle that.
                    store.collect_root_ids(&chunk_id, &mut root_chunks_scratch);
                    self.desired_root_chunks
                        .extend(root_chunks_scratch.drain(..));

                    self.desired_physical_chunks.insert(chunk_id);
                }
            }
        }

        chunk_batcher.finish()
    }

    /// Cancel all fetches of things that are not currently needed.
    #[must_use = "Returns root chunks whose download got cancelled. Mark them as unloaded!"]
    pub fn cancel_outdated_requests(&mut self, egui_now_time: f64) -> Vec<ChunkId> {
        self.chunk_requests
            .cancel_outdated_requests(egui_now_time, &self.desired_root_chunks)
    }
}
