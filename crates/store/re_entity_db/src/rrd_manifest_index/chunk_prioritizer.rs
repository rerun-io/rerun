use std::{collections::BTreeMap, ops::RangeInclusive};

use ahash::{HashMap, HashSet};
use arrow::{
    array::{Int32Array, RecordBatch},
    compute::take_record_batch,
};
use re_byte_size::SizeBytes as _;
use re_chunk::{ChunkId, TimeInt, Timeline};
use re_chunk_store::ChunkStore;
use re_log_encoding::{CodecResult, RrdManifest};

use crate::{
    chunk_promise::{ChunkPromise, ChunkPromiseBatch, ChunkPromises},
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

    #[error("Row index too large: {0}")]
    BadIndex(usize),
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

struct ChunkBatcher<'a> {
    load_chunks: &'a dyn Fn(RecordBatch) -> ChunkPromise,
    manifest: &'a RrdManifest,
    chunk_promises: &'a mut ChunkPromises,
    chunk_byte_size_uncompressed_raw: &'a [u64],
    chunk_byte_size_raw: &'a [u64],
    max_uncompressed_bytes_per_batch: u64,

    remaining_bytes_in_transit_budget: u64,
    uncompressed_bytes_in_batch: u64,
    bytes_in_batch: u64,
    indices: Vec<i32>,
}

impl<'a> ChunkBatcher<'a> {
    fn new(
        load_chunks: &'a dyn Fn(RecordBatch) -> ChunkPromise,
        manifest: &'a RrdManifest,
        chunk_promises: &'a mut ChunkPromises,
        options: &ChunkPrefetchOptions,
    ) -> Result<Self, re_log_encoding::CodecError> {
        Ok(Self {
            load_chunks,
            chunk_byte_size_uncompressed_raw: manifest
                .col_chunk_byte_size_uncompressed_raw()?
                .values(),
            chunk_byte_size_raw: manifest.col_chunk_byte_size_raw()?.values(),
            manifest,
            max_uncompressed_bytes_per_batch: options.max_uncompressed_bytes_per_batch,

            remaining_bytes_in_transit_budget: options
                .max_uncompressed_bytes_in_transit
                .saturating_sub(chunk_promises.num_uncompressed_bytes_pending()),
            chunk_promises,
            uncompressed_bytes_in_batch: 0,
            bytes_in_batch: 0,
            indices: Vec::new(),
        })
    }

    /// Returns (`uncompressed_size`, `byte_size`)
    fn chunk_sizes(&self, row_idx: usize) -> (u64, u64) {
        (
            self.chunk_byte_size_uncompressed_raw[row_idx],
            self.chunk_byte_size_raw[row_idx],
        )
    }

    /// Create promise from the current batch.
    fn batch(&mut self) -> Result<(), PrefetchError> {
        let rb = take_record_batch(
            &self.manifest.data,
            &Int32Array::from(std::mem::take(&mut self.indices)),
        )?;
        self.chunk_promises.add(ChunkPromiseBatch {
            promise: parking_lot::Mutex::new(Some((self.load_chunks)(rb))),
            size_bytes_uncompressed: self.uncompressed_bytes_in_batch,
            size_bytes: self.bytes_in_batch,
        });
        self.uncompressed_bytes_in_batch = 0;
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

        let (uncompressed_chunk_size, chunk_byte_size) = self.chunk_sizes(chunk_row_idx);

        let Ok(row_idx) = i32::try_from(chunk_row_idx) else {
            return Err(PrefetchError::BadIndex(chunk_row_idx)); // Very improbable
        };

        self.indices.push(row_idx);

        self.uncompressed_bytes_in_batch += uncompressed_chunk_size;
        self.bytes_in_batch += chunk_byte_size;

        remote_chunk.state = LoadState::InTransit;

        if self.max_uncompressed_bytes_per_batch < self.uncompressed_bytes_in_batch {
            self.batch()?;
        }
        self.remaining_bytes_in_transit_budget = self
            .remaining_bytes_in_transit_budget
            .saturating_sub(uncompressed_chunk_size);
        Ok(true)
    }

    /// Fetch a last batch if one is prepared.
    fn finish(&mut self) -> Result<(), PrefetchError> {
        if !self.indices.is_empty() {
            self.batch()?;
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

    checked_virtual_chunks: HashSet<ChunkId>,

    /// Chunks that are in the progress of being downloaded.
    chunk_promises: ChunkPromises,

    /// Intervals of all root chunks in the rrd manifest per timeline.
    remote_chunk_intervals: BTreeMap<Timeline, SortedRangeMap<TimeInt, ChunkId>>,

    /// All static root chunks in the rrd manifest.
    static_chunk_ids: HashSet<ChunkId>,

    /// Maps a [`ChunkId`] to a specific row in the [`RrdManifest::data`] record batch.
    manifest_row_from_chunk_id: HashMap<ChunkId, usize>,
}

impl re_byte_size::SizeBytes for ChunkPrioritizer {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            in_limit_chunks,
            checked_virtual_chunks,
            chunk_promises: _, // not yet implemented
            remote_chunk_intervals,
            static_chunk_ids,
            manifest_row_from_chunk_id,
        } = self;

        in_limit_chunks.heap_size_bytes()
            + checked_virtual_chunks.heap_size_bytes()
            + remote_chunk_intervals.heap_size_bytes()
            + static_chunk_ids.heap_size_bytes()
            + manifest_row_from_chunk_id.heap_size_bytes()
    }
}

impl ChunkPrioritizer {
    pub fn on_rrd_manifest(
        &mut self,
        manifest: &RrdManifest,
        native_static_map: &re_log_encoding::RrdManifestStaticMap,
        native_temporal_map: &re_log_encoding::RrdManifestTemporalMap,
    ) -> CodecResult<()> {
        self.update_static_chunks(native_static_map);
        self.update_chunk_intervals(native_temporal_map);
        self.update_manifest_row_from_chunk_id(manifest)?;

        Ok(())
    }

    fn update_manifest_row_from_chunk_id(&mut self, manifest: &RrdManifest) -> CodecResult<()> {
        self.manifest_row_from_chunk_id.clear();
        let chunk_id = manifest.col_chunk_id()?;
        for (row_idx, chunk_id) in chunk_id.enumerate() {
            self.manifest_row_from_chunk_id.insert(chunk_id, row_idx);
        }

        Ok(())
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

    /// Take a hashset of chunks that should be protected from being
    /// evicted by gc now.
    pub fn take_protected_chunks(&mut self) -> HashSet<ChunkId> {
        std::mem::take(&mut self.in_limit_chunks)
    }

    /// An iterator over chunks in priority order.
    ///
    /// See [`Self::prioritize_and_prefetch`] for more details.
    fn chunks_in_priority<'a>(
        static_chunk_ids: &'a HashSet<ChunkId>,
        store: &'a ChunkStore,
        start_time: TimeInt,
        chunks: &'a SortedRangeMap<TimeInt, ChunkId>,
    ) -> impl Iterator<Item = ChunkId> + use<'a> {
        let store_tracked = store.take_tracked_chunk_ids();
        let used = store_tracked.used_physical.into_iter();

        let mut missing_roots = Vec::new();

        // Reuse a vec for less allocations in the loop.
        let mut scratch = Vec::new();
        for missing in store_tracked.missing {
            store.collect_root_rrd_manifests(&missing, &mut scratch);
            missing_roots.extend(scratch.drain(..).map(|(id, _)| id));
        }

        let chunks_ids_after_time_cursor = move || {
            chunks
                .query(start_time..=TimeInt::MAX)
                .map(|(_, chunk_id)| *chunk_id)
        };
        let chunks_ids_before_time_cursor = move || {
            chunks
                .query(TimeInt::MIN..=start_time.saturating_sub(1))
                .map(|(_, chunk_id)| *chunk_id)
        };

        let chunk_ids_in_priority_order = itertools::chain!(
            used,
            missing_roots,
            static_chunk_ids.iter().copied(),
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
        options: &ChunkPrefetchOptions,
        load_chunks: &dyn Fn(RecordBatch) -> ChunkPromise,
        manifest: &RrdManifest,
        remote_chunks: &mut HashMap<ChunkId, ChunkInfo>,
    ) -> Result<(), PrefetchError> {
        let Some(chunks) = self.remote_chunk_intervals.get(&options.timeline) else {
            return Err(PrefetchError::UnknownTimeline(options.timeline));
        };

        let mut remaining_byte_budget = options.total_uncompressed_byte_budget;

        let mut chunk_batcher =
            ChunkBatcher::new(load_chunks, manifest, &mut self.chunk_promises, options)?;

        let chunk_ids_in_priority_order =
            Self::chunks_in_priority(&self.static_chunk_ids, store, options.start_time, chunks);

        let entity_paths = manifest.col_chunk_entity_path_raw()?;

        self.in_limit_chunks.clear();
        self.checked_virtual_chunks.clear();

        // Reuse a vec for less allocations in the loop.
        let mut scratch = Vec::new();

        for chunk_id in chunk_ids_in_priority_order {
            match remote_chunks.get_mut(&chunk_id) {
                Some(
                    remote_chunk @ ChunkInfo {
                        state: LoadState::Unloaded | LoadState::InTransit,
                        ..
                    },
                ) => {
                    // If we've already marked this as to be loaded, ignore it.
                    if self.checked_virtual_chunks.contains(&chunk_id) {
                        continue;
                    }

                    let row_idx = self.manifest_row_from_chunk_id[&chunk_id];

                    // We count only the chunks we are interested in as being part of the memory budget.
                    // The others can/will be evicted as needed.
                    let (uncompressed_chunk_size, _) = chunk_batcher.chunk_sizes(row_idx);

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
                        break;
                    }
                    self.checked_virtual_chunks.insert(chunk_id);
                    self.in_limit_chunks
                        .extend(store.physical_descendents_of(&chunk_id));
                }
                Some(ChunkInfo {
                    state: LoadState::Loaded,
                    ..
                })
                | None => {
                    {
                        if self.in_limit_chunks.contains(&chunk_id) {
                            continue;
                        }

                        let Some(chunk) = store.physical_chunk(&chunk_id) else {
                            re_log::warn_once!("Couldn't get loaded chunk from chunk store");
                            continue;
                        };

                        // Can we still fit this chunk in memory with our new prioritization?
                        remaining_byte_budget =
                            remaining_byte_budget.saturating_sub((**chunk).total_size_bytes());
                        if remaining_byte_budget == 0 {
                            break; // We've already loaded too much.
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
                        store.collect_root_rrd_manifests(&chunk_id, &mut scratch);
                        self.checked_virtual_chunks
                            .extend(scratch.drain(..).map(|(id, _)| id));

                        self.in_limit_chunks.insert(chunk_id);
                    }
                }
            }
        }

        chunk_batcher.finish()?;

        Ok(())
    }
}
