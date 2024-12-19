use std::{
    collections::{btree_map::Entry as BTreeMapEntry, hash_map::Entry as HashMapEntry, BTreeSet},
    time::Duration,
};

use ahash::{HashMap, HashSet};
use nohash_hasher::IntMap;
use re_byte_size::SizeBytes;
use web_time::Instant;

use re_chunk::{Chunk, ChunkId};
use re_log_types::{EntityPath, ResolvedTimeRange, TimeInt, Timeline};
use re_types_core::ComponentName;

use crate::{
    store::ChunkIdSetPerTime, ChunkStore, ChunkStoreChunkStats, ChunkStoreDiff, ChunkStoreDiffKind,
    ChunkStoreEvent, ChunkStoreStats,
};

// Used all over in docstrings.
#[allow(unused_imports)]
use crate::RowId;

// ---

#[derive(Debug, Clone, Copy)]
pub enum GarbageCollectionTarget {
    /// Try to drop _at least_ the given fraction.
    ///
    /// The fraction must be a float in the range [0.0 : 1.0].
    DropAtLeastFraction(f64),

    /// GC Everything that isn't protected.
    Everything,
}

#[derive(Debug, Clone)]
pub struct GarbageCollectionOptions {
    /// What target threshold should the GC try to meet.
    pub target: GarbageCollectionTarget,

    /// How long the garbage collection in allowed to run for.
    ///
    /// Trades off latency for throughput:
    /// - A smaller `time_budget` will clear less data in a shorter amount of time, allowing for a
    ///   more responsive UI at the cost of more GC overhead and more frequent runs.
    /// - A larger `time_budget` will clear more data in a longer amount of time, increasing the
    ///   chance of UI freeze frames but decreasing GC overhead and running less often.
    ///
    /// The default is an unbounded time budget (i.e. throughput only).
    pub time_budget: Duration,

    /// How many component revisions to preserve on each timeline.
    pub protect_latest: usize,

    /// Do not remove any data within these time ranges.
    pub protected_time_ranges: HashMap<Timeline, ResolvedTimeRange>,
}

impl GarbageCollectionOptions {
    pub fn gc_everything() -> Self {
        Self {
            target: GarbageCollectionTarget::Everything,
            time_budget: std::time::Duration::MAX,
            protect_latest: 0,
            protected_time_ranges: Default::default(),
        }
    }

    /// If true, we cannot remove this chunk.
    pub fn is_chunk_protected(&self, chunk: &Chunk) -> bool {
        for (timeline, protected_time_range) in &self.protected_time_ranges {
            if let Some(time_column) = chunk.timelines().get(timeline) {
                if time_column.time_range().intersects(*protected_time_range) {
                    return true;
                }
            }
        }
        false
    }
}

impl std::fmt::Display for GarbageCollectionTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DropAtLeastFraction(p) => {
                write!(f, "DropAtLeast({:.3}%)", *p * 100.0)
            }
            Self::Everything => write!(f, "Everything"),
        }
    }
}

pub type RemovableChunkIdPerTimePerComponentPerTimelinePerEntity =
    IntMap<EntityPath, IntMap<Timeline, IntMap<ComponentName, HashMap<TimeInt, Vec<ChunkId>>>>>;

impl ChunkStore {
    /// Triggers a garbage collection according to the desired `target`.
    ///
    /// Returns the list of `Chunk`s that were purged from the store in the form of [`ChunkStoreEvent`]s.
    ///
    /// ## Semantics
    ///
    /// Garbage collection works on a chunk-level basis and is driven by [`RowId`] order
    /// (specifically, the smallest `RowId` of each respective Chunk), i.e. the order defined
    /// by the clients' wall-clocks, allowing it to drop data across the different timelines in
    /// a fair, deterministic manner.
    /// Similarly, out-of-order data is supported out of the box.
    ///
    /// The garbage collector doesn't deallocate data in and of itself: all it does is drop the
    /// store's internal references to that data (the `Chunk`s), which will be deallocated once
    /// their reference count reaches 0.
    ///
    /// ## Limitations
    ///
    /// The garbage collector has limited support for latest-at semantics. The configuration option:
    /// [`GarbageCollectionOptions::protect_latest`] will protect the N latest values of each
    /// component on each timeline. The only practical guarantee this gives is that a latest-at query
    /// with a value of max-int will be unchanged. However, latest-at queries from other arbitrary
    /// points in time may provide different results pre- and post- GC.
    pub fn gc(
        &mut self,
        options: &GarbageCollectionOptions,
    ) -> (Vec<ChunkStoreEvent>, ChunkStoreStats) {
        re_tracing::profile_function!();

        self.gc_id += 1;

        let stats_before = self.stats();

        let total_size_bytes_before = stats_before.total().total_size_bytes as f64;
        let total_num_chunks_before = stats_before.total().num_chunks;
        let total_num_rows_before = stats_before.total().num_rows;

        let protected_chunk_ids = self.find_all_protected_chunk_ids(options.protect_latest);

        let diffs = match options.target {
            GarbageCollectionTarget::DropAtLeastFraction(p) => {
                assert!((0.0..=1.0).contains(&p));

                let num_bytes_to_drop = total_size_bytes_before * p;
                let target_size_bytes = total_size_bytes_before - num_bytes_to_drop;

                re_log::trace!(
                    kind = "gc",
                    id = self.gc_id,
                    %options.target,
                    total_num_chunks_before = re_format::format_uint(total_num_chunks_before),
                    total_num_rows_before = re_format::format_uint(total_num_rows_before),
                    total_size_bytes_before = re_format::format_bytes(total_size_bytes_before),
                    target_size_bytes = re_format::format_bytes(target_size_bytes),
                    drop_at_least_num_bytes = re_format::format_bytes(num_bytes_to_drop),
                    "starting GC"
                );

                self.gc_drop_at_least_num_bytes(options, num_bytes_to_drop, &protected_chunk_ids)
            }
            GarbageCollectionTarget::Everything => {
                re_log::trace!(
                    kind = "gc",
                    id = self.gc_id,
                    %options.target,
                    total_num_rows_before = re_format::format_uint(total_num_rows_before),
                    total_size_bytes_before = re_format::format_bytes(total_size_bytes_before),
                    "starting GC"
                );

                self.gc_drop_at_least_num_bytes(options, f64::INFINITY, &protected_chunk_ids)
            }
        };

        let stats_after = self.stats();
        let total_size_bytes_after = stats_after.total().total_size_bytes as f64;
        let total_num_chunks_after = stats_after.total().num_chunks;
        let total_num_rows_after = stats_after.total().num_rows;

        re_log::trace!(
            kind = "gc",
            id = self.gc_id,
            %options.target,
            total_num_chunks_before = re_format::format_uint(total_num_chunks_before),
            total_num_rows_before = re_format::format_uint(total_num_rows_before),
            total_size_bytes_before = re_format::format_bytes(total_size_bytes_before),
            total_num_chunks_after = re_format::format_uint(total_num_chunks_after),
            total_num_rows_after = re_format::format_uint(total_num_rows_after),
            total_size_bytes_after = re_format::format_bytes(total_size_bytes_after),
            "GC done"
        );

        let events = if self.config.enable_changelog {
            let events: Vec<_> = diffs
                .into_iter()
                .map(|diff| ChunkStoreEvent {
                    store_id: self.id.clone(),
                    store_generation: self.generation(),
                    event_id: self
                        .event_id
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                    diff,
                })
                .collect();
            {
                if cfg!(debug_assertions) {
                    let any_event_other_than_deletion = events
                        .iter()
                        .any(|e| e.kind != ChunkStoreDiffKind::Deletion);
                    assert!(!any_event_other_than_deletion);
                }

                Self::on_events(&events);
            }

            events
        } else {
            Vec::new()
        };

        (events, stats_before - stats_after)
    }

    /// For each `EntityPath`, `Timeline`, `Component` find the N latest [`ChunkId`]s.
    //
    // TODO(jleibs): More complex functionality might required expanding this to also
    // *ignore* specific entities, components, timelines, etc. for this protection.
    fn find_all_protected_chunk_ids(&self, target_count: usize) -> BTreeSet<ChunkId> {
        re_tracing::profile_function!();

        if target_count == 0 {
            return Default::default();
        }

        self.temporal_chunk_ids_per_entity_per_component
            .values()
            .flat_map(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline.iter().flat_map(
                    |(_timeline, temporal_chunk_ids_per_component)| {
                        temporal_chunk_ids_per_component.iter().flat_map(
                            |(_, temporal_chunk_ids_per_time)| {
                                temporal_chunk_ids_per_time
                                    .per_start_time
                                    .last_key_value()
                                    .map(|(_, chunk_ids)| chunk_ids.iter().copied())
                                    .into_iter()
                                    .flatten()
                                    .chain(
                                        temporal_chunk_ids_per_time
                                            .per_end_time
                                            .last_key_value()
                                            .map(|(_, chunk_ids)| chunk_ids.iter().copied())
                                            .into_iter()
                                            .flatten(),
                                    )
                                    .collect::<BTreeSet<_>>()
                                    .into_iter()
                                    .rev()
                                    .take(target_count)
                            },
                        )
                    },
                )
            })
            .collect()
    }

    fn gc_drop_at_least_num_bytes(
        &mut self,
        options: &GarbageCollectionOptions,
        mut num_bytes_to_drop: f64,
        protected_chunk_ids: &BTreeSet<ChunkId>,
    ) -> Vec<ChunkStoreDiff> {
        re_tracing::profile_function!(re_format::format_bytes(num_bytes_to_drop));

        let mut chunk_ids_to_be_removed =
            RemovableChunkIdPerTimePerComponentPerTimelinePerEntity::default();
        let mut chunk_ids_dangling = HashSet::default();

        let start_time = Instant::now();

        {
            re_tracing::profile_scope!("mark");

            for chunk_id in self
                .chunk_ids_per_min_row_id
                .values()
                .flatten()
                .filter(|chunk_id| !protected_chunk_ids.contains(chunk_id))
            {
                if let Some(chunk) = self.chunks_per_chunk_id.get(chunk_id) {
                    if options.is_chunk_protected(chunk) {
                        continue;
                    }

                    // NOTE: Do _NOT_ use `chunk.total_size_bytes` as it is sitting behind an Arc
                    // and would count as amortized (i.e. 0 bytes).
                    num_bytes_to_drop -= <Chunk as SizeBytes>::total_size_bytes(chunk) as f64;

                    // NOTE: We cannot blindly `retain` across all temporal tables, it's way too costly
                    // and slow. Rather we need to surgically remove the superfluous chunks.
                    let entity_path = chunk.entity_path();
                    let per_timeline = chunk_ids_to_be_removed
                        .entry(entity_path.clone())
                        .or_default();
                    for (&timeline, time_column) in chunk.timelines() {
                        let per_component = per_timeline.entry(timeline).or_default();
                        for component_name in chunk.component_names() {
                            let per_time = per_component.entry(component_name).or_default();

                            // NOTE: As usual, these are vectors of `ChunkId`s, as it is legal to
                            // have perfectly overlapping chunks.
                            let time_range = time_column.time_range();
                            per_time
                                .entry(time_range.min())
                                .or_default()
                                .push(chunk.id());
                            if time_range.min() != time_range.max() {
                                per_time
                                    .entry(time_range.max())
                                    .or_default()
                                    .push(chunk.id());
                            }
                        }
                    }
                } else {
                    chunk_ids_dangling.insert(*chunk_id);
                }

                // NOTE: There is no point in spending more than a fourth of the time budget on the
                // mark phase or there is no way the sweep phase will have any time to do anything
                // with the results anyhow.
                if start_time.elapsed() >= options.time_budget / 4 || num_bytes_to_drop <= 0.0 {
                    break;
                }
            }
        }

        {
            re_tracing::profile_scope!("sweep");

            let Self {
                id: _,
                info: _,
                config: _,
                type_registry: _,
                per_column_metadata: _, // column metadata is additive only
                chunks_per_chunk_id,
                chunk_ids_per_min_row_id,
                temporal_chunk_ids_per_entity_per_component,
                temporal_chunk_ids_per_entity,
                temporal_chunks_stats: _,
                static_chunk_ids_per_entity: _, // we don't GC static data
                static_chunks_stats: _,         // we don't GC static data
                insert_id: _,
                query_id: _,
                gc_id: _,
                event_id: _,
            } = self;

            let mut diffs = Vec::new();

            // NOTE: Dangling chunks should never happen: it is the job of the GC to ensure that.
            //
            // In release builds, we still want to do the nice thing and clean them up as best as we
            // can in order to prevent OOMs.
            //
            // We should really never be in there, so don't bother accounting that in the time
            // budget.
            debug_assert!(
                chunk_ids_dangling.is_empty(),
                "detected dangling chunks -- there's a GC bug"
            );
            if !chunk_ids_dangling.is_empty() {
                re_tracing::profile_scope!("dangling");

                chunk_ids_per_min_row_id.retain(|_row_id, chunk_ids| {
                    chunk_ids.retain(|chunk_id| !chunk_ids_dangling.contains(chunk_id));
                    !chunk_ids.is_empty()
                });

                // Component-less indices
                for temporal_chunk_ids_per_timeline in temporal_chunk_ids_per_entity.values_mut() {
                    for temporal_chunk_ids_per_time in temporal_chunk_ids_per_timeline.values_mut()
                    {
                        let ChunkIdSetPerTime {
                            max_interval_length: _,
                            per_start_time,
                            per_end_time,
                        } = temporal_chunk_ids_per_time;

                        // TODO(cmc): Technically, the optimal thing to do would be to
                        // recompute `max_interval_length` per time here.
                        // In practice, this adds a lot of complexity for likely very little
                        // performance benefit, since we expect the chunks to have similar
                        // interval lengths on the happy path.

                        for chunk_ids in per_start_time.values_mut() {
                            chunk_ids.retain(|chunk_id| !chunk_ids_dangling.contains(chunk_id));
                        }
                        for chunk_ids in per_end_time.values_mut() {
                            chunk_ids.retain(|chunk_id| !chunk_ids_dangling.contains(chunk_id));
                        }
                    }
                }

                // Per-component indices
                for temporal_chunk_ids_per_component in
                    temporal_chunk_ids_per_entity_per_component.values_mut()
                {
                    for temporal_chunk_ids_per_timeline in
                        temporal_chunk_ids_per_component.values_mut()
                    {
                        for temporal_chunk_ids_per_time in
                            temporal_chunk_ids_per_timeline.values_mut()
                        {
                            let ChunkIdSetPerTime {
                                max_interval_length: _,
                                per_start_time,
                                per_end_time,
                            } = temporal_chunk_ids_per_time;

                            // TODO(cmc): Technically, the optimal thing to do would be to
                            // recompute `max_interval_length` per time here.
                            // In practice, this adds a lot of complexity for likely very little
                            // performance benefit, since we expect the chunks to have similar
                            // interval lengths on the happy path.

                            for chunk_ids in per_start_time.values_mut() {
                                chunk_ids.retain(|chunk_id| !chunk_ids_dangling.contains(chunk_id));
                            }
                            for chunk_ids in per_end_time.values_mut() {
                                chunk_ids.retain(|chunk_id| !chunk_ids_dangling.contains(chunk_id));
                            }
                        }
                    }
                }

                diffs.extend(
                    chunk_ids_dangling
                        .into_iter()
                        .filter_map(|chunk_id| chunks_per_chunk_id.remove(&chunk_id))
                        .map(ChunkStoreDiff::deletion),
                );
            }

            if !chunk_ids_to_be_removed.is_empty() {
                diffs.extend(self.remove_chunks(
                    chunk_ids_to_be_removed,
                    Some((start_time, options.time_budget)),
                ));
            }

            diffs
        }
    }

    /// Surgically removes a _temporal_ [`ChunkId`] from all indices.
    ///
    /// This is orders of magnitude faster than trying to `retain()` on all our internal indices.
    ///
    /// See also [`ChunkStore::remove_chunks`].
    pub(crate) fn remove_chunk(&mut self, chunk_id: ChunkId) -> Vec<ChunkStoreDiff> {
        let Some(chunk) = self.chunks_per_chunk_id.get(&chunk_id) else {
            return Vec::new();
        };

        let mut chunk_ids_to_be_removed =
            RemovableChunkIdPerTimePerComponentPerTimelinePerEntity::default();

        {
            let chunk_ids_to_be_removed = chunk_ids_to_be_removed
                .entry(chunk.entity_path().clone())
                .or_default();

            for (timeline, time_range_per_component) in chunk.time_range_per_component() {
                let chunk_ids_to_be_removed = chunk_ids_to_be_removed.entry(timeline).or_default();

                for (component_name, per_desc) in time_range_per_component {
                    for (_component_desc, time_range) in per_desc {
                        let chunk_ids_to_be_removed =
                            chunk_ids_to_be_removed.entry(component_name).or_default();

                        chunk_ids_to_be_removed
                            .entry(time_range.min())
                            .or_default()
                            .push(chunk.id());
                        chunk_ids_to_be_removed
                            .entry(time_range.max())
                            .or_default()
                            .push(chunk.id());
                    }
                }
            }
        }

        self.remove_chunks(chunk_ids_to_be_removed, None)
    }

    /// Surgically removes a set of _temporal_ [`ChunkId`]s from all indices.
    ///
    /// This is orders of magnitude faster than trying to `retain()` on all our internal indices,
    /// when you already know where these chunks live.
    ///
    /// See also [`ChunkStore::remove_chunk`].
    pub(crate) fn remove_chunks(
        &mut self,
        chunk_ids_to_be_removed: RemovableChunkIdPerTimePerComponentPerTimelinePerEntity,
        time_budget: Option<(Instant, Duration)>,
    ) -> Vec<ChunkStoreDiff> {
        re_tracing::profile_function!();

        // NOTE: We cannot blindly `retain` across all temporal tables, it's way too costly
        // and slow. Rather we need to surgically remove the superfluous chunks.

        let mut chunk_ids_removed = HashSet::default();

        // Because we have both a per-component and a component-less index that refer to the same
        // chunks, we must make sure that they get garbage collected in sync.
        // That implies making sure that we don't run out of time budget after we've GC'd one but
        // before we had time to clean the other.

        for (entity_path, chunk_ids_to_be_removed) in chunk_ids_to_be_removed {
            let HashMapEntry::Occupied(mut temporal_chunk_ids_per_timeline) = self
                .temporal_chunk_ids_per_entity_per_component
                .entry(entity_path.clone())
            else {
                continue;
            };

            let HashMapEntry::Occupied(mut temporal_chunk_ids_per_timeline_componentless) =
                self.temporal_chunk_ids_per_entity.entry(entity_path)
            else {
                continue;
            };

            for (timeline, chunk_ids_to_be_removed) in chunk_ids_to_be_removed {
                // Component-less indices
                {
                    let HashMapEntry::Occupied(mut temporal_chunk_ids_per_time_componentless) =
                        temporal_chunk_ids_per_timeline_componentless
                            .get_mut()
                            .entry(timeline)
                    else {
                        continue;
                    };

                    let ChunkIdSetPerTime {
                        max_interval_length: _,
                        per_start_time,
                        per_end_time,
                    } = temporal_chunk_ids_per_time_componentless.get_mut();

                    // TODO(cmc): Technically, the optimal thing to do would be to
                    // recompute `max_interval_length` per time here.
                    // In practice, this adds a lot of complexity for likely very little
                    // performance benefit, since we expect the chunks to have similar
                    // interval lengths on the happy path.

                    for chunk_ids_to_be_removed in chunk_ids_to_be_removed.values() {
                        for (&time, chunk_ids) in chunk_ids_to_be_removed {
                            if let BTreeMapEntry::Occupied(mut chunk_id_set) =
                                per_start_time.entry(time)
                            {
                                for chunk_id in chunk_ids {
                                    chunk_id_set.get_mut().remove(chunk_id);
                                }
                                if chunk_id_set.get().is_empty() {
                                    chunk_id_set.remove_entry();
                                }
                            }

                            if let BTreeMapEntry::Occupied(mut chunk_id_set) =
                                per_end_time.entry(time)
                            {
                                for chunk_id in chunk_ids {
                                    chunk_id_set.get_mut().remove(chunk_id);
                                }
                                if chunk_id_set.get().is_empty() {
                                    chunk_id_set.remove_entry();
                                }
                            }

                            chunk_ids_removed.extend(chunk_ids);
                        }

                        if let Some((start_time, time_budget)) = time_budget {
                            if start_time.elapsed() >= time_budget {
                                break;
                            }
                        }
                    }

                    if per_start_time.is_empty() && per_end_time.is_empty() {
                        temporal_chunk_ids_per_time_componentless.remove_entry();
                    }
                }

                // Per-component indices
                //
                // NOTE: This must go all the way, no matter the time budget left. Otherwise the
                // component-less and per-component indices would go out of sync.

                let HashMapEntry::Occupied(mut temporal_chunk_ids_per_component) =
                    temporal_chunk_ids_per_timeline.get_mut().entry(timeline)
                else {
                    continue;
                };

                for (component_name, chunk_ids_to_be_removed) in chunk_ids_to_be_removed {
                    let HashMapEntry::Occupied(mut temporal_chunk_ids_per_time) =
                        temporal_chunk_ids_per_component
                            .get_mut()
                            .entry(component_name)
                    else {
                        continue;
                    };

                    let ChunkIdSetPerTime {
                        max_interval_length: _,
                        per_start_time,
                        per_end_time,
                    } = temporal_chunk_ids_per_time.get_mut();

                    // TODO(cmc): Technically, the optimal thing to do would be to
                    // recompute `max_interval_length` per time here.
                    // In practice, this adds a lot of complexity for likely very little
                    // performance benefit, since we expect the chunks to have similar
                    // interval lengths on the happy path.

                    for (time, chunk_ids) in chunk_ids_to_be_removed {
                        if let BTreeMapEntry::Occupied(mut chunk_id_set) =
                            per_start_time.entry(time)
                        {
                            for chunk_id in chunk_ids
                                .iter()
                                .filter(|chunk_id| chunk_ids_removed.contains(*chunk_id))
                            {
                                chunk_id_set.get_mut().remove(chunk_id);
                            }
                            if chunk_id_set.get().is_empty() {
                                chunk_id_set.remove_entry();
                            }
                        }

                        if let BTreeMapEntry::Occupied(mut chunk_id_set) = per_end_time.entry(time)
                        {
                            for chunk_id in chunk_ids
                                .iter()
                                .filter(|chunk_id| chunk_ids_removed.contains(*chunk_id))
                            {
                                chunk_id_set.get_mut().remove(chunk_id);
                            }
                            if chunk_id_set.get().is_empty() {
                                chunk_id_set.remove_entry();
                            }
                        }
                    }

                    if per_start_time.is_empty() && per_end_time.is_empty() {
                        temporal_chunk_ids_per_time.remove_entry();
                    }
                }

                if temporal_chunk_ids_per_component.get().is_empty() {
                    temporal_chunk_ids_per_component.remove_entry();
                }
            }

            if temporal_chunk_ids_per_timeline.get().is_empty() {
                temporal_chunk_ids_per_timeline.remove_entry();
            }

            if temporal_chunk_ids_per_timeline_componentless
                .get()
                .is_empty()
            {
                temporal_chunk_ids_per_timeline_componentless.remove_entry();
            }
        }

        self.chunk_ids_per_min_row_id.retain(|_row_id, chunk_ids| {
            chunk_ids.retain(|chunk_id| !chunk_ids_removed.contains(chunk_id));
            !chunk_ids.is_empty()
        });

        chunk_ids_removed
            .into_iter()
            .filter_map(|chunk_id| self.chunks_per_chunk_id.remove(&chunk_id))
            .inspect(|chunk| {
                self.temporal_chunks_stats -= ChunkStoreChunkStats::from_chunk(chunk);
            })
            .map(ChunkStoreDiff::deletion)
            .collect()
    }
}
