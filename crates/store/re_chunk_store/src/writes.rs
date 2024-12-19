use std::{collections::BTreeSet, sync::Arc};

use ahash::HashMap;
use arrow2::array::{Array as _, ListArray as Arrow2ListArray};
use itertools::Itertools as _;

use re_byte_size::SizeBytes;
use re_chunk::{Chunk, EntityPath, RowId};

use crate::{
    store::ChunkIdSetPerTime, ChunkStore, ChunkStoreChunkStats, ChunkStoreConfig, ChunkStoreDiff,
    ChunkStoreError, ChunkStoreEvent, ChunkStoreResult, ColumnMetadataState,
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
    pub fn insert_chunk(&mut self, chunk: &Arc<Chunk>) -> ChunkStoreResult<Vec<ChunkStoreEvent>> {
        if self.chunks_per_chunk_id.contains_key(&chunk.id()) {
            // We assume that chunk IDs are unique, and that reinserting a chunk has no effect.
            re_log::debug_once!(
                "Chunk #{} was inserted more than once (this has no effect)",
                chunk.id()
            );
            return Ok(Vec::new());
        }

        if !chunk.is_sorted() {
            return Err(ChunkStoreError::UnsortedChunk);
        }

        let Some(row_id_range) = chunk.row_id_range() else {
            return Ok(Vec::new());
        };

        re_tracing::profile_function!();

        self.insert_id += 1;

        let mut chunk = Arc::clone(chunk);

        // We're in a transition period during which the Rerun ecosystem is slowly moving over to tagged data.
        //
        // During that time, it is common to end up in situations where the blueprint intermixes both tagged
        // and untagged components, which invariably leads to undefined behavior.
        // To prevent that, we just always hot-patch it to untagged, for now.
        //
        // Examples:
        // * An SDK logs a blueprint (tagged), which is then updated by the viewer (which uses untagged log calls).
        // * Somebody loads an old .rbl from somewhere and starts logging new blueprint data to it.
        // * Etc.
        if self.id.kind == re_log_types::StoreKind::Blueprint {
            let patched = chunk.patched_for_blueprint_021_compat();
            let patched = patched.clone_as_untagged();
            chunk = Arc::new(patched);
        }

        #[cfg(debug_assertions)]
        for (component_name, per_desc) in chunk.components().iter() {
            assert!(
                per_desc.len() <= 1,
                "[DEBUG ONLY] Insert Chunk with multiple values for component named `{component_name}`: this is currently UB\n{chunk}",
            );
        }

        let non_compacted_chunk = Arc::clone(&chunk); // we'll need it to create the store event

        let (chunk, diffs) = if chunk.is_static() {
            // Static data: make sure to keep the most recent chunk available for each component column.
            re_tracing::profile_scope!("static");

            let row_id_range_per_component = chunk.row_id_range_per_component();

            let mut overwritten_chunk_ids = HashMap::default();

            for (component_desc, list_array) in chunk.components().iter_flattened() {
                let is_empty = list_array
                    .validity()
                    .map_or(false, |validity| validity.is_empty());
                if is_empty {
                    continue;
                }

                let Some((_row_id_min_for_component, row_id_max_for_component)) =
                    row_id_range_per_component
                        .get(&component_desc.component_name)
                        .and_then(|per_desc| per_desc.get(component_desc))
                else {
                    continue;
                };

                self.static_chunk_ids_per_entity
                    .entry(chunk.entity_path().clone())
                    .or_default()
                    .entry(component_desc.component_name)
                    .and_modify(|cur_chunk_id| {
                        // NOTE: When attempting to overwrite static data, the chunk with the most
                        // recent data within -- according to RowId -- wins.

                        let cur_row_id_max_for_component = self
                            .chunks_per_chunk_id
                            .get(cur_chunk_id)
                            .map_or(RowId::ZERO, |chunk| {
                                chunk
                                    .row_id_range_per_component()
                                    .get(&component_desc.component_name)
                                    .and_then(|per_desc| per_desc.get(component_desc))
                                    .map_or(RowId::ZERO, |(_, row_id_max)| *row_id_max)
                            });

                        if *row_id_max_for_component > cur_row_id_max_for_component {
                            // We are about to overwrite the existing chunk with the new one, at
                            // least for this one specific component.
                            // Keep track of the overwritten ChunkId: we'll need it further down in
                            // order to check whether that chunk is now dangling.

                            // NOTE: The chunks themselves are indexed using the smallest RowId in
                            // the chunk _as a whole_, as opposed to the smallest RowId of one
                            // specific component in that chunk.
                            let cur_row_id_min_for_chunk = self
                                .chunks_per_chunk_id
                                .get(cur_chunk_id)
                                .and_then(|chunk| {
                                    chunk.row_id_range().map(|(row_id_min, _)| row_id_min)
                                });

                            debug_assert!(
                                cur_row_id_min_for_chunk.is_some(),
                                "This condition cannot fail, we just want to avoid unwrapping",
                            );
                            if let Some(cur_row_id_min_for_chunk) = cur_row_id_min_for_chunk {
                                overwritten_chunk_ids
                                    .insert(*cur_chunk_id, cur_row_id_min_for_chunk);
                            }

                            *cur_chunk_id = chunk.id();
                        }
                    })
                    .or_insert_with(|| chunk.id());
            }

            self.static_chunks_stats += ChunkStoreChunkStats::from_chunk(&chunk);

            let mut diffs = vec![ChunkStoreDiff::addition(
                non_compacted_chunk, /* added */
                None,                /* compacted */
            )];

            // NOTE: Our chunks can only cover a single entity path at a time, therefore we know we
            // only have to check that one entity for complete overwrite.
            debug_assert!(
                self.static_chunk_ids_per_entity
                    .contains_key(chunk.entity_path()),
                "This condition cannot fail, we just want to avoid unwrapping",
            );
            if let Some(per_component) = self.static_chunk_ids_per_entity.get(chunk.entity_path()) {
                re_tracing::profile_scope!("static dangling checks");

                // At this point, we are in possession of a list of ChunkIds that were at least
                // _partially_ overwritten (i.e. some, but not necessarily all, of the components
                // that they used to provide the data for are now provided by another, newer chunk).
                //
                // To determine whether any of these chunks are actually fully overwritten, and
                // therefore dangling, we need to make sure there are no components left
                // referencing these ChunkIds whatsoever.
                //
                // Because our storage model guarantees that a single chunk cannot cover more than
                // one entity, this is actually pretty cheap to do, since we only have to loop over
                // all the components of a single entity.

                for (chunk_id, chunk_row_id_min) in overwritten_chunk_ids {
                    let has_been_fully_overwritten = !per_component
                        .values()
                        .any(|cur_chunk_id| *cur_chunk_id == chunk_id);

                    if has_been_fully_overwritten {
                        // The chunk is now dangling: remove it from all relevant indices, update
                        // the stats, and fire deletion events.

                        let chunk_id_removed =
                            self.chunk_ids_per_min_row_id.remove(&chunk_row_id_min);
                        debug_assert!(chunk_id_removed.is_some());

                        let chunk_removed = self.chunks_per_chunk_id.remove(&chunk_id);
                        debug_assert!(chunk_removed.is_some());

                        if let Some(chunk_removed) = chunk_removed {
                            self.static_chunks_stats -=
                                ChunkStoreChunkStats::from_chunk(&chunk_removed);
                            diffs.push(ChunkStoreDiff::deletion(chunk_removed));
                        }
                    }
                }
            }

            (Arc::clone(&chunk), diffs)
        } else {
            // Temporal data: just index the chunk on every dimension of interest.
            re_tracing::profile_scope!("temporal");

            let (elected_chunk, chunk_or_compacted) = {
                re_tracing::profile_scope!("election");

                let elected_chunk = self.find_and_elect_compaction_candidate(&chunk);

                let chunk_or_compacted = if let Some(elected_chunk) = &elected_chunk {
                    let chunk_rowid_min = chunk.row_id_range().map(|(min, _)| min);
                    let elected_rowid_min = elected_chunk.row_id_range().map(|(min, _)| min);

                    let mut compacted = if elected_rowid_min < chunk_rowid_min {
                        re_tracing::profile_scope!("concat");
                        elected_chunk.concatenated(&chunk)?
                    } else {
                        re_tracing::profile_scope!("concat");
                        chunk.concatenated(elected_chunk)?
                    };

                    {
                        re_tracing::profile_scope!("sort");
                        compacted.sort_if_unsorted();
                    }

                    re_log::trace!(
                        "compacted {} ({} rows) and {} ({} rows) together, resulting in {} ({} rows)",
                        chunk.id(),
                        re_format::format_uint(chunk.num_rows()),
                        elected_chunk.id(),
                        re_format::format_uint(elected_chunk.num_rows()),
                        compacted.id(),
                        re_format::format_uint(compacted.num_rows()),
                    );

                    Arc::new(compacted)
                } else {
                    Arc::clone(&chunk)
                };

                (elected_chunk, chunk_or_compacted)
            };

            {
                re_tracing::profile_scope!("insertion (w/ component)");

                let temporal_chunk_ids_per_timeline = self
                    .temporal_chunk_ids_per_entity_per_component
                    .entry(chunk_or_compacted.entity_path().clone())
                    .or_default();

                // NOTE: We must make sure to use the time range of each specific component column
                // here, or we open ourselves to nasty edge cases.
                //
                // See the `latest_at_sparse_component_edge_case` test.
                for (timeline, time_range_per_component) in
                    chunk_or_compacted.time_range_per_component()
                {
                    let temporal_chunk_ids_per_component =
                        temporal_chunk_ids_per_timeline.entry(timeline).or_default();

                    for (component_name, per_desc) in time_range_per_component {
                        for (_component_desc, time_range) in per_desc {
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
                                .insert(chunk_or_compacted.id());
                            temporal_chunk_ids_per_time
                                .per_end_time
                                .entry(time_range.max())
                                .or_default()
                                .insert(chunk_or_compacted.id());
                        }
                    }
                }
            }

            {
                re_tracing::profile_scope!("insertion (w/o component)");

                let temporal_chunk_ids_per_timeline = self
                    .temporal_chunk_ids_per_entity
                    .entry(chunk_or_compacted.entity_path().clone())
                    .or_default();

                for (timeline, time_column) in chunk_or_compacted.timelines() {
                    let temporal_chunk_ids_per_time = temporal_chunk_ids_per_timeline
                        .entry(*timeline)
                        .or_default();

                    let time_range = time_column.time_range();

                    // See `ChunkIdSetPerTime::max_interval_length`'s documentation.
                    temporal_chunk_ids_per_time.max_interval_length = u64::max(
                        temporal_chunk_ids_per_time.max_interval_length,
                        time_range.abs_length(),
                    );

                    temporal_chunk_ids_per_time
                        .per_start_time
                        .entry(time_range.min())
                        .or_default()
                        .insert(chunk_or_compacted.id());
                    temporal_chunk_ids_per_time
                        .per_end_time
                        .entry(time_range.max())
                        .or_default()
                        .insert(chunk_or_compacted.id());
                }
            }

            self.temporal_chunks_stats += ChunkStoreChunkStats::from_chunk(&chunk_or_compacted);

            let mut diff = ChunkStoreDiff::addition(
                // NOTE: We are advertising only the non-compacted chunk as "added", i.e. only the new data.
                //
                // This makes sure that downstream subscribers only have to process what is new,
                // instead of needlessly reprocessing old rows that would appear to have been
                // removed and reinserted due to compaction.
                //
                // Subscribers will still be capable of tracking which chunks have been merged with which
                // by using the compaction report that we fill below.
                Arc::clone(&non_compacted_chunk), /* added */
                None,                             /* compacted */
            );
            if let Some(elected_chunk) = &elected_chunk {
                // NOTE: The chunk that we've just added has been compacted already!
                let srcs = std::iter::once((non_compacted_chunk.id(), non_compacted_chunk))
                    .chain(
                        self.remove_chunk(elected_chunk.id())
                            .into_iter()
                            .filter(|diff| diff.kind == crate::ChunkStoreDiffKind::Deletion)
                            .map(|diff| (diff.chunk.id(), diff.chunk)),
                    )
                    .collect();

                diff.compacted = Some(crate::ChunkCompactionReport {
                    srcs,
                    new_chunk: chunk_or_compacted.clone(),
                });
            }

            (chunk_or_compacted, vec![diff])
        };

        self.chunks_per_chunk_id.insert(chunk.id(), chunk.clone());
        self.chunk_ids_per_min_row_id
            .entry(row_id_range.0)
            .or_default()
            .push(chunk.id());

        for (component_descr, list_array) in chunk.components().iter_flattened() {
            self.type_registry.insert(
                component_descr.component_name,
                Arrow2ListArray::<i32>::get_child_type(list_array.data_type()).clone(),
            );

            let column_metadata_state = self
                .per_column_metadata
                .entry(chunk.entity_path().clone())
                .or_default()
                .entry(component_descr.component_name)
                .or_default()
                .entry(component_descr.clone())
                .or_insert(ColumnMetadataState {
                    is_semantically_empty: true,
                });
            {
                let is_semantically_empty =
                    re_chunk::util::is_list_array_semantically_empty(list_array);

                column_metadata_state.is_semantically_empty &= is_semantically_empty;
            }
        }

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

            Self::on_events(&events);

            events
        } else {
            Vec::new()
        };

        Ok(events)
    }

    /// Finds the most appropriate candidate for compaction.
    ///
    /// The algorithm is simple: for each incoming [`Chunk`], we take a look at its future neighbors.
    /// Each neighbor is a potential candidate for compaction.
    ///
    /// Because the chunk is going to be inserted into many different indices -- for each of its timelines
    /// and components -- it will have many direct neighbors.
    /// Everytime we encounter a neighbor, it earns points.
    ///
    /// The neighbor with the most points at the end of the process is elected.
    fn find_and_elect_compaction_candidate(&self, chunk: &Arc<Chunk>) -> Option<Arc<Chunk>> {
        re_tracing::profile_function!();

        let mut candidates_below_threshold: HashMap<ChunkId, bool> = HashMap::default();
        let mut check_if_chunk_below_threshold =
            |store: &Self, candidate_chunk_id: ChunkId| -> bool {
                let ChunkStoreConfig {
                    enable_changelog: _,
                    chunk_max_bytes,
                    chunk_max_rows,
                    chunk_max_rows_if_unsorted,
                } = store.config;

                *candidates_below_threshold
                    .entry(candidate_chunk_id)
                    .or_insert_with(|| {
                        store.chunks_per_chunk_id.get(&candidate_chunk_id).map_or(
                            false,
                            |candidate| {
                                if !chunk.concatenable(candidate) {
                                    return false;
                                }

                                let total_bytes = <Chunk as SizeBytes>::total_size_bytes(chunk)
                                    + <Chunk as SizeBytes>::total_size_bytes(candidate);
                                let is_below_bytes_threshold = total_bytes <= chunk_max_bytes;

                                let total_rows = (chunk.num_rows() + candidate.num_rows()) as u64;
                                let is_below_rows_threshold = if candidate.is_time_sorted() {
                                    total_rows <= chunk_max_rows
                                } else {
                                    total_rows <= chunk_max_rows_if_unsorted
                                };

                                is_below_bytes_threshold && is_below_rows_threshold
                            },
                        )
                    })
            };

        let mut candidates: HashMap<ChunkId, u64> = HashMap::default();

        let temporal_chunk_ids_per_timeline = self
            .temporal_chunk_ids_per_entity_per_component
            .get(chunk.entity_path())?;

        for (timeline, time_range_per_component) in chunk.time_range_per_component() {
            let Some(temporal_chunk_ids_per_component) =
                temporal_chunk_ids_per_timeline.get(&timeline)
            else {
                continue;
            };

            for (component_name, per_desc) in time_range_per_component {
                for (_component_desc, time_range) in per_desc {
                    let Some(temporal_chunk_ids_per_time) =
                        temporal_chunk_ids_per_component.get(&component_name)
                    else {
                        continue;
                    };

                    {
                        // Direct neighbors (before): 1 point each.
                        if let Some((_data_time, chunk_id_set)) = temporal_chunk_ids_per_time
                            .per_start_time
                            .range(..time_range.min())
                            .next_back()
                        {
                            for &chunk_id in chunk_id_set {
                                if check_if_chunk_below_threshold(self, chunk_id) {
                                    *candidates.entry(chunk_id).or_default() += 1;
                                }
                            }
                        }

                        // Direct neighbors (after): 1 point each.
                        if let Some((_data_time, chunk_id_set)) = temporal_chunk_ids_per_time
                            .per_start_time
                            .range(time_range.max().inc()..)
                            .next()
                        {
                            for &chunk_id in chunk_id_set {
                                if check_if_chunk_below_threshold(self, chunk_id) {
                                    *candidates.entry(chunk_id).or_default() += 1;
                                }
                            }
                        }

                        let chunk_id_set = temporal_chunk_ids_per_time
                            .per_start_time
                            .get(&time_range.min());

                        // Shared start times: 2 points each.
                        for chunk_id in chunk_id_set.iter().flat_map(|set| set.iter().copied()) {
                            if check_if_chunk_below_threshold(self, chunk_id) {
                                *candidates.entry(chunk_id).or_default() += 2;
                            }
                        }
                    }
                }
            }
        }

        debug_assert!(!candidates.contains_key(&chunk.id()));

        let mut candidates = candidates.into_iter().collect_vec();
        candidates.sort_by_key(|(_chunk_id, points)| *points);
        candidates.reverse();

        candidates
            .into_iter()
            .find_map(|(chunk_id, _points)| self.chunks_per_chunk_id.get(&chunk_id).map(Arc::clone))
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
            info: _,
            config: _,
            type_registry: _,
            per_column_metadata,
            chunks_per_chunk_id,
            chunk_ids_per_min_row_id,
            temporal_chunk_ids_per_entity_per_component,
            temporal_chunk_ids_per_entity,
            temporal_chunks_stats,
            static_chunk_ids_per_entity,
            static_chunks_stats,
            insert_id: _,
            query_id: _,
            gc_id: _,
            event_id,
        } = self;

        per_column_metadata.remove(entity_path);

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
            temporal_chunk_ids_per_entity_per_component.remove(entity_path);

            let dropped_temporal_chunk_ids: BTreeSet<_> = temporal_chunk_ids_per_entity
                .remove(entity_path)
                .unwrap_or_default()
                .into_values()
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

        if self.config.enable_changelog {
            let events: Vec<_> = dropped_static_chunks
                .into_iter()
                .chain(dropped_temporal_chunks)
                .map(ChunkStoreDiff::deletion)
                .map(|diff| ChunkStoreEvent {
                    store_id: id.clone(),
                    store_generation: generation.clone(),
                    event_id: event_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                    diff,
                })
                .collect();

            Self::on_events(&events);

            events
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use re_chunk::{TimePoint, Timeline};
    use re_log_types::example_components::{MyColor, MyLabel, MyPoint};
    use similar_asserts::assert_eq;

    use crate::ChunkStoreDiffKind;

    use super::*;

    // TODO(cmc): We could have more test coverage here, especially regarding thresholds etc.
    // For now the development and maintenance cost doesn't seem to be worth it.
    // We can re-assess later if things turns out to be shaky in practice.

    #[test]
    fn compaction_simple() -> anyhow::Result<()> {
        re_log::setup_logging();

        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );

        let entity_path = EntityPath::from("this/that");

        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();
        let row_id6 = RowId::new();
        let row_id7 = RowId::new();
        let row_id8 = RowId::new();
        let row_id9 = RowId::new();
        let row_id10 = RowId::new();

        let timepoint1 = [(Timeline::new_sequence("frame"), 1)];
        let timepoint2 = [(Timeline::new_sequence("frame"), 3)];
        let timepoint3 = [(Timeline::new_sequence("frame"), 5)];
        let timepoint4 = [(Timeline::new_sequence("frame"), 7)];
        let timepoint5 = [(Timeline::new_sequence("frame"), 9)];

        let points1 = &[MyPoint::new(1.0, 1.0)];
        let points2 = &[MyPoint::new(2.0, 2.0)];
        let points3 = &[MyPoint::new(3.0, 3.0)];
        let points4 = &[MyPoint::new(4.0, 4.0)];
        let points5 = &[MyPoint::new(5.0, 5.0)];

        let chunk1 = Chunk::builder(entity_path.clone())
            .with_component_batches(row_id1, timepoint1, [points1 as _])
            .with_component_batches(row_id2, timepoint2, [points2 as _])
            .with_component_batches(row_id3, timepoint3, [points3 as _])
            .build()?;
        let chunk2 = Chunk::builder(entity_path.clone())
            .with_component_batches(row_id4, timepoint4, [points4 as _])
            .with_component_batches(row_id5, timepoint5, [points5 as _])
            .build()?;
        let chunk3 = Chunk::builder(entity_path.clone())
            .with_component_batches(row_id6, timepoint1, [points1 as _])
            .with_component_batches(row_id7, timepoint2, [points2 as _])
            .with_component_batches(row_id8, timepoint3, [points3 as _])
            .build()?;
        let chunk4 = Chunk::builder(entity_path.clone())
            .with_component_batches(row_id9, timepoint4, [points4 as _])
            .with_component_batches(row_id10, timepoint5, [points5 as _])
            .build()?;

        let chunk1 = Arc::new(chunk1);
        let chunk2 = Arc::new(chunk2);
        let chunk3 = Arc::new(chunk3);
        let chunk4 = Arc::new(chunk4);

        eprintln!("---\n{store}\ninserting {}", chunk1.id());

        store.insert_chunk(&chunk1)?;

        eprintln!("---\n{store}\ninserting {}", chunk2.id());

        store.insert_chunk(&chunk2)?;

        eprintln!("---\n{store}\ninserting {}", chunk3.id());

        store.insert_chunk(&chunk3)?;

        eprintln!("---\n{store}\ninserting {}", chunk4.id());

        store.insert_chunk(&chunk4)?;

        eprintln!("---\n{store}");

        let got = store
            .chunks_per_chunk_id
            .first_key_value()
            .map(|(_id, chunk)| chunk)
            .unwrap();

        let expected = Chunk::builder_with_id(got.id(), entity_path.clone())
            .with_component_batches(row_id1, timepoint1, [points1 as _])
            .with_component_batches(row_id2, timepoint2, [points2 as _])
            .with_component_batches(row_id3, timepoint3, [points3 as _])
            .with_component_batches(row_id4, timepoint4, [points4 as _])
            .with_component_batches(row_id5, timepoint5, [points5 as _])
            .with_component_batches(row_id6, timepoint1, [points1 as _])
            .with_component_batches(row_id7, timepoint2, [points2 as _])
            .with_component_batches(row_id8, timepoint3, [points3 as _])
            .with_component_batches(row_id9, timepoint4, [points4 as _])
            .with_component_batches(row_id10, timepoint5, [points5 as _])
            .build()?;

        assert_eq!(1, store.chunks_per_chunk_id.len());
        assert_eq!(
            expected,
            **got,
            "{}",
            similar_asserts::SimpleDiff::from_str(
                &format!("{expected}"),
                &format!("{got}"),
                "expected",
                "got",
            ),
        );

        Ok(())
    }

    #[test]
    fn static_overwrites() -> anyhow::Result<()> {
        re_log::setup_logging();

        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );

        let entity_path = EntityPath::from("this/that");

        let row_id1_1 = RowId::new();
        let row_id2_1 = RowId::new();
        let row_id2_2 = RowId::new();

        let timepoint_static = TimePoint::default();

        let points1 = &[MyPoint::new(1.0, 1.0)];
        let colors1 = &[MyColor::from_rgb(1, 1, 1)];
        let labels1 = &[MyLabel("111".to_owned())];

        let points2 = &[MyPoint::new(2.0, 2.0)];
        let colors2 = &[MyColor::from_rgb(2, 2, 2)];
        let labels2 = &[MyLabel("222".to_owned())];

        let chunk1 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id1_1,
                timepoint_static.clone(),
                [points1 as _, colors1 as _, labels1 as _],
            )
            .build()?;
        let chunk2 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id2_1,
                timepoint_static.clone(),
                [points2 as _, colors2 as _],
            )
            .build()?;
        let chunk3 = Chunk::builder(entity_path.clone())
            .with_component_batches(row_id2_2, timepoint_static, [labels2 as _])
            .build()?;

        let chunk1 = Arc::new(chunk1);
        let chunk2 = Arc::new(chunk2);
        let chunk3 = Arc::new(chunk3);

        let events = store.insert_chunk(&chunk1)?;
        assert!(
            events.len() == 1
                && events[0].chunk.id() == chunk1.id()
                && events[0].kind == ChunkStoreDiffKind::Addition,
            "the first write should result in the addition of chunk1 and nothing else"
        );

        let events = store.insert_chunk(&chunk2)?;
        assert!(
            events.len() == 1
                && events[0].chunk.id() == chunk2.id()
                && events[0].kind == ChunkStoreDiffKind::Addition,
            "the second write should result in the addition of chunk2 and nothing else"
        );

        let stats_before = store.stats();
        {
            let ChunkStoreChunkStats {
                num_chunks,
                total_size_bytes: _,
                num_rows,
                num_events,
            } = stats_before.static_chunks;
            assert_eq!(2, num_chunks);
            assert_eq!(2, num_rows);
            assert_eq!(5, num_events);
        }

        let events = store.insert_chunk(&chunk3)?;
        assert!(
            events.len() == 2
                && events[0].chunk.id() == chunk3.id()
                && events[0].kind == ChunkStoreDiffKind::Addition
                && events[1].chunk.id() == chunk1.id()
                && events[1].kind == ChunkStoreDiffKind::Deletion,
            "the final write should result in the addition of chunk3 _and_ the deletion of the now fully overwritten chunk1"
        );

        let stats_after = store.stats();
        {
            let ChunkStoreChunkStats {
                num_chunks,
                total_size_bytes: _,
                num_rows,
                num_events,
            } = stats_after.static_chunks;
            assert_eq!(2, num_chunks);
            assert_eq!(2, num_rows);
            assert_eq!(3, num_events);
        }

        Ok(())
    }
}
