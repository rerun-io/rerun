use std::{
    collections::BTreeSet,
    sync::{atomic::Ordering, Arc},
};

use itertools::Itertools;
use re_chunk::{Chunk, LatestAtQuery, RangeQuery};
use re_log_types::{EntityPath, TimeInt, Timeline};
use re_types_core::{ComponentName, ComponentNameSet};

use crate::{store::ChunkIdSetPerTime, ChunkStore};

// Used all over in docstrings.
#[allow(unused_imports)]
use crate::RowId;

// ---

impl ChunkStore {
    /// Retrieve all the [`ComponentName`]s that have been written to for a given [`EntityPath`] on
    /// the specified [`Timeline`].
    ///
    /// Static components are always included in the results.
    ///
    /// Returns `None` if the entity doesn't exist at all on this `timeline`.
    pub fn all_components(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
    ) -> Option<ComponentNameSet> {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        let static_components: Option<ComponentNameSet> = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .map(|static_chunks_per_component| {
                static_chunks_per_component.keys().copied().collect()
            });

        let temporal_components: Option<ComponentNameSet> = self
            .temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .map(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline
                    .iter()
                    .filter(|(cur_timeline, _)| *cur_timeline == timeline)
                    .flat_map(|(_, temporal_chunk_ids_per_component)| {
                        temporal_chunk_ids_per_component.keys().copied()
                    })
                    .collect()
            });

        match (static_components, temporal_components) {
            (None, None) => None,
            (None, comps @ Some(_)) | (comps @ Some(_), None) => comps,
            (Some(static_comps), Some(temporal_comps)) => {
                Some(static_comps.into_iter().chain(temporal_comps).collect())
            }
        }
    }

    /// Check whether a given entity has a specific [`ComponentName`] either on the specified
    /// timeline, or in its static data.
    #[inline]
    pub fn entity_has_component(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
        component_name: &ComponentName,
    ) -> bool {
        re_tracing::profile_function!();
        self.all_components(timeline, entity_path)
            .map_or(false, |components| components.contains(component_name))
    }

    /// Find the earliest time at which something was logged for a given entity on the specified
    /// timeline.
    ///
    /// Ignores static data.
    #[inline]
    pub fn entity_min_time(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
    ) -> Option<TimeInt> {
        let temporal_chunk_ids_per_timeline = self
            .temporal_chunk_ids_per_entity_per_component
            .get(entity_path)?;
        let temporal_chunk_ids_per_component = temporal_chunk_ids_per_timeline.get(timeline)?;

        let mut time_min = TimeInt::MAX;
        for temporal_chunk_ids_per_time in temporal_chunk_ids_per_component.values() {
            let Some(time) = temporal_chunk_ids_per_time
                .per_start_time
                .first_key_value()
                .map(|(time, _)| *time)
            else {
                continue;
            };
            time_min = TimeInt::min(time_min, time);
        }

        (time_min != TimeInt::MAX).then_some(time_min)
    }
}

// LatestAt
impl ChunkStore {
    /// Returns the most-relevant chunk(s) for the given [`LatestAtQuery`] and [`ComponentName`].
    ///
    /// The returned vector is guaranteed free of duplicates, by definition.
    ///
    /// The [`ChunkStore`] always work at the [`Chunk`] level (as opposed to the row level): it is
    /// oblivious to the data therein.
    /// For that reason, and because [`Chunk`]s are allowed to temporally overlap, it is possible
    /// that a query has more than one relevant chunk.
    ///
    /// The caller should filter the returned chunks further (see [`Chunk::latest_at`]) in order to
    /// determine what exact row contains the final result.
    ///
    /// If the entity has static component data associated with it, it will unconditionally
    /// override any temporal component data.
    pub fn latest_at_relevant_chunks(
        &self,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> Vec<Arc<Chunk>> {
        re_tracing::profile_function!(format!("{query:?}"));

        self.query_id.fetch_add(1, Ordering::Relaxed);

        // Reminder: if a chunk has been indexed for a given component, then it must contain at
        // least one non-null value for that column.

        if let Some(static_chunk) = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .and_then(|static_chunks_per_component| {
                static_chunks_per_component.get(&component_name)
            })
            .and_then(|chunk_id| self.chunks_per_chunk_id.get(chunk_id))
        {
            return vec![Arc::clone(static_chunk)];
        }

        let chunks = self
            .temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .and_then(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline.get(&query.timeline())
            })
            .and_then(|temporal_chunk_ids_per_component| {
                temporal_chunk_ids_per_component.get(&component_name)
            })
            .and_then(|temporal_chunk_ids_per_time| {
                self.latest_at(query, temporal_chunk_ids_per_time)
            })
            .unwrap_or_default();

        debug_assert!(chunks.iter().map(|chunk| chunk.id()).all_unique());

        chunks
    }

    /// Returns the most-relevant _temporal_ chunk(s) for the given [`LatestAtQuery`].
    ///
    /// The returned vector is guaranteed free of duplicates, by definition.
    ///
    /// The [`ChunkStore`] always work at the [`Chunk`] level (as opposed to the row level): it is
    /// oblivious to the data therein.
    /// For that reason, and because [`Chunk`]s are allowed to temporally overlap, it is possible
    /// that a query has more than one relevant chunk.
    ///
    /// The caller should filter the returned chunks further (see [`Chunk::latest_at`]) in order to
    /// determine what exact row contains the final result.
    ///
    /// **This ignores static data.**
    pub fn latest_at_relevant_chunks_for_all_components(
        &self,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
    ) -> Vec<Arc<Chunk>> {
        re_tracing::profile_function!(format!("{query:?}"));

        self.query_id.fetch_add(1, Ordering::Relaxed);

        let chunks = self
            .temporal_chunk_ids_per_entity
            .get(entity_path)
            .and_then(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline.get(&query.timeline())
            })
            .and_then(|temporal_chunk_ids_per_time| {
                self.latest_at(query, temporal_chunk_ids_per_time)
            })
            .unwrap_or_default();

        debug_assert!(chunks.iter().map(|chunk| chunk.id()).all_unique());

        chunks
    }

    fn latest_at(
        &self,
        query: &LatestAtQuery,
        temporal_chunk_ids_per_time: &ChunkIdSetPerTime,
    ) -> Option<Vec<Arc<Chunk>>> {
        re_tracing::profile_function!();

        let upper_bound = temporal_chunk_ids_per_time
            .per_start_time
            .range(..=query.at())
            .next_back()
            .map(|(time, _)| *time)?;

        // Overlapped chunks
        // =================
        //
        // To deal with potentially overlapping chunks, we keep track of the longest
        // interval in the entire map, which gives us an upper bound on how much we
        // would need to walk backwards in order to find all potential overlaps.
        //
        // This is a fairly simple solution that scales much better than interval-tree
        // based alternatives, both in terms of complexity and performance, in the normal
        // case where most chunks in a collection have similar lengths.
        //
        // The most degenerate case -- a single chunk overlaps everything else -- results
        // in `O(n)` performance, which gets amortized by the query cache.
        // If that turns out to be a problem in practice, we can experiment with more
        // complex solutions then.
        let lower_bound = upper_bound
            .as_i64()
            .saturating_sub(temporal_chunk_ids_per_time.max_interval_length as _);

        let temporal_chunk_ids = temporal_chunk_ids_per_time
            .per_start_time
            .range(..=query.at())
            .rev()
            .take_while(|(time, _)| time.as_i64() >= lower_bound)
            .flat_map(|(_time, chunk_ids)| chunk_ids.iter())
            .copied()
            .collect::<BTreeSet<_>>();

        Some(
            temporal_chunk_ids
                .iter()
                .filter_map(|chunk_id| self.chunks_per_chunk_id.get(chunk_id).cloned())
                .collect(),
        )
    }
}

// Range
impl ChunkStore {
    /// Returns the most-relevant chunk(s) for the given [`RangeQuery`] and [`ComponentName`].
    ///
    /// The returned vector is guaranteed free of duplicates, by definition.
    ///
    /// The criteria for returning a chunk is only that it may contain data that overlaps with
    /// the queried range.
    ///
    /// The caller should filter the returned chunks further (see [`Chunk::range`]) in order to
    /// determine how exactly each row of data fit with the rest.
    ///
    /// If the entity has static component data associated with it, it will unconditionally
    /// override any temporal component data.
    pub fn range_relevant_chunks(
        &self,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> Vec<Arc<Chunk>> {
        re_tracing::profile_function!(format!("{query:?}"));

        self.query_id.fetch_add(1, Ordering::Relaxed);

        if let Some(static_chunk) = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .and_then(|static_chunks_per_component| {
                static_chunks_per_component.get(&component_name)
            })
            .and_then(|chunk_id| self.chunks_per_chunk_id.get(chunk_id))
        {
            return vec![Arc::clone(static_chunk)];
        }

        let chunks = self
            .range(
                query,
                self.temporal_chunk_ids_per_entity_per_component
                    .get(entity_path)
                    .and_then(|temporal_chunk_ids_per_timeline| {
                        temporal_chunk_ids_per_timeline.get(&query.timeline())
                    })
                    .and_then(|temporal_chunk_ids_per_component| {
                        temporal_chunk_ids_per_component.get(&component_name)
                    })
                    .into_iter(),
            )
            .into_iter()
            // Post-processing: `Self::range` doesn't have access to the chunk metadata, so now we
            // need to make sure that the resulting chunks' per-component time range intersects with the
            // time range of the query itself.
            .filter(|chunk| {
                chunk
                    .timelines()
                    .get(&query.timeline())
                    .map_or(false, |time_chunk| {
                        time_chunk
                            .time_range_per_component(chunk.components())
                            .get(&component_name)
                            .map_or(false, |time_range| time_range.intersects(query.range()))
                    })
            })
            .collect_vec();

        debug_assert!(chunks.iter().map(|chunk| chunk.id()).all_unique());

        chunks
    }

    /// Returns the most-relevant _temporal_ chunk(s) for the given [`RangeQuery`].
    ///
    /// The returned vector is guaranteed free of duplicates, by definition.
    ///
    /// The criteria for returning a chunk is only that it may contain data that overlaps with
    /// the queried range.
    ///
    /// The caller should filter the returned chunks further (see [`Chunk::range`]) in order to
    /// determine how exactly each row of data fit with the rest.
    ///
    /// **This ignores static data.**
    pub fn range_relevant_chunks_for_all_components(
        &self,
        query: &RangeQuery,
        entity_path: &EntityPath,
    ) -> Vec<Arc<Chunk>> {
        re_tracing::profile_function!(format!("{query:?}"));

        self.query_id.fetch_add(1, Ordering::Relaxed);

        let chunks = self
            .range(
                query,
                self.temporal_chunk_ids_per_entity
                    .get(entity_path)
                    .and_then(|temporal_chunk_ids_per_timeline| {
                        temporal_chunk_ids_per_timeline.get(&query.timeline())
                    })
                    .into_iter(),
            )
            .into_iter()
            // Post-processing: `Self::range` doesn't have access to the chunk metadata, so now we
            // need to make sure that the resulting chunks' global time ranges intersect with the
            // time range of the query itself.
            .filter(|chunk| {
                chunk
                    .timelines()
                    .get(&query.timeline())
                    .map_or(false, |time_chunk| {
                        time_chunk.time_range().intersects(query.range())
                    })
            })
            .collect_vec();

        debug_assert!(chunks.iter().map(|chunk| chunk.id()).all_unique());

        chunks
    }

    fn range<'a>(
        &'a self,
        query: &RangeQuery,
        temporal_chunk_ids_per_times: impl Iterator<Item = &'a ChunkIdSetPerTime>,
    ) -> Vec<Arc<Chunk>> {
        re_tracing::profile_function!();

        temporal_chunk_ids_per_times
            .map(|temporal_chunk_ids_per_time| {
                let start_time = temporal_chunk_ids_per_time
                    .per_start_time
                    .range(..=query.range.min())
                    .next_back()
                    .map_or(TimeInt::MIN, |(&time, _)| time);

                let end_time = temporal_chunk_ids_per_time
                    .per_start_time
                    .range(..=query.range.max())
                    .next_back()
                    .map_or(start_time, |(&time, _)| time);

                // NOTE: Just being extra cautious because, even though this shouldnt possibly ever happen,
                // indexing a std map with a backwards range is an instant crash.
                let end_time = TimeInt::max(start_time, end_time);

                (start_time, end_time, temporal_chunk_ids_per_time)
            })
            .flat_map(|(start_time, end_time, temporal_chunk_ids_per_time)| {
                temporal_chunk_ids_per_time
                    .per_start_time
                    .range(start_time..=end_time)
                    .map(|(_time, chunk_ids)| chunk_ids)
            })
            .flat_map(|temporal_chunk_ids| {
                temporal_chunk_ids
                    .iter()
                    .filter_map(|chunk_id| self.chunks_per_chunk_id.get(chunk_id).cloned())
            })
            .collect()
    }
}
