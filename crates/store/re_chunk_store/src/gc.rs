use std::collections::BTreeSet;
use std::collections::btree_map::Entry as BTreeMapEntry;
use std::collections::hash_map::Entry as HashMapEntry;
use std::time::Duration;

use ahash::{HashMap, HashSet};
use nohash_hasher::IntMap;
use web_time::Instant;

use re_byte_size::SizeBytes;
use re_chunk::{Chunk, ChunkId, ComponentIdentifier, TimelineName};
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt};

// Used all over in docstrings.
#[expect(unused_imports)]
use crate::RowId;
use crate::store::ChunkIdSetPerTime;
use crate::{
    ChunkStore, ChunkStoreChunkStats, ChunkStoreDiff, ChunkStoreDiffKind, ChunkStoreEvent,
    ChunkStoreStats,
};

// ---

// TODO: are we effectively just... dropping the mark phase entirely? I think that's really what
// we're doing here, right?

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
    pub protected_time_ranges: IntMap<TimelineName, AbsoluteTimeRange>,

    /// Remove chunks giving priority to those that are the furthest away from this timestamp.
    //
    // TODO: a few words regarding how this behaves with `protect_latest` & `protected_time_ranges`
    // would be nice.
    pub furthest_from: Option<(TimelineName, TimeInt)>,
}

impl GarbageCollectionOptions {
    pub fn gc_everything() -> Self {
        Self {
            target: GarbageCollectionTarget::Everything,
            time_budget: std::time::Duration::MAX,
            protect_latest: 0,
            protected_time_ranges: Default::default(),
            furthest_from: None,
        }
    }

    /// If true, we cannot remove this chunk.
    pub fn is_chunk_temporally_protected(&self, chunk: &Chunk) -> bool {
        for (timeline, protected_time_range) in &self.protected_time_ranges {
            if let Some(time_column) = chunk.timelines().get(timeline)
                && time_column.time_range().intersects(*protected_time_range)
            {
                return true;
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

pub type RemovableChunkIdPerTimePerComponentPerTimelinePerEntity = IntMap<
    EntityPath,
    IntMap<TimelineName, IntMap<ComponentIdentifier, HashMap<TimeInt, Vec<ChunkId>>>>,
>;

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
    //
    // TODO: maybe we should give a lifting to the docs above.
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

        // TODO: this definitely does not make sense when using distance-based GC, but I guess
        // that's taken care of at the call site?
        // -> actually it's fine to allow us either way, but again this should also be taken into
        // account by the row-id driven GC.
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
    //
    // TODO: this probably needs a bit more information wrt how this behaves with virtual chunks.
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

        let mut chunk_ids_dangling = HashSet::default();
        let mut chunk_ids_to_be_removed =
            RemovableChunkIdPerTimePerComponentPerTimelinePerEntity::default();

        let start_time = Instant::now();
        {
            re_tracing::profile_scope!("mark");

            // These chunks cannot be dangling by definition, since we need to access their data in
            // order to sort them in the first place.
            //
            // TODO(cmc): we would very much like that to be iterative or at least paginated in
            // some way, so that it doesn't eat away all of the mark phase's time budget for no
            // reason, but that requires making things much more complicated, so let's see how far
            // we get with a simple "sort and collect everything" approach first.
            //
            // TODO: this comment still has its place, although the "dangling" terminology will not
            // make sense anymore.
            let chunks_furthest_away = if let Some((timeline, time)) =
                options.furthest_from.as_ref()
            {
                let chunks = self.find_temporal_chunks_furthest_from(timeline, *time);

                // This will only apply for tests run from this crate's src/ directory, which is good
                // enough for our purposes.
                if cfg!(test) {
                    let chunks_slow = self.find_temporal_chunks_furthest_from_slow(timeline, *time);
                    assert_eq!(chunks_slow, chunks);
                }

                chunks
            } else {
                vec![]
            };

            let chunks_in_min_row_id_order =
                self.chunk_ids_per_min_row_id
                    .iter()
                    .filter_map(|(_, chunk_id)| {
                        if let Some(chunk) = self.chunks_per_chunk_id.get(chunk_id) {
                            Some(chunk.clone())
                        } else {
                            // TODO: right, dangling in that sense.
                            // -> yeah that goes away entirely.
                            chunk_ids_dangling.insert(*chunk_id);
                            None
                        }
                    });

            let chunks_in_priority_order = chunks_furthest_away
                .into_iter()
                .chain(chunks_in_min_row_id_order);

            for chunk in
                // TODO: yeah that does not make sense as far as im aware
                // -> this should only affect the row-id driven GC
                chunks_in_priority_order
                    .filter(|chunk| !protected_chunk_ids.contains(&chunk.id()))
            {
                // TODO: it seems weird to me that this is somehow still a thing? surely it should
                // only be a thing in the specific case where we're doing min_row_id-based
                // shenanigans, right?
                if options.is_chunk_temporally_protected(&chunk) {
                    continue;
                }

                // NOTE: Do _NOT_ use `chunk.total_size_bytes` as it is sitting behind an Arc
                // and would count as amortized (i.e. 0 bytes).
                num_bytes_to_drop -= <Chunk as SizeBytes>::total_size_bytes(&*chunk) as f64;

                // NOTE: We cannot blindly `retain` across all temporal tables, it's way too costly
                // and slow. Rather we need to surgically remove the superfluous chunks.
                //
                // TODO: well that's simply not true anymore, right?
                let entity_path = chunk.entity_path();
                let per_timeline = chunk_ids_to_be_removed
                    .entry(entity_path.clone())
                    .or_default();
                for (&timeline, time_column) in chunk.timelines() {
                    let per_component = per_timeline.entry(timeline).or_default();
                    for component in chunk.components_identifiers() {
                        let per_time = per_component.entry(component).or_default();

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

                // NOTE: There is no point in spending more than a fourth of the time budget on the
                // mark phase or there is no way the sweep phase will have any time to do anything
                // with the results anyhow.
                //
                // TODO: Interestingly, the sweep phase is gonna become much, much cheaper if we
                // stop touching all the indices.
                // -> so much cheaper in fact that the split between mark and sweep might not even
                // make sense anymore?
                if start_time.elapsed() >= options.time_budget / 4 || num_bytes_to_drop <= 0.0 {
                    break;
                }
            }
        }

        let Self {
            id: _,
            config: _,
            time_type_registry: _,  // purely additive
            type_registry: _,       // purely additive
            per_column_metadata: _, // purely additive only
            chunks_per_chunk_id,
            chunk_ids_per_min_row_id,
            temporal_chunk_ids_per_entity_per_component,
            temporal_chunk_ids_per_entity,
            temporal_chunks_stats: _,
            static_chunk_ids_per_entity: _, // we don't GC static data
            static_chunks_stats: _,         // we don't GC static data
            insert_id: _,
            gc_id: _,
            event_id: _,
        } = self;

        {
            re_tracing::profile_scope!("dangling");

            let mut diffs = Vec::new();

            // TODO: the entire notion of dangling goes away, obviously.
            // -> or does it? what did dangling exactly mean in this context? i.e. dangling in
            //    which _direction_?
            //
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

                chunk_ids_per_min_row_id
                    .retain(|_row_id, chunk_id| !chunk_ids_dangling.contains(chunk_id));

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
                        //
                        // TODO: some version of that comment still applies, but this piece of code
                        // will disappear.
                        // -> Document this on the field itself if that's not already the case.

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
                            //
                            // TODO: ditto

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
                re_tracing::profile_scope!("sweep");

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
    //
    // TODO: pretty sure this all mark phase and therefore disappears entirely.
    pub(crate) fn remove_chunk(&mut self, chunk_id: ChunkId) -> Vec<ChunkStoreDiff> {
        re_tracing::profile_function!();

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

                for (component, time_range) in time_range_per_component {
                    let chunk_ids_to_be_removed =
                        chunk_ids_to_be_removed.entry(component).or_default();

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

        self.remove_chunks(chunk_ids_to_be_removed, None)
    }

    /// Surgically removes a set of _temporal_ [`ChunkId`]s from all indices.
    ///
    /// This is orders of magnitude faster than trying to `retain()` on all our internal indices,
    /// when you already know where these chunks live.
    ///
    /// See also [`ChunkStore::remove_chunk`].
    //
    // TODO: pretty sure this all mark phase and therefore disappears entirely.
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
            re_tracing::profile_scope!("chunk-id");

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
                re_tracing::profile_scope!("timeline");
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
                    //
                    // TODO: ditto

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

                        if let Some((start_time, time_budget)) = time_budget
                            && start_time.elapsed() >= time_budget
                        {
                            break;
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

                for (component_descr, chunk_ids_to_be_removed) in chunk_ids_to_be_removed {
                    let HashMapEntry::Occupied(mut temporal_chunk_ids_per_time) =
                        temporal_chunk_ids_per_component
                            .get_mut()
                            .entry(component_descr)
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
                    //
                    // TODO: ditto

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

        // TODO: we still need that guy too, otherwise the row-id driven GC will be non-sensical...
        // unless we just add a check for physicality in the mark loop instead, which seems like a
        // much better alternative indeed.
        let min_row_ids_removed = chunk_ids_removed.iter().filter_map(|chunk_id| {
            let chunk = self.chunks_per_chunk_id.get(chunk_id)?;
            chunk.row_id_range().map(|(min, _)| min)
        });
        for row_id in min_row_ids_removed {
            if self.chunk_ids_per_min_row_id.remove(&row_id).is_none() {
                re_log::warn!(
                    %row_id,
                    "Row ID marked for removal was not found, there's bug in the Chunk Store"
                );
            }
        }

        // TODO: literally the only thing left, right?
        {
            re_tracing::profile_scope!("last collect");
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
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk::TimePoint;
    use re_log_types::{StoreId, Timeline, TimelineName};
    use re_sdk_types::{RowId, archetypes};

    use crate::{Chunk, ChunkStore, ChunkStoreConfig, GarbageCollectionOptions};

    use super::*;

    #[test]
    fn gc_furthest_from() {
        const NUM_CHUNKS: i64 = 10_000;
        const NUM_ROWS_PER_CHUNK: i64 = 1_000;

        fn setup_store() -> ChunkStore {
            let store_id = StoreId::random(re_log_types::StoreKind::Recording, "test_app");
            let mut store = ChunkStore::new(store_id, ChunkStoreConfig::ALL_DISABLED);

            for i in 0..NUM_CHUNKS {
                let timepoint = (i * NUM_ROWS_PER_CHUNK
                    ..i * NUM_ROWS_PER_CHUNK + NUM_ROWS_PER_CHUNK)
                    .map(|t| (Timeline::log_tick(), t))
                    .collect::<TimePoint>();
                let p = i as f64;
                let chunk = Chunk::builder("my_entity")
                    .with_archetype(
                        RowId::new(),
                        timepoint,
                        &archetypes::Points3D::new([[p, p, p]]),
                    )
                    .build()
                    .unwrap();
                store.insert_chunk(&Arc::new(chunk)).unwrap();
            }

            store
        }

        // The implementation performs some extra assertions for correctness when running in cfg(test).
        for pivot in [0, NUM_CHUNKS / 2, NUM_CHUNKS] {
            let mut store = setup_store();

            assert_eq!(NUM_CHUNKS as usize, store.num_chunks());
            for _ in 0..3 {
                // Call `store.gc()` more than once just to make sure nothing weird happens with
                // all the shadow indices left by the first call.
                store.gc(&GarbageCollectionOptions {
                    furthest_from: Some((TimelineName::log_tick(), TimeInt::new_temporal(pivot))),
                    ..GarbageCollectionOptions::gc_everything()
                });
            }
            assert_eq!(0, store.num_chunks());
        }
    }
}
