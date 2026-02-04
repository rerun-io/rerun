use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;

use ahash::{HashMap, HashSet};
use arrow::array::RecordBatch;
use re_byte_size::SizeBytes as _;
use re_chunk::{ChunkId, TimeInt, Timeline, TimelineName};
use re_chunk_store::{ChunkStore, QueriedChunkIdTracker};
use re_log_encoding::RrdManifest;
use re_log_types::AbsoluteTimeRange;

use crate::{
    chunk_promise::{BatchInfo, ChunkPromise, ChunkPromiseBatch, ChunkPromises},
    rrd_manifest_index::{ChunkInfo, LoadState},
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
    pub timeline: Timeline,

    /// Start loading chunks from this time onwards,
    /// before looping back to the start.
    pub start_time: TimeInt,

    /// Batch together requests until we reach this size.
    pub max_uncompressed_bytes_per_batch: u64,

    /// Total budget for all loaded chunks.
    pub total_uncompressed_byte_budget: u64,

    /// Maximum number of bytes in transit at once.
    pub max_uncompressed_bytes_in_transit: u64,
}

/// Special chunks for which we need the entire history, not just the latest-at value.
///
/// Right now, this is only used for transform-related chunks.
#[derive(Clone, Default)]
struct HighPrioChunks {
    static_chunks: Vec<ChunkId>,

    /// Sorted by time range min.
    temporal_chunks: BTreeMap<TimelineName, Vec<HighPrioChunk>>,
}

impl HighPrioChunks {
    /// All static chunks, plus all temporal chunks on this timeline before the given time.
    fn all_before(
        &self,
        timeline: TimelineName,
        time: TimeInt,
    ) -> impl Iterator<Item = ChunkId> + '_ {
        self.static_chunks.iter().copied().chain(
            self.temporal_chunks
                .get(&timeline)
                .into_iter()
                .flat_map(|chunks| chunks.iter())
                .filter(move |chunk| chunk.time_range.min <= time)
                .map(|chunk| chunk.chunk_id),
        )
    }
}

impl re_byte_size::SizeBytes for HighPrioChunks {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            static_chunks,
            temporal_chunks,
        } = self;

        static_chunks.heap_size_bytes() + temporal_chunks.heap_size_bytes()
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
    bytes: u64,
}

/// Helper struct responsible for batching requests and creating
/// promises for missing chunks.
struct ChunkRequestBatcher<'a> {
    load_chunks: &'a dyn Fn(RecordBatch) -> ChunkPromise,
    manifest: &'a RrdManifest,
    chunk_promises: &'a mut ChunkPromises,
    chunk_byte_size_uncompressed: &'a [u64],
    chunk_byte_size: &'a [u64],
    max_uncompressed_bytes_per_batch: u64,

    remaining_bytes_in_transit_budget: u64,
    current_batch: CurrentBatch,
}

impl<'a> ChunkRequestBatcher<'a> {
    fn new(
        load_chunks: &'a dyn Fn(RecordBatch) -> ChunkPromise,
        manifest: &'a RrdManifest,
        chunk_promises: &'a mut ChunkPromises,
        options: &ChunkPrefetchOptions,
    ) -> Self {
        Self {
            load_chunks,
            chunk_byte_size_uncompressed: manifest.col_chunk_byte_size_uncompressed(),
            chunk_byte_size: manifest.col_chunk_byte_size(),
            manifest,
            max_uncompressed_bytes_per_batch: options.max_uncompressed_bytes_per_batch,

            remaining_bytes_in_transit_budget: options
                .max_uncompressed_bytes_in_transit
                .saturating_sub(chunk_promises.num_uncompressed_bytes_pending()),
            chunk_promises,
            current_batch: Default::default(),
        }
    }

    /// Create promise from the current batch.
    fn finish_batch(&mut self) -> Result<(), PrefetchError> {
        let row_indices: BTreeSet<usize> = self.current_batch.row_indices.iter().copied().collect();

        let col_chunk_ids: &[ChunkId] = self.manifest.col_chunk_ids();

        let mut chunk_ids = BTreeSet::default();
        for &row_idx in &row_indices {
            chunk_ids.insert(col_chunk_ids[row_idx]);
        }

        let rb = re_arrow_util::take_record_batch(
            self.manifest.data(),
            &std::mem::take(&mut self.current_batch.row_indices),
        )?;
        self.chunk_promises.add(ChunkPromiseBatch {
            promise: re_mutex::Mutex::new(Some((self.load_chunks)(rb))),
            info: std::sync::Arc::new(BatchInfo {
                chunk_ids,
                row_indices,
                size_bytes_uncompressed: self.current_batch.uncompressed_bytes,
                size_bytes: self.current_batch.bytes,
            }),
        });
        self.current_batch.bytes = 0;
        self.current_batch.uncompressed_bytes = 0;
        Ok(())
    }

    /// Add a chunk to be fetched.
    ///
    /// If we hit `max_uncompressed_bytes_per_batch` this will create a
    /// [`ChunkPromise`] that includes the given chunk.
    fn try_fetch(
        &mut self,
        chunk_row_idx: usize,
        remote_chunk: &mut ChunkInfo,
    ) -> Result<bool, PrefetchError> {
        if self.remaining_bytes_in_transit_budget == 0 {
            return Ok(false);
        }

        let uncompressed_chunk_size = self.chunk_byte_size_uncompressed[chunk_row_idx];
        let chunk_byte_size = self.chunk_byte_size[chunk_row_idx];

        self.current_batch.row_indices.push(chunk_row_idx);
        self.current_batch.uncompressed_bytes += uncompressed_chunk_size;
        self.current_batch.bytes += chunk_byte_size;

        remote_chunk.state = LoadState::InTransit;

        if self.max_uncompressed_bytes_per_batch < self.current_batch.uncompressed_bytes {
            self.finish_batch()?;
        }
        self.remaining_bytes_in_transit_budget = self
            .remaining_bytes_in_transit_budget
            .saturating_sub(uncompressed_chunk_size);
        Ok(true)
    }

    /// Fetch a last request batch if one is prepared.
    fn finish(&mut self) -> Result<(), PrefetchError> {
        if !self.current_batch.row_indices.is_empty() {
            self.finish_batch()?;
        }

        Ok(())
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

#[derive(Default)]
#[cfg_attr(feature = "testing", derive(Clone))]
pub struct ChunkPrioritizer {
    /// All physical chunks that are 'in' the memory limit.
    ///
    /// These chunks are protected from being gc'd.
    pub(super) in_limit_chunks: HashSet<ChunkId>,

    /// Tracks whether the viewer was missing any chunks last time we prioritized chunks.
    had_missing_chunks: bool,

    checked_virtual_chunks: HashSet<ChunkId>,

    /// Chunks that are in the progress of being downloaded.
    chunk_promises: ChunkPromises,

    /// Intervals of all root chunks in the rrd manifest per timeline.
    remote_chunk_intervals: BTreeMap<Timeline, SortedRangeMap<TimeInt, ChunkId>>,

    /// All static root chunks in the rrd manifest.
    static_chunk_ids: HashSet<ChunkId>,

    /// Chunks that should be downloaded before any else.
    high_priority_chunks: HighPrioChunks,

    /// Maps a [`ChunkId`] to a specific row in the [`RrdManifest::data`] record batch.
    manifest_row_from_chunk_id: HashMap<ChunkId, usize>,
}

impl re_byte_size::SizeBytes for ChunkPrioritizer {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            in_limit_chunks,
            had_missing_chunks: _,
            checked_virtual_chunks,
            chunk_promises: _, // not yet implemented
            remote_chunk_intervals,
            static_chunk_ids,
            high_priority_chunks,
            manifest_row_from_chunk_id,
        } = self;

        in_limit_chunks.heap_size_bytes()
            + checked_virtual_chunks.heap_size_bytes()
            + remote_chunk_intervals.heap_size_bytes()
            + static_chunk_ids.heap_size_bytes()
            + high_priority_chunks.heap_size_bytes()
            + manifest_row_from_chunk_id.heap_size_bytes()
    }
}

impl ChunkPrioritizer {
    pub fn on_rrd_manifest(
        &mut self,
        manifest: &RrdManifest,
        native_static_map: &re_log_encoding::RrdManifestStaticMap,
        native_temporal_map: &re_log_encoding::RrdManifestTemporalMap,
    ) {
        self.update_static_chunks(native_static_map);
        self.update_chunk_intervals(native_temporal_map);
        self.update_manifest_row_from_chunk_id(manifest);
        self.update_high_priority_chunks(native_static_map, native_temporal_map);
    }

    /// Returns true if the chunk store had missing chunks last time we prioritized chunks.
    pub fn had_missing_chunks(&self) -> bool {
        self.had_missing_chunks
    }

    /// Find all chunk IDs that contain components with the given prefix.
    fn find_chunks_with_component_prefix(
        native_static_map: &re_log_encoding::RrdManifestStaticMap,
        native_temporal_map: &re_log_encoding::RrdManifestTemporalMap,
        prefix: &str,
    ) -> HighPrioChunks {
        let mut static_chunks: HashSet<ChunkId> = Default::default();
        let mut temporal_chunks: BTreeMap<TimelineName, Vec<HighPrioChunk>> = Default::default();

        for components in native_static_map.values() {
            for (component, chunk_id) in components {
                if component.as_str().starts_with(prefix) {
                    static_chunks.insert(*chunk_id);
                }
            }
        }

        for timelines in native_temporal_map.values() {
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

        HighPrioChunks {
            static_chunks: static_chunks.into_iter().collect(),
            temporal_chunks,
        }
    }

    fn update_high_priority_chunks(
        &mut self,
        native_static_map: &re_log_encoding::RrdManifestStaticMap,
        native_temporal_map: &re_log_encoding::RrdManifestTemporalMap,
    ) {
        // Find chunks containing transform-related components.
        // We need to download _all_ of them because any one of them could
        // contain a crucial part of the transform hierarchy.
        // Latest-at fails, because a single entity can define the transform of multiple
        // parts of a hierarchy, and not all of the transform are required to be
        // available at each time point.
        // More here: https://linear.app/rerun/issue/RR-3441/required-transform-frames-arent-always-loaded
        self.high_priority_chunks = Self::find_chunks_with_component_prefix(
            native_static_map,
            native_temporal_map,
            "Transform3D:", // Hard-coding this here is VERY hacky, but I want to ship MVP
        );
    }

    fn update_manifest_row_from_chunk_id(&mut self, manifest: &RrdManifest) {
        self.manifest_row_from_chunk_id.clear();
        let col_chunk_ids = manifest.col_chunk_ids();
        for (row_idx, &chunk_id) in col_chunk_ids.iter().enumerate() {
            self.manifest_row_from_chunk_id.insert(chunk_id, row_idx);
        }
    }

    fn update_static_chunks(&mut self, native_static_map: &re_log_encoding::RrdManifestStaticMap) {
        for entity_chunks in native_static_map.values() {
            for &chunk_id in entity_chunks.values() {
                self.static_chunk_ids.insert(chunk_id);
            }
        }
    }

    fn update_chunk_intervals(
        &mut self,
        native_temporal_map: &re_log_encoding::RrdManifestTemporalMap,
    ) {
        let mut per_timeline_chunks: BTreeMap<Timeline, Vec<(RangeInclusive<TimeInt>, ChunkId)>> =
            BTreeMap::default();

        for timelines in native_temporal_map.values() {
            for (timeline, components) in timelines {
                let timeline_chunks = per_timeline_chunks.entry(*timeline).or_default();
                for chunks in components.values() {
                    for (chunk_id, entry) in chunks {
                        timeline_chunks.push((entry.time_range.into(), *chunk_id));
                    }
                }
            }
        }

        self.remote_chunk_intervals.clear();
        for (timeline, chunks) in per_timeline_chunks {
            self.remote_chunk_intervals
                .insert(timeline, SortedRangeMap::new(chunks));
        }
    }

    pub fn chunk_promises(&self) -> &ChunkPromises {
        &self.chunk_promises
    }

    pub fn chunk_promises_mut(&mut self) -> &mut ChunkPromises {
        &mut self.chunk_promises
    }

    /// The hashset of chunks that should be protected from being
    /// evicted by gc now.
    pub fn protected_chunks(&self) -> &HashSet<ChunkId> {
        &self.in_limit_chunks
    }

    /// An iterator over chunks in priority order.
    ///
    /// See [`Self::prioritize_and_prefetch`] for more details.
    fn chunks_in_priority<'a>(
        static_chunk_ids: &'a HashSet<ChunkId>,
        high_priority_chunks: &'a HighPrioChunks,
        store: &'a ChunkStore,
        used_and_missing: QueriedChunkIdTracker,
        timeline: TimelineName,
        time_cursor: TimeInt,
        chunks: &'a SortedRangeMap<TimeInt, ChunkId>,
    ) -> impl Iterator<Item = ChunkId> + use<'a> {
        let QueriedChunkIdTracker {
            used_physical,
            missing,
        } = used_and_missing;

        let used = used_physical.into_iter();

        let mut missing_roots = Vec::new();

        // Reuse a vec for less allocations in the loop.
        let mut scratch = Vec::new();
        for missing in missing {
            store.collect_root_rrd_manifests(&missing, &mut scratch);
            missing_roots.extend(scratch.drain(..).map(|(id, _)| id));
        }

        let chunks_ids_after_time_cursor = move || {
            chunks
                .query(time_cursor..=TimeInt::MAX)
                .map(|(_, chunk_id)| *chunk_id)
        };
        let chunks_ids_before_time_cursor = move || {
            chunks
                .query(TimeInt::MIN..=time_cursor.saturating_sub(1))
                .map(|(_, chunk_id)| *chunk_id)
        };

        let high_prio_chunks_before_time_cursor =
            high_priority_chunks.all_before(timeline, time_cursor);
        let chunk_ids_in_priority_order = itertools::chain!(
            used,
            missing_roots,
            static_chunk_ids.iter().copied(),
            high_prio_chunks_before_time_cursor,
            std::iter::once_with(chunks_ids_after_time_cursor).flatten(),
            std::iter::once_with(chunks_ids_before_time_cursor).flatten(),
        );
        chunk_ids_in_priority_order
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
    /// We go through these chunks until we hit `options.total_uncompressed_byte_budget`
    /// and prefetch missing chunks until we hit `options.max_uncompressed_bytes_in_transit`.
    pub fn prioritize_and_prefetch(
        &mut self,
        store: &ChunkStore,
        used_and_missing: QueriedChunkIdTracker,
        options: &ChunkPrefetchOptions,
        load_chunks: &dyn Fn(RecordBatch) -> ChunkPromise,
        manifest: &RrdManifest,
        remote_chunks: &mut HashMap<ChunkId, ChunkInfo>,
    ) -> Result<(), PrefetchError> {
        let Some(chunks) = self.remote_chunk_intervals.get(&options.timeline) else {
            return Err(PrefetchError::UnknownTimeline(options.timeline));
        };

        self.had_missing_chunks = !used_and_missing.missing.is_empty();

        let mut remaining_byte_budget = options.total_uncompressed_byte_budget;

        let mut chunk_batcher =
            ChunkRequestBatcher::new(load_chunks, manifest, &mut self.chunk_promises, options);

        if chunk_batcher.remaining_bytes_in_transit_budget == 0 {
            // Early-out: too many bytes already in-transit.
            // But, make sure we don't GC the chunks that were used this frame:
            for chunk_id in used_and_missing.used_physical {
                self.in_limit_chunks.insert(chunk_id);
            }

            return Ok(());
        }

        let chunk_ids_in_priority_order = Self::chunks_in_priority(
            &self.static_chunk_ids,
            &self.high_priority_chunks,
            store,
            used_and_missing,
            *options.timeline.name(),
            options.start_time,
            chunks,
        );

        let entity_paths = manifest.col_chunk_entity_path_raw();

        self.in_limit_chunks.clear();
        self.checked_virtual_chunks.clear();

        // Reuse vecs for less allocations in the loop.
        let mut manifest_chunks_scratch = Vec::new();
        let mut physical_chunks_scratch = Vec::new();

        'outer: for chunk_id in chunk_ids_in_priority_order {
            // If we've already marked this as to be loaded, ignore it.
            if self.in_limit_chunks.contains(&chunk_id)
                || self.checked_virtual_chunks.contains(&chunk_id)
            {
                continue;
            }

            match remote_chunks.get_mut(&chunk_id) {
                Some(
                    remote_chunk @ ChunkInfo {
                        state: LoadState::Unloaded | LoadState::InTransit,
                        ..
                    },
                ) => {
                    let row_idx = self.manifest_row_from_chunk_id[&chunk_id];

                    // We count only the chunks we are interested in as being part of the memory budget.
                    // The others can/will be evicted as needed.
                    let uncompressed_chunk_size =
                        chunk_batcher.chunk_byte_size_uncompressed[row_idx];

                    if options.total_uncompressed_byte_budget < uncompressed_chunk_size {
                        warn_entity_exceeds_memory(entity_paths, row_idx);
                        continue;
                    }

                    {
                        // Can we fit this chunk in memory?
                        remaining_byte_budget =
                            remaining_byte_budget.saturating_sub(uncompressed_chunk_size);
                        if remaining_byte_budget == 0 {
                            break; // We've already loaded too much.
                        }
                    }

                    if remote_chunk.state == LoadState::Unloaded
                        && !chunk_batcher.try_fetch(row_idx, remote_chunk)?
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
                    self.checked_virtual_chunks.insert(chunk_id);
                    store.collect_physical_descendents_of(&chunk_id, &mut physical_chunks_scratch);
                    self.in_limit_chunks
                        .extend(physical_chunks_scratch.drain(..));
                }
                Some(ChunkInfo {
                    state: LoadState::Loaded,
                    ..
                }) => {
                    store.collect_physical_descendents_of(&chunk_id, &mut physical_chunks_scratch);
                    for chunk_id in physical_chunks_scratch.drain(..) {
                        if self.in_limit_chunks.contains(&chunk_id) {
                            continue;
                        }

                        let Some(chunk) = store.physical_chunk(&chunk_id) else {
                            re_log::warn_once!("Couldn't get physical chunk from chunk store");
                            continue;
                        };
                        // Can we still fit this chunk in memory with our new prioritization?
                        remaining_byte_budget =
                            remaining_byte_budget.saturating_sub((**chunk).total_size_bytes());
                        if remaining_byte_budget == 0 {
                            break 'outer; // We've already loaded too much.
                        }
                        self.in_limit_chunks.insert(chunk_id);
                    }

                    self.checked_virtual_chunks.insert(chunk_id);
                }
                None => {
                    // If it's not in the rrd manifest it should be a physical chunk.
                    let Some(chunk) = store.physical_chunk(&chunk_id) else {
                        re_log::warn_once!("Couldn't get physical chunk from chunk store");
                        continue;
                    };

                    // Can we still fit this chunk in memory with our new prioritization?
                    remaining_byte_budget =
                        remaining_byte_budget.saturating_sub((**chunk).total_size_bytes());
                    if remaining_byte_budget == 0 {
                        break 'outer; // We've already loaded too much.
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
                    store.collect_root_rrd_manifests(&chunk_id, &mut manifest_chunks_scratch);
                    self.checked_virtual_chunks
                        .extend(manifest_chunks_scratch.drain(..).map(|(id, _)| id));
                }
            }
        }

        chunk_batcher.finish()?;

        Ok(())
    }
}
