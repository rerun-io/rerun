use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ahash::HashMap;
use arrow::array::Array as _;
use itertools::Itertools as _;

use re_byte_size::SizeBytes;
use re_chunk::{Chunk, EntityPath, RowId};
use re_log_encoding::{RrdManifest, RrdManifestTemporalMapEntry};

use crate::store::ChunkIdSetPerTime;
use crate::{
    ChunkDirectLineage, ChunkDirectLineageReport, ChunkId, ChunkStore, ChunkStoreChunkStats,
    ChunkStoreConfig, ChunkStoreDiff, ChunkStoreDiffAddition, ChunkStoreError, ChunkStoreEvent,
    ChunkStoreResult, ColumnMetadataState,
};

// ---

impl ChunkStore {
    /// This insert a batch of virtual chunks into the store, according to the given [`RrdManifest`].
    ///
    /// Since no physical data is involved, no events are fired.
    /// All queries will return partial results until the missing physical data gets loaded in.
    pub fn insert_rrd_manifest(
        &mut self,
        rrd_manifest: Arc<RrdManifest>,
    ) -> ChunkStoreResult<ChunkStoreEvent> {
        re_tracing::profile_function!();

        let Self {
            id: _,
            config: _,
            time_type_registry,
            type_registry,
            per_column_metadata,
            chunks_per_chunk_id: _,      // physical data only
            chunk_ids_per_min_row_id: _, // physical data only
            chunks_lineage,
            dangling_splits: _,   // cannot split during virtual insert
            leaky_compactions: _, // cannot compact during virtual insert
            temporal_chunk_ids_per_entity_per_component,
            temporal_chunk_ids_per_entity,
            temporal_physical_chunks_stats: _, // stats are for physical data only
            static_chunk_ids_per_entity,
            static_chunks_stats: _, // stats are for physical data only
            queried_chunk_id_tracker: _,
            insert_id: _,
            gc_id: _,
            event_id: _,
        } = self;

        let sorbet_schema = re_sorbet::SorbetSchema::try_from_raw_arrow_schema(Arc::new(
            rrd_manifest.sorbet_schema().clone(),
        ))?;

        time_type_registry.extend(
            sorbet_schema
                .columns
                .index_columns()
                .map(|descr| (descr.timeline_name(), descr.timeline().typ())),
        );

        for descr in sorbet_schema.columns.component_columns() {
            let Some((component_type, inner_datatype)) = descr
                .component_type
                .map(|typ| (typ, descr.inner_datatype()))
            else {
                continue;
            };

            if let Some(old_typ) = type_registry.insert(component_type, inner_datatype.clone())
                && old_typ != inner_datatype
            {
                re_log::warn_once!(
                    "Component '{}' with component type '{}' on entity '{}' changed type from {} to {}",
                    descr.component,
                    component_type,
                    descr.entity_path,
                    re_arrow_util::format_data_type(&old_typ),
                    re_arrow_util::format_data_type(&inner_datatype)
                );
            }
        }

        for descr in sorbet_schema.columns.component_columns() {
            let inner_datatype = descr.inner_datatype();
            let previous = per_column_metadata
                .entry(descr.entity_path.clone())
                .or_default()
                .insert(
                    descr.component,
                    (
                        descr.component_descriptor(),
                        ColumnMetadataState {
                            is_semantically_empty: descr.is_semantically_empty,
                        },
                        inner_datatype.clone(),
                    ),
                );

            if let Some(previous) = previous
                && previous.2 != inner_datatype
            {
                re_log::warn_once!(
                    "Component '{}' on entity '{}' changed type from {} to {}",
                    descr.component,
                    descr.entity_path,
                    re_arrow_util::format_data_type(&previous.2),
                    re_arrow_util::format_data_type(&inner_datatype)
                );
            }
        }

        let native_static_map = rrd_manifest.get_static_data_as_a_map()?;
        chunks_lineage.extend(
            native_static_map
                .values()
                .flat_map(|per_component| per_component.values())
                .map(|chunk_id| {
                    (
                        *chunk_id,
                        ChunkDirectLineage::ReferencedFrom(rrd_manifest.clone()),
                    )
                }),
        );
        *static_chunk_ids_per_entity = native_static_map;

        let native_temporal_map = rrd_manifest.get_temporal_data_as_a_map()?;
        chunks_lineage.extend(
            native_temporal_map
                .values()
                .flat_map(|per_timeline| per_timeline.values())
                .flat_map(|per_component| per_component.values())
                .flat_map(|per_chunk| per_chunk.keys())
                .map(|chunk_id| {
                    (
                        *chunk_id,
                        ChunkDirectLineage::ReferencedFrom(rrd_manifest.clone()),
                    )
                }),
        );
        for (entity_path, per_timeline) in native_temporal_map {
            for (timeline, per_component) in per_timeline {
                for (component, per_chunk) in per_component {
                    for (chunk_id, entry) in per_chunk {
                        let RrdManifestTemporalMapEntry {
                            time_range,
                            num_rows: _,
                        } = entry;
                        // with component
                        {
                            let per_timeline = temporal_chunk_ids_per_entity_per_component
                                .entry(entity_path.clone())
                                .or_default();
                            let per_component = per_timeline.entry(*timeline.name()).or_default();
                            let ChunkIdSetPerTime {
                                max_interval_length,
                                per_start_time,
                                per_end_time,
                            } = per_component.entry(component).or_default();
                            *max_interval_length =
                                (*max_interval_length).max(time_range.abs_length());
                            per_start_time
                                .entry(time_range.min)
                                .or_default()
                                .insert(chunk_id);
                            per_end_time
                                .entry(time_range.max)
                                .or_default()
                                .insert(chunk_id);
                        }

                        // without component
                        {
                            let per_timeline = temporal_chunk_ids_per_entity
                                .entry(entity_path.clone())
                                .or_default();
                            let ChunkIdSetPerTime {
                                max_interval_length,
                                per_start_time,
                                per_end_time,
                            } = per_timeline.entry(*timeline.name()).or_default();
                            *max_interval_length =
                                (*max_interval_length).max(time_range.abs_length());
                            per_start_time
                                .entry(time_range.min)
                                .or_default()
                                .insert(chunk_id);
                            per_end_time
                                .entry(time_range.max)
                                .or_default()
                                .insert(chunk_id);
                        }
                    }
                }
            }
        }

        Ok(ChunkStoreEvent {
            store_id: self.id.clone(),
            store_generation: self.generation(),
            event_id: self
                .event_id
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            diff: ChunkStoreDiff::virtual_addition(rrd_manifest),
        })
    }

    /// Inserts a [`Chunk`] in the store.
    ///
    /// Iff the store was modified, all registered subscribers will be notified and the
    /// resulting [`ChunkStoreEvent`] will be returned, or `None` otherwise.
    ///
    /// * Trying to insert an unsorted chunk ([`Chunk::is_sorted`]) will fail with an error.
    /// * Inserting a duplicated [`ChunkId`] will result in a no-op.
    /// * Inserting an empty [`Chunk`] will result in a no-op.
    pub fn insert_chunk(&mut self, chunk: &Arc<Chunk>) -> ChunkStoreResult<Vec<ChunkStoreEvent>> {
        if !self.is_root_chunk(&chunk.id()) {
            if cfg!(debug_assertions) {
                re_log::warn_once!("Attempted to insert non-root chunk (this has no effect)");
            } else {
                re_log::debug_once!("Attempted to insert non-root chunk (this has no effect)");
            }
            return Ok(vec![]);
        }

        let diffs = self.insert_chunk_impl(chunk, ChunkDirectLineageReport::Volatile)?;

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

        if self.config.enable_changelog {
            Self::on_events(&events);
        }

        Ok(events)
    }

    fn insert_chunk_impl(
        &mut self,
        chunk: &Arc<Chunk>,
        lineage: ChunkDirectLineageReport,
    ) -> ChunkStoreResult<Vec<ChunkStoreDiff>> {
        let mut all_diffs = Vec::new();

        if chunk.is_empty() {
            return Ok(all_diffs); // nothing to do
        }

        if chunk.components().is_empty() {
            // This can happen in 2 scenarios: A) a badly manually crafted chunk or B) an Indicator
            // chunk that went through the Sorbet migration process, and ended up with zero
            // component columns.
            //
            // When that happens, the election process in the compactor will get confused, and then not
            // only that weird empty Chunk will end up being stored, but it will also prevent the
            // election from making progress and therefore prevent Chunks that are in dire need of
            // compaction from being compacted.
            //
            // The solution is simple: just drop it.
            return Ok(all_diffs);
        }

        if let Some(prev_chunk) = self.chunks_per_chunk_id.get(&chunk.id()) {
            if cfg!(debug_assertions) {
                if let Err(difference) = Chunk::ensure_similar(prev_chunk, chunk) {
                    re_log::error_once!(
                        "The chunk id {} was used twice for two _different_ chunks. Difference: {difference}",
                        chunk.id()
                    );
                } else {
                    re_log::warn_once!("The same chunk was inserted twice (this has no effect)");
                }
            } else {
                re_log::debug_once!("The same chunk was inserted twice (this has no effect)");
            }

            // We assume that chunk IDs are unique, and so inserting the same chunk twice has no effect.
            return Ok(all_diffs);
        }

        if !chunk.is_sorted() {
            return Err(ChunkStoreError::UnsortedChunk);
        }

        re_tracing::profile_function!();

        {
            // Is there any chunk still actively loaded into the store that already contains the data
            // we're about to insert, by virtue of directly or indirectly descending from a compaction
            // that originally included that same data?
            // If so, we should not insert anything: the data is already there, in a more optimized form, even.

            let mut source_id = chunk.id();
            while let Some(chunk_id) = self.leaky_compactions.get(&source_id) {
                if self.chunks_per_chunk_id.contains_key(chunk_id) {
                    return Ok(vec![]);
                }
                source_id = *chunk_id;
            }
        }

        // If this chunk has already been inserted before, and yielded a bunch of smaller splits, then we
        // need to make sure that all these splits are now gone, so we don't end up with unnecessary
        // fragmented overlaps affecting performance for no good reason.
        if let Some(split_chunk_ids) = self.dangling_splits.remove(&chunk.id()) {
            all_diffs.extend(
                // There is no way for these splits to be ever useful again since their IDs are random
                // anyway. So deep removal it is.
                self.remove_chunks_deep(
                    split_chunk_ids
                        .into_iter()
                        .filter_map(|chunk_id| self.chunks_per_chunk_id.get(&chunk_id).cloned())
                        .collect(),
                    None,
                )
                .into_iter()
                .map(Into::into),
            );
        }

        self.chunks_lineage
            .entry(chunk.id())
            // `.or_insert_with` because we don't want to lose the RRD manifest lineage if there is one.
            .or_insert_with(|| (&lineage).into());

        // Splitting a static chunk just seems like a terrible idea in general.
        let chunk_is_static = chunk.is_static();

        // If we're coming in here from the recursive call caused by a split, then we shouldn't try splitting at all.
        //
        // Mathematically, this is not needed: since we already split as much as needed on the first-and-only
        // flattened split iteration, we will never split anything again.
        // On the other hand, the explicitness makes this much easier to understand, and prevents unfortunate
        // accidents when this code inevitably get refactored in the future.
        //
        // Also, splitting a static chunk just seems like a terrible idea in general.
        let chunk_descends_from_split = self.descends_from_a_split(&chunk.id());

        // We should never try and split a chunk issued from a compaction, it's just counter-productive.
        let chunk_descends_from_compaction = self.descends_from_a_compaction(&chunk.id());

        if !chunk_is_static && !chunk_descends_from_split && !chunk_descends_from_compaction {
            // Always perform the recursive splitting inline, so that we don't generate and send store
            // events that won't make any sense for downstream consumers, since these halfway chunks
            // never get exposed to the outside world in any way.
            let mut split_chunks: Vec<Arc<Chunk>> = Chunk::split_chunk_if_needed(
                chunk.clone(),
                &re_chunk::ChunkSplitConfig {
                    chunk_max_bytes: self.config.chunk_max_bytes,
                    chunk_max_rows: self.config.chunk_max_rows,
                    chunk_max_rows_if_unsorted: self.config.chunk_max_rows_if_unsorted,
                },
            );
            loop {
                re_tracing::profile_scope!("compute-recursive-splits");

                let num_split_chunks = split_chunks.len();
                split_chunks = split_chunks
                    .into_iter()
                    .flat_map(|split_chunk| {
                        if self.descends_from_a_compaction(&split_chunk.id()) {
                            // Never split a chunk that descends from a direct or indirect compaction, ever.
                            vec![]
                        } else {
                            Chunk::split_chunk_if_needed(
                                split_chunk.clone(),
                                &re_chunk::ChunkSplitConfig {
                                    chunk_max_bytes: self.config.chunk_max_bytes,
                                    chunk_max_rows: self.config.chunk_max_rows,
                                    chunk_max_rows_if_unsorted: self
                                        .config
                                        .chunk_max_rows_if_unsorted,
                                },
                            )
                        }
                    })
                    .collect_vec();

                if split_chunks.len() == num_split_chunks {
                    // There was nothing new to split, therefore: we're done.
                    break;
                }
            }
            if split_chunks.len() > 1 {
                re_tracing::profile_scope!("add-splits");

                // For a split, we keep track of our descendents so that we can accurately drop
                // dangling splits later on if the parent get re-inserted.
                self.dangling_splits
                    .insert(chunk.id(), split_chunks.iter().map(|c| c.id()).collect());

                for split_chunk in &split_chunks {
                    let siblings = split_chunks
                        .iter()
                        .filter(|c| c.id() != split_chunk.id())
                        .cloned()
                        .collect_vec();
                    let lineage = ChunkDirectLineageReport::SplitFrom(chunk.clone(), siblings);

                    let mut diffs = self.insert_chunk_impl(split_chunk, lineage)?;
                    for diff in &mut diffs {
                        // In case of a split, the unprocessed chunk should always match the parent
                        // chunk as specified in the `SplitFrom` lineage.
                        // By default this won't be the case due to how the recursion is implemented,
                        // so make sure to patch the data appropriately.
                        if let ChunkStoreDiff::Addition(add) = diff {
                            add.chunk_before_processing = chunk.clone();
                        }
                    }

                    all_diffs.extend(diffs);
                }

                return Ok(all_diffs);
            }
        }

        self.insert_id += 1;

        let chunk_before_processing = Arc::clone(chunk); // we'll need it to create the store event

        let (chunk_after_processing, diffs) = if chunk.is_static() {
            // Static data: make sure to keep the most recent chunk available for each component column.
            re_tracing::profile_scope!("static");

            let row_id_range_per_component = chunk.row_id_range_per_component();

            let mut overwritten_chunk_ids = HashMap::default();

            for (component, column) in chunk.components().iter() {
                let is_empty = column
                    .list_array
                    .nulls()
                    .is_some_and(|validity| validity.is_empty());
                if is_empty {
                    continue;
                }

                let Some((_row_id_min_for_component, row_id_max_for_component)) =
                    row_id_range_per_component.get(component)
                else {
                    continue;
                };

                self.static_chunk_ids_per_entity
                    .entry(chunk.entity_path().clone())
                    .or_default()
                    .entry(*component)
                    .and_modify(|cur_chunk_id| {
                        // NOTE: When attempting to overwrite static data, the chunk with the most
                        // recent data within -- according to RowId -- wins.

                        let cur_row_id_max_for_component = self
                            .chunks_per_chunk_id
                            .get(cur_chunk_id)
                            .map_or(RowId::ZERO, |chunk| {
                                chunk
                                    .row_id_range_per_component()
                                    .get(component)
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

                            if let Some(cur_row_id_min_for_chunk) = cur_row_id_min_for_chunk {
                                overwritten_chunk_ids
                                    .insert(*cur_chunk_id, cur_row_id_min_for_chunk);
                            }

                            *cur_chunk_id = chunk.id();
                        }
                    })
                    .or_insert_with(|| chunk.id());
            }

            self.static_chunks_stats += ChunkStoreChunkStats::from_chunk(chunk);

            let mut diffs = vec![ChunkStoreDiff::addition(
                chunk_before_processing, /* chunk_before_processing */
                chunk.clone(),           /* chunk_after_processing */
                lineage,                 /* lineage */
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

            (Arc::clone(chunk), diffs)
        } else {
            // Temporal data: just index the chunk on every dimension of interest.
            re_tracing::profile_scope!("temporal");

            let (elected_chunk, chunk_or_compacted) = {
                re_tracing::profile_scope!("election");

                let elected_chunk = self.find_and_elect_compaction_candidate(chunk);

                let chunk_or_compacted = if let Some(elected_chunk) = &elected_chunk {
                    let chunk_rowid_min = chunk.row_id_range().map(|(min, _)| min);
                    let elected_rowid_min = elected_chunk.row_id_range().map(|(min, _)| min);

                    let mut compacted = if elected_rowid_min < chunk_rowid_min {
                        re_tracing::profile_scope!("concat");
                        elected_chunk.concatenated(chunk)?
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
                    Arc::clone(chunk)
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

                    for (component, time_range) in time_range_per_component {
                        let temporal_chunk_ids_per_time = temporal_chunk_ids_per_component
                            .entry(component)
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

            self.temporal_physical_chunks_stats +=
                ChunkStoreChunkStats::from_chunk(&chunk_or_compacted);

            let mut add = ChunkStoreDiffAddition {
                // NOTE: We are advertising only the non-compacted chunk as "added", i.e. only the new data.
                //
                // This makes sure that downstream subscribers only have to process what is new,
                // instead of needlessly reprocessing old rows that would appear to have been
                // removed and reinserted due to compaction.
                //
                // Subscribers will still be capable of tracking which chunks have been merged with which
                // by using the compaction report that we fill below.
                chunk_before_processing: Arc::clone(&chunk_before_processing),
                chunk_after_processing: chunk_or_compacted.clone(),
                direct_lineage: lineage, // lineage (might be patched below)
            };
            if let Some(elected_chunk) = &elected_chunk {
                // NOTE: The chunk that we've just added has been compacted already!
                let srcs: BTreeMap<_, _> =
                    std::iter::once((chunk_before_processing.id(), chunk_before_processing))
                        .chain(
                            // NOTE: deep removal, we don't want a compacted chunk to linger on!
                            self.remove_chunks_deep(vec![elected_chunk.clone()], None)
                                .into_iter()
                                .map(|diff| (diff.chunk.id(), diff.chunk)),
                        )
                        .collect();

                for source_id in srcs.keys().copied() {
                    let found = self
                        .leaky_compactions
                        .insert(source_id, chunk_or_compacted.id())
                        .is_some();

                    #[expect(clippy::manual_assert)]
                    if cfg!(debug_assertions) && found {
                        let mut source_id = source_id;
                        while let Some(chunk_id) = self.leaky_compactions.get(&source_id) {
                            if self.chunks_per_chunk_id.contains_key(chunk_id) {
                                panic!(
                                    "leaky compaction tracker should never get overwritten as long as one \
                                    or more direct or indirect compacted chunks still exist"
                                );
                            }
                            source_id = *chunk_id;
                        }
                    }
                }

                add.direct_lineage = ChunkDirectLineageReport::CompactedFrom(srcs);
            }

            (chunk_or_compacted, vec![add.into()])
        };

        self.chunks_per_chunk_id
            .insert(chunk_after_processing.id(), chunk_after_processing.clone());

        for diff in &diffs {
            if let ChunkStoreDiff::Addition(add) = diff
                && let report @ ChunkDirectLineageReport::CompactedFrom(_) = &add.direct_lineage
            {
                self.chunks_lineage
                    .insert(add.chunk_after_processing.id(), report.into());
            }
        }
        all_diffs.extend(diffs);

        // NOTE: ⚠️Make sure to recompute the Row ID range! The chunk might have been compacted
        // with another one, which might or might not have modified the range.
        if let Some(min_row_id) = chunk_after_processing.row_id_range().map(|(min, _)| min)
            && self
                .chunk_ids_per_min_row_id
                .insert(min_row_id, chunk_after_processing.id())
                .is_some()
        {
            re_log::warn_once!(
                "Detected duplicated RowId in the data, this will lead to undefined behavior"
            );
        }

        for (name, columns) in chunk_after_processing.timelines() {
            let new_typ = columns.timeline().typ();
            if let Some(old_typ) = self.time_type_registry.insert(*name, new_typ)
                && old_typ != new_typ
            {
                re_log::warn_once!(
                    "Timeline '{name}' changed type from {old_typ:?} to {new_typ:?}. \
                        Rerun does not support using different types for the same timeline.",
                );
            }
        }

        for column in chunk_after_processing.components().values() {
            let re_types_core::SerializedComponentColumn {
                list_array,
                descriptor,
            } = column;

            if let Some(component_type) = descriptor.component_type
                && let Some(old_typ) = self
                    .type_registry
                    .insert(component_type, list_array.value_type())
                && old_typ != column.list_array.value_type()
            {
                re_log::warn_once!(
                    "Component '{}' with component type '{}' on entity '{}' changed type from {} to {}",
                    descriptor.component,
                    component_type,
                    chunk_after_processing.entity_path(),
                    re_arrow_util::format_data_type(&old_typ),
                    re_arrow_util::format_data_type(&column.list_array.value_type())
                );
            }

            let (descr, column_metadata_state, datatype) = self
                .per_column_metadata
                .entry(chunk_after_processing.entity_path().clone())
                .or_default()
                .entry(descriptor.component)
                .or_insert_with(|| {
                    (
                        descriptor.clone(),
                        ColumnMetadataState {
                            is_semantically_empty: true,
                        },
                        list_array.value_type().clone(),
                    )
                });
            {
                if *datatype != list_array.value_type() {
                    // TODO(grtlr): If we encounter two different data types, we should split the chunk.
                    // More information: https://github.com/rerun-io/rerun/pull/10082#discussion_r2140549340
                    re_log::warn!(
                        "Datatype of column {descr} in {} has changed from {datatype} to {}",
                        chunk_after_processing.entity_path(),
                        list_array.value_type()
                    );
                    *datatype = list_array.value_type().clone();
                }

                let is_semantically_empty =
                    re_arrow_util::is_list_array_semantically_empty(list_array);

                column_metadata_state.is_semantically_empty &= is_semantically_empty;
            }
        }

        Ok(all_diffs)
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

        // Early exit if the newly added Chunk is already the result of a split, directly or indirectly.
        // Compacting chunks coming from a split lineage is generally a mistake, as that is likely
        // to lead to overlaps that weren't there in the first place.
        if self.descends_from_a_split(&chunk.id()) {
            return None;
        }

        {
            // Make sure to early exit if the newly added Chunk is already beyond the compaction thresholds
            // on its own.

            let ChunkStoreConfig {
                enable_changelog: _,
                chunk_max_bytes,
                chunk_max_rows,
                chunk_max_rows_if_unsorted,
            } = self.config;

            let total_bytes = <Chunk as SizeBytes>::total_size_bytes(chunk);
            let is_below_bytes_threshold = total_bytes <= chunk_max_bytes;

            let total_rows = (chunk.num_rows()) as u64;
            let is_below_rows_threshold = if chunk.is_time_sorted() {
                total_rows <= chunk_max_rows
            } else {
                total_rows <= chunk_max_rows_if_unsorted
            };

            if !(is_below_bytes_threshold && is_below_rows_threshold) {
                return None;
            }
        }

        let mut candidates_below_threshold: HashMap<ChunkId, u64> = HashMap::default();
        let mut check_if_chunk_below_threshold =
            |store: &Self, candidate_chunk_id: ChunkId| -> u64 {
                let ChunkStoreConfig {
                    enable_changelog: _,
                    chunk_max_bytes,
                    chunk_max_rows,
                    chunk_max_rows_if_unsorted,
                } = store.config;

                *candidates_below_threshold
                    .entry(candidate_chunk_id)
                    .or_insert_with(|| {
                        store
                            .chunks_per_chunk_id
                            .get(&candidate_chunk_id)
                            .map_or(0, |candidate| {
                                if chunk.id() == candidate_chunk_id {
                                    return 0;
                                }

                                if !chunk.concatenable(candidate) {
                                    return 0;
                                }

                                // Refuse the candidate if it descends from a split chunk, directly or indirectly.
                                // Compacting chunks coming from a split lineage is generally a mistake, as that is likely
                                // to lead to overlaps that weren't there in the first place.
                                if self.descends_from_a_split(&candidate_chunk_id) {
                                    return 0;
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

                                if is_below_bytes_threshold && is_below_rows_threshold {
                                    return candidate.num_rows() as u64;
                                }

                                0
                            })
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

            for (component, time_range) in time_range_per_component {
                let Some(temporal_chunk_ids_per_time) =
                    temporal_chunk_ids_per_component.get(&component)
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
                            *candidates.entry(chunk_id).or_default() +=
                                check_if_chunk_below_threshold(self, chunk_id);
                        }
                    }

                    // Direct neighbors (after): 1 point each.
                    if let Some((_data_time, chunk_id_set)) = temporal_chunk_ids_per_time
                        .per_start_time
                        .range(time_range.max().inc()..)
                        .next()
                    {
                        for &chunk_id in chunk_id_set {
                            *candidates.entry(chunk_id).or_default() +=
                                check_if_chunk_below_threshold(self, chunk_id);
                        }
                    }

                    // Shared start times: 2 points each.
                    {
                        let chunk_id_set = temporal_chunk_ids_per_time
                            .per_start_time
                            .get(&time_range.min());
                        for chunk_id in chunk_id_set.iter().flat_map(|set| set.iter().copied()) {
                            *candidates.entry(chunk_id).or_default() +=
                                check_if_chunk_below_threshold(self, chunk_id) * 2;
                        }
                    }
                }
            }
        }

        let mut candidates = candidates.into_iter().collect_vec();
        {
            re_tracing::profile_scope!("sort_candidates");
            candidates.sort_by_key(|(_chunk_id, points)| *points);
            candidates.reverse();
        }

        candidates
            .into_iter()
            .filter(|(_chunk_id, points)| *points > 0)
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
            config: _,
            time_type_registry: _,
            type_registry: _,
            per_column_metadata,
            chunks_per_chunk_id,
            chunks_lineage: _, // lineage metadata must never be dropped, regardless
            dangling_splits: _, // this counts as lineage metadata too
            leaky_compactions: _, // this counts as lineage metadata too
            chunk_ids_per_min_row_id,
            temporal_chunk_ids_per_entity_per_component,
            temporal_chunk_ids_per_entity,
            temporal_physical_chunks_stats,
            static_chunk_ids_per_entity,
            static_chunks_stats,
            queried_chunk_id_tracker: _,
            insert_id: _,
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

            for chunk_id in &dropped_static_chunk_ids {
                if let Some(min_row_id) = chunks_per_chunk_id
                    .get(chunk_id)
                    .and_then(|chunk| chunk.row_id_range().map(|(min, _)| min))
                {
                    chunk_ids_per_min_row_id.remove(&min_row_id);
                }
            }

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

            for chunk_id in &dropped_temporal_chunk_ids {
                if let Some(min_row_id) = chunks_per_chunk_id
                    .get(chunk_id)
                    .and_then(|chunk| chunk.row_id_range().map(|(min, _)| min))
                {
                    chunk_ids_per_min_row_id.remove(&min_row_id);
                }
            }

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
                *temporal_physical_chunks_stats -= ChunkStoreChunkStats::from_chunk(chunk);
            });

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

        if self.config.enable_changelog {
            Self::on_events(&events);
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use re_chunk::{TimeInt, TimePoint, Timeline};
    use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
    use re_log_types::{build_frame_nr, build_log_time};
    use re_sdk_types::components::Blob;
    use re_types_core::ComponentDescriptor;
    use similar_asserts::assert_eq;

    use super::*;

    // TODO(cmc): We could have more test coverage here, especially regarding thresholds etc.
    // For now the development and maintenance cost doesn't seem to be worth it.
    // We can re-assess later if things turns out to be shaky in practice.

    #[test]
    fn compaction_simple() -> anyhow::Result<()> {
        re_log::setup_logging();

        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
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
            .with_component_batches(
                row_id1,
                timepoint1,
                [(MyPoints::descriptor_points(), points1 as _)],
            )
            .with_component_batches(
                row_id2,
                timepoint2,
                [(MyPoints::descriptor_points(), points2 as _)],
            )
            .with_component_batches(
                row_id3,
                timepoint3,
                [(MyPoints::descriptor_points(), points3 as _)],
            )
            .build()?;
        let chunk2 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id4,
                timepoint4,
                [(MyPoints::descriptor_points(), points4 as _)],
            )
            .with_component_batches(
                row_id5,
                timepoint5,
                [(MyPoints::descriptor_points(), points5 as _)],
            )
            .build()?;
        let chunk3 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id6,
                timepoint1,
                [(MyPoints::descriptor_points(), points1 as _)],
            )
            .with_component_batches(
                row_id7,
                timepoint2,
                [(MyPoints::descriptor_points(), points2 as _)],
            )
            .with_component_batches(
                row_id8,
                timepoint3,
                [(MyPoints::descriptor_points(), points3 as _)],
            )
            .build()?;
        let chunk4 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id9,
                timepoint4,
                [(MyPoints::descriptor_points(), points4 as _)],
            )
            .with_component_batches(
                row_id10,
                timepoint5,
                [(MyPoints::descriptor_points(), points5 as _)],
            )
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
            .with_component_batches(
                row_id1,
                timepoint1,
                [(MyPoints::descriptor_points(), points1 as _)],
            )
            .with_component_batches(
                row_id2,
                timepoint2,
                [(MyPoints::descriptor_points(), points2 as _)],
            )
            .with_component_batches(
                row_id3,
                timepoint3,
                [(MyPoints::descriptor_points(), points3 as _)],
            )
            .with_component_batches(
                row_id4,
                timepoint4,
                [(MyPoints::descriptor_points(), points4 as _)],
            )
            .with_component_batches(
                row_id5,
                timepoint5,
                [(MyPoints::descriptor_points(), points5 as _)],
            )
            .with_component_batches(
                row_id6,
                timepoint1,
                [(MyPoints::descriptor_points(), points1 as _)],
            )
            .with_component_batches(
                row_id7,
                timepoint2,
                [(MyPoints::descriptor_points(), points2 as _)],
            )
            .with_component_batches(
                row_id8,
                timepoint3,
                [(MyPoints::descriptor_points(), points3 as _)],
            )
            .with_component_batches(
                row_id9,
                timepoint4,
                [(MyPoints::descriptor_points(), points4 as _)],
            )
            .with_component_batches(
                row_id10,
                timepoint5,
                [(MyPoints::descriptor_points(), points5 as _)],
            )
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
    fn no_components() -> anyhow::Result<()> {
        re_log::setup_logging();
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            Default::default(),
        );

        {
            let entity_path = EntityPath::from("/nothing-at-all");
            let chunk = Chunk::builder(entity_path.clone()).build()?;
            let chunk = Arc::new(chunk);

            let events = store.insert_chunk(&chunk)?;
            assert!(events.is_empty());
        }
        {
            let entity_path = EntityPath::from("/static-row-no-components");
            let chunk = Chunk::builder(entity_path.clone())
                .with_component_batches(RowId::new(), TimePoint::STATIC, [])
                .build()?;
            let chunk = Arc::new(chunk);

            let events = store.insert_chunk(&chunk)?;
            assert!(events.is_empty());
        }

        let timepoint_log = build_log_time(TimeInt::new_temporal(10).into());
        let timepoint_frame = build_frame_nr(123);

        {
            let entity_path = EntityPath::from("/log-time-row-no-components");
            let chunk = Chunk::builder(entity_path.clone())
                .with_component_batches(RowId::new(), [timepoint_log], [])
                .build()?;
            let chunk = Arc::new(chunk);

            let events = store.insert_chunk(&chunk)?;
            assert!(events.is_empty());
        }
        {
            let entity_path = EntityPath::from("/frame-nr-row-no-components");
            let chunk = Chunk::builder(entity_path.clone())
                .with_component_batches(RowId::new(), [timepoint_frame], [])
                .build()?;
            let chunk = Arc::new(chunk);

            let events = store.insert_chunk(&chunk)?;
            assert!(events.is_empty());
        }
        {
            let entity_path = EntityPath::from("/both-log-frame-row-no-components");
            let chunk = Chunk::builder(entity_path.clone())
                .with_component_batches(RowId::new(), [timepoint_log, timepoint_frame], [])
                .build()?;
            let chunk = Arc::new(chunk);

            let events = store.insert_chunk(&chunk)?;
            assert!(events.is_empty());
        }

        Ok(())
    }

    #[test]
    fn static_overwrites() -> anyhow::Result<()> {
        re_log::setup_logging();

        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            Default::default(),
        );

        let entity_path = EntityPath::from("this/that");

        let row_id1_1 = RowId::new();
        let row_id2_1 = RowId::new();
        let row_id2_2 = RowId::new();
        let row_id3_1 = RowId::new();

        let timepoint_static = TimePoint::STATIC;

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
                [
                    (MyPoints::descriptor_points(), points1 as _),
                    (MyPoints::descriptor_colors(), colors1 as _),
                    (MyPoints::descriptor_labels(), labels1 as _),
                ],
            )
            .build()?;
        let chunk2 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id2_1,
                timepoint_static.clone(),
                [
                    (MyPoints::descriptor_points(), points2 as _),
                    (MyPoints::descriptor_colors(), colors2 as _),
                ],
            )
            .build()?;
        let chunk3 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id2_2,
                timepoint_static.clone(),
                [(MyPoints::descriptor_labels(), labels2 as _)],
            )
            .build()?;
        let chunk4 = Chunk::builder(entity_path.clone())
            .with_component_batches(row_id3_1, timepoint_static.clone(), [])
            .build()?;

        let chunk1 = Arc::new(chunk1);
        let chunk2 = Arc::new(chunk2);
        let chunk3 = Arc::new(chunk3);
        let chunk4 = Arc::new(chunk4);

        let events = store.insert_chunk(&chunk1)?;
        assert!(
            events.len() == 1
                && events[0].delta_chunk().unwrap().id() == chunk1.id()
                && matches!(events[0].diff, ChunkStoreDiff::Addition(_)),
            "the first write should result in the addition of chunk1 and nothing else"
        );

        let events = store.insert_chunk(&chunk2)?;
        assert!(
            events.len() == 1
                && events[0].delta_chunk().unwrap().id() == chunk2.id()
                && matches!(events[0].diff, ChunkStoreDiff::Addition(_)),
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
                && events[0].delta_chunk().unwrap().id() == chunk3.id()
                && matches!(events[0].diff, ChunkStoreDiff::Addition(_))
                && events[1].delta_chunk().unwrap().id() == chunk1.id()
                && matches!(events[1].diff, ChunkStoreDiff::Deletion(_)),
            "the third write should result in the addition of chunk3 _and_ the deletion of the now fully overwritten chunk1"
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

        let events = store.insert_chunk(&chunk4)?;
        assert!(
            events.is_empty(),
            "the fourth write should result in no changes at all"
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

    #[test]
    fn row_id_min_overwrites() -> anyhow::Result<()> {
        re_log::setup_logging();

        let entity_path = EntityPath::from("this/that");

        let timepoint = TimePoint::default().with(Timeline::log_tick(), 42);

        let row_id1_1 = RowId::new();
        let row_id2_1 = RowId::new();

        let labels1 = &[MyLabel("111".to_owned())];
        let labels2 = &[MyLabel("222".to_owned())];

        let chunk1 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id1_1,
                timepoint.clone(),
                [(MyPoints::descriptor_labels(), labels1 as _)],
            )
            .build()?;
        let chunk2 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id2_1,
                timepoint.clone(),
                [(MyPoints::descriptor_labels(), labels2 as _)],
            )
            .build()?;

        let chunk1 = Arc::new(chunk1);
        let chunk2 = Arc::new(chunk2);

        fn assert_chunk_ids_per_min_row_id(
            store: &ChunkStore,
            chunks: impl IntoIterator<Item = (RowId, ChunkId)>,
        ) {
            assert_eq!(
                chunks.into_iter().collect::<BTreeMap<_, _>>(),
                store.chunk_ids_per_min_row_id
            );
        }

        {
            // Insert `chunk1` then `chunk2`.

            let mut store = ChunkStore::new(
                re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
                ChunkStoreConfig {
                    enable_changelog: false,
                    chunk_max_bytes: u64::MAX,
                    chunk_max_rows: u64::MAX,
                    chunk_max_rows_if_unsorted: u64::MAX,
                },
            );

            let _ = store.insert_chunk(&chunk1)?;
            assert_chunk_ids_per_min_row_id(&store, [(row_id1_1, chunk1.id())]);

            let _ = store.insert_chunk(&chunk1)?; // noop
            assert_chunk_ids_per_min_row_id(&store, [(row_id1_1, chunk1.id())]);

            // `chunk2` gets appended to `chunk1`:
            // * the only Row ID left is `row_id1_1`
            // * there shouldn't be any warning of any kind
            // * the only chunk left in the store is the new, compacted chunk
            let _ = store.insert_chunk(&chunk2)?;
            assert_eq!(1, store.chunks_per_chunk_id.len());
            let compacted_chunk_id = store.chunks_per_chunk_id.values().next().unwrap().id();
            assert_chunk_ids_per_min_row_id(&store, [(row_id1_1, compacted_chunk_id)]);
        }

        {
            // Insert `chunk2` then `chunk1`.

            let mut store = ChunkStore::new(
                re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
                ChunkStoreConfig {
                    enable_changelog: false,
                    chunk_max_bytes: u64::MAX,
                    chunk_max_rows: u64::MAX,
                    chunk_max_rows_if_unsorted: u64::MAX,
                },
            );

            let _ = store.insert_chunk(&chunk2)?;
            assert_chunk_ids_per_min_row_id(&store, [(row_id2_1, chunk2.id())]);

            let _ = store.insert_chunk(&chunk2)?; // noop
            assert_chunk_ids_per_min_row_id(&store, [(row_id2_1, chunk2.id())]);

            // Exactly the same as before, because chunks get compacted in Row ID order, regardless
            // of the order they are inserted in.
            //
            // `chunk2` gets appended to `chunk1`:
            // * the only Row ID left is `row_id1_1`
            // * there shouldn't be any warning of any kind
            // * the only chunk left in the store is the new, compacted chunk
            let _ = store.insert_chunk(&chunk1)?;
            assert_eq!(1, store.chunks_per_chunk_id.len());
            let compacted_chunk_id = store.chunks_per_chunk_id.values().next().unwrap().id();
            assert_chunk_ids_per_min_row_id(&store, [(row_id1_1, compacted_chunk_id)]);
        }

        Ok(())
    }

    #[test]
    fn compaction_blobs() -> anyhow::Result<()> {
        #![expect(clippy::cloned_ref_to_slice_refs)]

        re_log::setup_logging();

        // Create a store with a specific byte limit for testing
        // Default chunk_max_bytes is 12 * 8 * 4096 = 393,216 bytes
        let chunk_max_bytes = 300_000u64; // 300KB limit for easier testing
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            ChunkStoreConfig {
                chunk_max_bytes,
                ..Default::default()
            },
        );

        let entity_path = EntityPath::from("blob/data");

        // Calculate blob sizes relative to the limit
        let blob_size_1_3rd = (chunk_max_bytes / 3) as usize; // ~100KB
        let blob_size_1_2nd = (chunk_max_bytes / 2) as usize; // ~150KB

        // Create test data
        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();

        let timepoint1 = [(Timeline::new_sequence("frame"), 1)];
        let timepoint2 = [(Timeline::new_sequence("frame"), 2)];
        let timepoint3 = [(Timeline::new_sequence("frame"), 3)];
        let timepoint4 = [(Timeline::new_sequence("frame"), 4)];
        let timepoint5 = [(Timeline::new_sequence("frame"), 5)];

        // Create blobs of different sizes
        let blob1 = Blob::from(vec![1u8; blob_size_1_3rd]); // 1/3 limit
        let blob2 = Blob::from(vec![2u8; blob_size_1_2nd]); // 1/2 limit
        let blob3 = Blob::from(vec![3u8; blob_size_1_2nd]); // 1/2 limit
        let blob4 = Blob::from(vec![4u8; blob_size_1_2nd]); // 1/2 limit
        let blob5 = Blob::from(vec![5u8; blob_size_1_3rd]); // 1/3 limit

        // Create a simple descriptor for blob components
        let blob_descriptor = ComponentDescriptor::partial("blob");

        // Create chunks according to the pattern:
        // 1. Chunk with blob 1/3rd the limit
        let chunk1 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id1,
                timepoint1,
                [(
                    blob_descriptor.clone(),
                    &[blob1.clone()] as &dyn re_types_core::ComponentBatch,
                )],
            )
            .build()?;

        // 2. Chunk with three blobs 1/2 the limit (will be split across multiple chunks)
        let chunk2 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id2,
                timepoint2,
                [(
                    blob_descriptor.clone(),
                    &[blob2.clone()] as &dyn re_types_core::ComponentBatch,
                )],
            )
            .with_component_batches(
                row_id3,
                timepoint3,
                [(
                    blob_descriptor.clone(),
                    &[blob3.clone()] as &dyn re_types_core::ComponentBatch,
                )],
            )
            .with_component_batches(
                row_id4,
                timepoint4,
                [(
                    blob_descriptor.clone(),
                    &[blob4.clone()] as &dyn re_types_core::ComponentBatch,
                )],
            )
            .build()?;

        // 3. Chunk with blob 1/3rd the limit
        let chunk3 = Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id5,
                timepoint5,
                [(
                    blob_descriptor.clone(),
                    &[blob5.clone()] as &dyn re_types_core::ComponentBatch,
                )],
            )
            .build()?;

        let chunk1 = Arc::new(chunk1);
        let chunk2 = Arc::new(chunk2);
        let chunk3 = Arc::new(chunk3);

        eprintln!(
            "Inserting chunk1 (blob 1/3 limit: {} bytes)",
            <Chunk as SizeBytes>::total_size_bytes(&chunk1),
        );
        store.insert_chunk(&chunk1)?;
        eprintln!("Store has {} chunks", store.chunks_per_chunk_id.len());

        eprintln!(
            "Inserting chunk2 (3 blobs 1/2 limit each: {} bytes)",
            <Chunk as SizeBytes>::total_size_bytes(&chunk2),
        );
        store.insert_chunk(&chunk2)?;
        eprintln!("Store has {} chunks", store.chunks_per_chunk_id.len());

        eprintln!(
            "Inserting chunk3 (blob 1/3 limit: {} bytes)",
            <Chunk as SizeBytes>::total_size_bytes(&chunk3),
        );
        store.insert_chunk(&chunk3)?;
        eprintln!("Store has {} chunks", store.chunks_per_chunk_id.len());

        // Verify the expected compaction results:
        // Expected:
        // - The first chunk was left untouched.
        // - The second chunk was split into 3 smaller chunks.
        // - The third chunk was left untouched.
        // So we expect 5 chunks total.

        eprintln!("Final store state:");
        eprintln!("{store}");

        // Check that we have the expected number of chunks after compaction
        assert_eq!(
            5,
            store.chunks_per_chunk_id.len(),
            "Expected 4 chunks after compaction: [blob1], [blob2], [blob3], [blob4], [blob5]"
        );

        // Verify the chunks contain the expected data by checking their sizes
        let mut chunk_sizes: Vec<_> = store
            .chunks_per_chunk_id
            .values()
            .map(|chunk| <Chunk as SizeBytes>::total_size_bytes(chunk))
            .collect();
        chunk_sizes.sort();

        eprintln!("Chunk sizes: {chunk_sizes:?}");

        let smallest_expected = <Chunk as SizeBytes>::total_size_bytes(&chunk1);
        let largest_expected = <Chunk as SizeBytes>::total_size_bytes(&chunk2) / 3;

        // Allow some tolerance for metadata overhead
        let tolerance = 10_000u64; // 10KB tolerance

        for &chunk_size in &chunk_sizes[0..2] {
            assert!(
                chunk_size >= smallest_expected.saturating_sub(tolerance)
                    && chunk_size <= smallest_expected + tolerance,
                "Smallest chunk size {chunk_size} should be around {smallest_expected} ± {tolerance}",
            );
        }

        for &chunk_size in &chunk_sizes[2..] {
            assert!(
                chunk_size >= largest_expected.saturating_sub(tolerance)
                    && chunk_size <= largest_expected + tolerance,
                "Largest chunk size {chunk_size} should be around {largest_expected} ± {tolerance}",
            );
        }

        Ok(())
    }
}
