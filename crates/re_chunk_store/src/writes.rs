use std::{collections::BTreeSet, sync::Arc};

use arrow2::array::{Array as _, ListArray as ArrowListArray};
use itertools::Itertools as _;

use re_chunk::{Chunk, EntityPath, RowId};

use crate::{
    store::ChunkIdSetPerTime, ChunkStore, ChunkStoreChunkStats, ChunkStoreDiff, ChunkStoreDiffKind,
    ChunkStoreError, ChunkStoreEvent, ChunkStoreResult,
};

// Used all over in docstrings.
#[allow(unused_imports)]
use crate::ChunkId;

// ---

impl ChunkStore {
    /// Inserts a [`Chunk`] in the store.
    ///
    /// Iff the store was modified, all registered subscribers will be notified and the
    /// resulting [`ChunkStoreEvent`] will be returned, or `None` otherwise.
    ///
    /// * Trying to insert an unsorted chunk ([`Chunk::is_sorted`]) will fail with an error.
    /// * Inserting a duplicated [`ChunkId`] will result in a no-op.
    /// * Inserting an empty [`Chunk`] will result in a no-op.
    pub fn insert_chunk(
        &mut self,
        chunk: &Arc<Chunk>,
    ) -> ChunkStoreResult<Option<ChunkStoreEvent>> {
        if self.chunks_per_chunk_id.contains_key(&chunk.id()) {
            // We assume that chunk IDs are unique, and that reinserting a chunk has no effect.
            re_log::warn_once!(
                "Chunk #{} was inserted more than once (this has no effect)",
                chunk.id()
            );
            return Ok(None);
        }

        if !chunk.is_sorted() {
            return Err(ChunkStoreError::UnsortedChunk);
        }

        let Some(row_id_range) = chunk.row_id_range() else {
            return Ok(None);
        };

        re_tracing::profile_function!(format!("{}", row_id_range.0));

        self.insert_id += 1;

        self.chunks_per_chunk_id.insert(chunk.id(), chunk.clone());
        self.chunk_ids_per_min_row_id
            .entry(row_id_range.0)
            .or_default()
            .push(chunk.id());

        if chunk.is_static() {
            // Static data: make sure to keep the most recent chunk available for each component column.

            let row_id_range_per_component = chunk.row_id_range_per_component();

            for (&component_name, list_array) in chunk.components() {
                let is_empty = list_array
                    .validity()
                    .map_or(false, |validity| validity.is_empty());
                if is_empty {
                    continue;
                }

                let Some((_row_id_min, row_id_max)) =
                    row_id_range_per_component.get(&component_name)
                else {
                    continue;
                };

                self.static_chunk_ids_per_entity
                    .entry(chunk.entity_path().clone())
                    .or_default()
                    .entry(component_name)
                    .and_modify(|cur_chunk_id| {
                        // NOTE: When attempting to overwrite static data, the chunk with the most
                        // recent data within -- according to RowId -- wins.

                        let cur_row_id_max = self.chunks_per_chunk_id.get(cur_chunk_id).map_or(
                            RowId::ZERO,
                            |chunk| {
                                chunk
                                    .row_id_range_per_component()
                                    .get(&component_name)
                                    .map_or(RowId::ZERO, |(_, row_id_max)| *row_id_max)
                            },
                        );
                        if *row_id_max > cur_row_id_max {
                            *cur_chunk_id = chunk.id();
                        }
                    })
                    .or_insert_with(|| chunk.id());
            }

            self.static_chunks_stats += ChunkStoreChunkStats::from_chunk(chunk);
        } else {
            // Temporal data: just index the chunk on every dimension of interest.

            let temporal_chunk_ids_per_timeline = self
                .temporal_chunk_ids_per_entity
                .entry(chunk.entity_path().clone())
                .or_default();

            // NOTE: We must make sure to use the time range of each specific component column
            // here, or we open ourselves to nasty edge cases.
            //
            // See the `latest_at_sparse_component_edge_case` test.
            for (timeline, time_range_per_component) in chunk.time_range_per_component() {
                let temporal_chunk_ids_per_component =
                    temporal_chunk_ids_per_timeline.entry(timeline).or_default();

                for (component_name, time_range) in time_range_per_component {
                    let temporal_chunk_ids_per_time = temporal_chunk_ids_per_component
                        .entry(component_name)
                        .or_default();

                    // See `ChunkIdSetPerTime::max_interval_length`'s documentation.
                    temporal_chunk_ids_per_time.max_interval_length = u64::max(
                        temporal_chunk_ids_per_time.max_interval_length,
                        time_range.abs_length(),
                    );

                    temporal_chunk_ids_per_time
                        .per_start_time
                        .entry(time_range.min())
                        .or_default()
                        .insert(chunk.id());
                    temporal_chunk_ids_per_time
                        .per_end_time
                        .entry(time_range.max())
                        .or_default()
                        .insert(chunk.id());
                }
            }

            self.temporal_chunks_stats += ChunkStoreChunkStats::from_chunk(chunk);
        }

        for (&component_name, list_array) in chunk.components() {
            self.type_registry.insert(
                component_name,
                ArrowListArray::<i32>::get_child_type(list_array.data_type()).clone(),
            );
        }

        let event = ChunkStoreEvent {
            store_id: self.id.clone(),
            store_generation: self.generation(),
            event_id: self
                .event_id
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            diff: ChunkStoreDiff::addition(Arc::clone(chunk)),
        };

        {
            let events = &[event.clone()];

            if cfg!(debug_assertions) {
                let any_event_other_than_addition = events
                    .iter()
                    .any(|e| e.kind != ChunkStoreDiffKind::Addition);
                assert!(!any_event_other_than_addition);
            }

            Self::on_events(events);
        }

        Ok(Some(event))
    }

    /// Unconditionally drops all the data for a given `entity_path`.
    ///
    /// Returns the list of `Chunk`s that were dropped from the store in the form of [`ChunkStoreEvent`]s.
    ///
    /// This is _not_ recursive. The store is unaware of the entity hierarchy.
    pub fn drop_entity_path(&mut self, entity_path: &EntityPath) -> Vec<ChunkStoreEvent> {
        re_tracing::profile_function!(entity_path.to_string());

        self.gc_id += 1; // close enough

        let generation = self.generation();

        let Self {
            id,
            config: _,
            type_registry: _,
            chunks_per_chunk_id,
            chunk_ids_per_min_row_id,
            temporal_chunk_ids_per_entity,
            temporal_chunks_stats,
            static_chunk_ids_per_entity,
            static_chunks_stats,
            insert_id: _,
            query_id: _,
            gc_id: _,
            event_id,
        } = self;

        let dropped_static_chunks = {
            let dropped_static_chunk_ids: BTreeSet<_> = static_chunk_ids_per_entity
                .remove(entity_path)
                .unwrap_or_default()
                .into_values()
                .collect();

            chunk_ids_per_min_row_id.retain(|_row_id, chunk_ids| {
                chunk_ids.retain(|chunk_id| !dropped_static_chunk_ids.contains(chunk_id));
                !chunk_ids.is_empty()
            });

            dropped_static_chunk_ids.into_iter()
        };

        let dropped_temporal_chunks = {
            let dropped_temporal_chunk_ids: BTreeSet<_> = temporal_chunk_ids_per_entity
                .remove(entity_path)
                .unwrap_or_default()
                .into_values()
                .flat_map(|temporal_chunk_ids_per_component| {
                    temporal_chunk_ids_per_component.into_values()
                })
                .flat_map(|temporal_chunk_ids_per_time| {
                    let ChunkIdSetPerTime {
                        max_interval_length: _,
                        per_start_time,
                        per_end_time: _, // same chunk IDs as above
                    } = temporal_chunk_ids_per_time;

                    per_start_time
                        .into_values()
                        .flat_map(|chunk_ids| chunk_ids.into_iter())
                })
                .collect();

            chunk_ids_per_min_row_id.retain(|_row_id, chunk_ids| {
                chunk_ids.retain(|chunk_id| !dropped_temporal_chunk_ids.contains(chunk_id));
                !chunk_ids.is_empty()
            });

            dropped_temporal_chunk_ids.into_iter()
        };

        let dropped_static_chunks = dropped_static_chunks
            .filter_map(|chunk_id| chunks_per_chunk_id.remove(&chunk_id))
            .inspect(|chunk| {
                *static_chunks_stats -= ChunkStoreChunkStats::from_chunk(chunk);
            })
            // NOTE: gotta collect to release the mut ref on `chunks_per_chunk_id`.
            .collect_vec();

        let dropped_temporal_chunks = dropped_temporal_chunks
            .filter_map(|chunk_id| chunks_per_chunk_id.remove(&chunk_id))
            .inspect(|chunk| {
                *temporal_chunks_stats -= ChunkStoreChunkStats::from_chunk(chunk);
            });

        dropped_static_chunks
            .into_iter()
            .chain(dropped_temporal_chunks)
            .map(ChunkStoreDiff::deletion)
            .map(|diff| ChunkStoreEvent {
                store_id: id.clone(),
                store_generation: generation.clone(),
                event_id: event_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                diff,
            })
            .collect()
    }
}
