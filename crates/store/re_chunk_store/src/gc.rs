use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use nohash_hasher::IntMap;
use web_time::Instant;

use re_byte_size::SizeBytes;
use re_chunk::{Chunk, ChunkId, TimelineName};
use re_log_types::{AbsoluteTimeRange, TimeInt};

use crate::{
    ChunkStore, ChunkStoreChunkStats, ChunkStoreDiff, ChunkStoreDiffKind, ChunkStoreEvent,
    ChunkStoreStats,
};

// Used all over in docstrings.
#[expect(unused_imports)]
use crate::RowId;

// ---

#[derive(Debug, Clone, Copy)]
pub enum GarbageCollectionTarget {
    /// Try to drop _at least_ the given fraction.
    ///
    /// The fraction must be a float in the range [0.0 : 1.0].
    DropAtLeastFraction(f64),

    /// GC Everything that isn't [protected](GarbageCollectionOptions::protect_latest).
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
    ///
    /// This is ignored when using [`GarbageCollectionOptions::furthest_from`], unless the GC falls
    /// back to row ID based collection.
    pub protect_latest: usize,

    /// Do not remove any data within these time ranges.
    ///
    /// This is ignored when using [`GarbageCollectionOptions::furthest_from`], unless the GC falls
    /// back to row ID based collection.
    pub protected_time_ranges: IntMap<TimelineName, AbsoluteTimeRange>,

    /// Remove chunks giving priority to those that are the furthest away from this timestamp.
    ///
    /// This ignores [`protect_latest`] as well as [`protected_time_ranges`], unless the GC falls
    /// back to row ID based collection.
    ///
    /// [`protect_latest`]: `GarbageCollectionOptions::protect_latest`
    /// [`protected_time_ranges`]: `GarbageCollectionOptions::protected_time_ranges`
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

impl ChunkStore {
    /// Triggers a garbage collection according to the desired `target`.
    ///
    /// Returns the list of physical `Chunk`s that were purged from the store in the form
    /// of [`ChunkStoreEvent`]s.
    ///
    /// ## Semantics
    ///
    /// Garbage collection works on a chunk-level basis, giving priority to those that are the
    /// furthest away from the timestamp specified in [`GarbageCollectionOptions::furthest_from`].
    ///
    /// If no timestamp is specified, or if not enough data could be collected during the
    /// timestamp-driven pass, then garbage collection falls back to [`RowId`] order
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

                self.gc_drop_at_least_num_bytes(options, num_bytes_to_drop)
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

                self.gc_drop_at_least_num_bytes(options, f64::INFINITY)
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

    /// For each `EntityPath`, `Timeline`, `Component` find the N latest *physical* [`ChunkId`]s.
    ///
    /// This only accounts for physical/loaded chunks, and therefore will work properly even in the
    /// context of a store that has offloaded some chunks at the end of its range.
    //
    // TODO(jleibs): More complex functionality might required expanding this to also
    // *ignore* specific entities, components, timelines, etc. for this protection.
    fn find_all_protected_physical_chunk_ids(&self, target_count: usize) -> BTreeSet<ChunkId> {
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
                                    .values()
                                    .rev()
                                    .flatten()
                                    .copied()
                                    .chain(
                                        temporal_chunk_ids_per_time
                                            .per_end_time
                                            .values()
                                            .rev()
                                            .flatten()
                                            .copied(),
                                    )
                                    .filter(|chunk_id| {
                                        self.chunks_per_chunk_id.contains_key(chunk_id)
                                    })
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
    ) -> Vec<ChunkStoreDiff> {
        re_tracing::profile_function!(re_format::format_bytes(num_bytes_to_drop));

        let start_time = Instant::now();
        let chunks_to_be_removed = {
            re_tracing::profile_scope!("mark");

            // These are all physical/loaded chunks by definition, since we need to access their data in
            // order to sort them in the first place.
            //
            // TODO(cmc): we would very much like that to be iterative or at least paginated in
            // some way, so that it doesn't eat away all of the mark phase's time budget for no
            // reason, but that requires making things much more complicated, so let's see how far
            // we get with a simple "sort and collect everything" approach first.
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

            let chunks_in_min_row_id_order = {
                // NOTE: latest-at protection only applies for RowID-based collection
                let protected_chunk_ids =
                    self.find_all_protected_physical_chunk_ids(options.protect_latest);

                self.chunk_ids_per_min_row_id
                    .values()
                    .filter(move |chunk_id| !protected_chunk_ids.contains(chunk_id))
                    .filter_map(|chunk_id| self.chunks_per_chunk_id.get(chunk_id).cloned()) // physical only
                    .filter(|chunk| !chunk.is_static()) // cannot gc static data
            };

            let chunks_in_priority_order = chunks_furthest_away
                .into_iter()
                .chain(chunks_in_min_row_id_order)
                .filter(|chunk| !options.is_chunk_temporally_protected(chunk));

            let mut chunks_to_be_removed = Vec::new();
            for chunk in chunks_in_priority_order {
                // NOTE: Do _NOT_ use `chunk.total_size_bytes` as it is sitting behind an Arc
                // and would count as amortized (i.e. 0 bytes).
                num_bytes_to_drop -= <Chunk as SizeBytes>::total_size_bytes(&*chunk) as f64;

                // We divide the time budget equally between the mark and sweep phases.
                if start_time.elapsed() >= options.time_budget / 2 || num_bytes_to_drop <= 0.0 {
                    break;
                }

                chunks_to_be_removed.push(chunk);
            }

            chunks_to_be_removed
        };

        {
            re_tracing::profile_scope!("sweep");
            self.remove_chunks(
                chunks_to_be_removed,
                Some((start_time, options.time_budget)),
            )
        }
    }

    /// Surgically removes a set of _temporal_ [`ChunkId`]s from all indices.
    ///
    /// This is orders of magnitude faster than trying to `retain()` on all our internal indices,
    /// when you already know where these chunks live.
    ///
    /// See also [`ChunkStore::remove_chunk`].
    pub(crate) fn remove_chunks(
        &mut self,
        chunks_to_be_removed: Vec<Arc<Chunk>>,
        time_budget: Option<(Instant, Duration)>,
    ) -> Vec<ChunkStoreDiff> {
        re_tracing::profile_function!();

        let Self {
            id: _,
            config: _,
            time_type_registry: _,  // purely additive
            type_registry: _,       // purely additive
            per_column_metadata: _, // purely additive only
            chunks_per_chunk_id,
            chunk_ids_per_min_row_id,
            temporal_chunk_ids_per_entity_per_component: _, // purely additive: virtual index
            temporal_chunk_ids_per_entity: _,               // purely additive: virtual index
            temporal_physical_chunks_stats,
            static_chunk_ids_per_entity: _, // we don't GC static data
            static_chunks_stats: _,         // we don't GC static data
            insert_id: _,
            gc_id: _,
            event_id: _,
        } = self;

        let mut diffs = Vec::with_capacity(chunks_to_be_removed.len());
        for chunk in chunks_to_be_removed {
            if let Some((start_time, time_budget)) = time_budget
                && start_time.elapsed() >= time_budget
            {
                break;
            }

            if let Some(row_id_min) = chunk.row_id_range().map(|(min, _)| min) {
                chunk_ids_per_min_row_id.remove(&row_id_min);
            }
            let Some(chunk) = chunks_per_chunk_id.remove(&chunk.id()) else {
                continue;
            };

            // TODO(cmc): Technically, the optimal thing to do would be to recompute
            // `max_interval_length` per time here.
            // In practice, this adds a lot of complexity for likely very little
            // performance benefit, since we expect the chunks to have similar interval
            // lengths on the happy path.

            *temporal_physical_chunks_stats -= ChunkStoreChunkStats::from_chunk(&chunk);

            diffs.push(ChunkStoreDiff::deletion(chunk));
        }

        diffs
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
