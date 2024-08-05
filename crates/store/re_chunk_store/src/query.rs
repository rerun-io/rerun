use std::{
    collections::BTreeSet,
    sync::{atomic::Ordering, Arc},
};

use itertools::Itertools;
use re_chunk::{Chunk, LatestAtQuery, RangeQuery};
use re_log_types::ResolvedTimeRange;
use re_log_types::{EntityPath, TimeInt, Timeline};
use re_types_core::SizeBytes as _;
use re_types_core::{ComponentName, ComponentNameSet};

use crate::{store::ChunkIdSetPerTime, ChunkStore};

// Used all over in docstrings.
#[allow(unused_imports)]
use crate::RowId;

// ---

// These APIs often have `temporal` and `static` variants.
// It is sometimes useful to be able to separately query either,
// such as when we want to tell the user that they logged a component
// as both static and temporal, which is probably wrong.

impl ChunkStore {
    /// Retrieve all the [`ComponentName`]s that have been written to for a given [`EntityPath`] on
    /// the specified [`Timeline`].
    ///
    /// Static components are always included in the results.
    ///
    /// Returns `None` if the entity doesn't exist at all on this `timeline`.
    pub fn all_components_on_timeline(
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

    /// Retrieve all the [`ComponentName`]s that have been written to for a given [`EntityPath`].
    ///
    /// Static components are always included in the results.
    ///
    /// Returns `None` if the entity has never had any data logged to it.
    pub fn all_components(&self, entity_path: &EntityPath) -> Option<ComponentNameSet> {
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

    /// Check whether an entity has a static component or a temporal component on the specified timeline.
    ///
    /// This does _not_ check if the entity actually currently holds any data for that component.
    #[inline]
    pub fn entity_has_component_on_timeline(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
        component_name: &ComponentName,
    ) -> bool {
        re_tracing::profile_function!();

        self.entity_has_static_component(entity_path, component_name)
            || self.entity_has_temporal_component_on_timeline(timeline, entity_path, component_name)
    }

    /// Check whether an entity has a static component or a temporal component on any timeline.
    ///
    /// This does _not_ check if the entity actually currently holds any data for that component.
    pub fn entity_has_component(
        &self,
        entity_path: &EntityPath,
        component_name: &ComponentName,
    ) -> bool {
        re_tracing::profile_function!();

        self.entity_has_static_component(entity_path, component_name)
            || self.entity_has_temporal_component(entity_path, component_name)
    }

    /// Check whether an entity has a specific static component.
    ///
    /// This does _not_ check if the entity actually currently holds any data for that component.
    #[inline]
    pub fn entity_has_static_component(
        &self,
        entity_path: &EntityPath,
        component_name: &ComponentName,
    ) -> bool {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        self.static_chunk_ids_per_entity
            .get(entity_path)
            .is_some_and(|static_chunk_ids_per_component| {
                static_chunk_ids_per_component.contains_key(component_name)
            })
    }

    /// Check whether an entity has a temporal component on any timeline.
    ///
    /// This does _not_ check if the entity actually currently holds any data for that component.
    #[inline]
    pub fn entity_has_temporal_component(
        &self,
        entity_path: &EntityPath,
        component_name: &ComponentName,
    ) -> bool {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        self.temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .iter()
            .flat_map(|temporal_chunk_ids_per_timeline| temporal_chunk_ids_per_timeline.values())
            .any(|temporal_chunk_ids_per_component| {
                temporal_chunk_ids_per_component.contains_key(component_name)
            })
    }

    /// Check whether an entity has a temporal component on a specific timeline.
    ///
    /// This does _not_ check if the entity actually currently holds any data for that component.
    #[inline]
    pub fn entity_has_temporal_component_on_timeline(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
        component_name: &ComponentName,
    ) -> bool {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        self.temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .iter()
            .filter_map(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline.get(timeline)
            })
            .any(|temporal_chunk_ids_per_component| {
                temporal_chunk_ids_per_component.contains_key(component_name)
            })
    }

    /// Check whether an entity has any data on a specific timeline, or any static data.
    ///
    /// This is different from checking if the entity has any component, it also ensures
    /// that some _data_ currently exists in the store for this entity.
    #[inline]
    pub fn entity_has_data_on_timeline(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
    ) -> bool {
        re_tracing::profile_function!();

        self.entity_has_static_data(entity_path)
            || self.entity_has_temporal_data_on_timeline(timeline, entity_path)
    }

    /// Check whether an entity has any static data or any temporal data on any timeline.
    ///
    /// This is different from checking if the entity has any component, it also ensures
    /// that some _data_ currently exists in the store for this entity.
    #[inline]
    pub fn entity_has_data(&self, entity_path: &EntityPath) -> bool {
        re_tracing::profile_function!();

        self.entity_has_static_data(entity_path) || self.entity_has_temporal_data(entity_path)
    }

    /// Check whether an entity has any static data.
    ///
    /// This is different from checking if the entity has any component, it also ensures
    /// that some _data_ currently exists in the store for this entity.
    #[inline]
    pub fn entity_has_static_data(&self, entity_path: &EntityPath) -> bool {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        self.static_chunk_ids_per_entity
            .get(entity_path)
            .is_some_and(|static_chunk_ids_per_component| {
                static_chunk_ids_per_component
                    .values()
                    .any(|chunk_id| self.chunks_per_chunk_id.contains_key(chunk_id))
            })
    }

    /// Check whether an entity has any temporal data.
    ///
    /// This is different from checking if the entity has any component, it also ensures
    /// that some _data_ currently exists in the store for this entity.
    #[inline]
    pub fn entity_has_temporal_data(&self, entity_path: &EntityPath) -> bool {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        self.temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .is_some_and(|temporal_chunks_per_timeline| {
                temporal_chunks_per_timeline
                    .values()
                    .flat_map(|temporal_chunks_per_component| {
                        temporal_chunks_per_component.values()
                    })
                    .flat_map(|chunk_id_sets| chunk_id_sets.per_start_time.values())
                    .flat_map(|chunk_id_set| chunk_id_set.iter())
                    .any(|chunk_id| self.chunks_per_chunk_id.contains_key(chunk_id))
            })
    }

    /// Check whether an entity has any temporal data.
    ///
    /// This is different from checking if the entity has any component, it also ensures
    /// that some _data_ currently exists in the store for this entity.
    #[inline]
    pub fn entity_has_temporal_data_on_timeline(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
    ) -> bool {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        self.temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .and_then(|temporal_chunks_per_timeline| temporal_chunks_per_timeline.get(timeline))
            .is_some_and(|temporal_chunks_per_component| {
                temporal_chunks_per_component
                    .values()
                    .flat_map(|chunk_id_sets| chunk_id_sets.per_start_time.values())
                    .flat_map(|chunk_id_set| chunk_id_set.iter())
                    .any(|chunk_id| self.chunks_per_chunk_id.contains_key(chunk_id))
            })
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

    /// Returns the min and max times at which data was logged for an entity on a specific timeline.
    ///
    /// This ignores static data.
    pub fn entity_time_range(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
    ) -> Option<ResolvedTimeRange> {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        let temporal_chunk_ids_per_timeline =
            self.temporal_chunk_ids_per_entity.get(entity_path)?;
        let chunk_id_sets = temporal_chunk_ids_per_timeline.get(timeline)?;

        let start = chunk_id_sets.per_start_time.first_key_value()?.0;
        let end = chunk_id_sets.per_end_time.last_key_value()?.0;

        Some(ResolvedTimeRange::new(*start, *end))
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

// Counting
impl ChunkStore {
    /// Returns the number of temporal events logged for an entity on a specific timeline.
    ///
    /// This ignores static data.
    pub fn num_temporal_events_on_timeline(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
    ) -> u64 {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        self.temporal_chunk_ids_per_entity
            .get(entity_path)
            .and_then(|temporal_chunks_events_per_timeline| {
                temporal_chunks_events_per_timeline.get(timeline)
            })
            .map_or(0, |chunk_id_sets| {
                chunk_id_sets
                    .per_start_time
                    .values()
                    .flat_map(|chunk_ids| {
                        chunk_ids
                            .iter()
                            .filter_map(|chunk_id| self.chunks_per_chunk_id.get(chunk_id))
                            .map(|chunk| chunk.num_events_cumulative())
                    })
                    .sum()
            })
    }

    /// Returns the number of static events logged for an entity for a specific component.
    ///
    /// This ignores temporal events.
    pub fn num_static_events_for_component(
        &self,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> u64 {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        self.static_chunk_ids_per_entity
            .get(entity_path)
            .and_then(|static_chunks_per_component| {
                static_chunks_per_component.get(&component_name)
            })
            .and_then(|chunk_id| self.chunks_per_chunk_id.get(chunk_id))
            .and_then(|chunk| chunk.num_events_for_component(component_name))
            .unwrap_or(0)
    }

    /// Returns the number of temporal events logged for an entity for a specific component on a given timeline.
    ///
    /// This ignores static events.
    pub fn num_temporal_events_for_component_on_timeline(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
        component_name: ComponentName,
    ) -> u64 {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        self.temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .and_then(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline.get(timeline)
            })
            .and_then(|temporal_chunk_ids_per_component| {
                temporal_chunk_ids_per_component.get(&component_name)
            })
            .map_or(0, |chunk_id_sets| {
                chunk_id_sets
                    .per_start_time
                    .values()
                    .flat_map(|chunk_ids| chunk_ids.iter())
                    .filter_map(|chunk_id| self.chunks_per_chunk_id.get(chunk_id))
                    .filter_map(|chunk| chunk.num_events_for_component(component_name))
                    .sum()
            })
    }

    /// Returns number of bytes used for an entity on a specific timeline.
    ///
    /// This always includes static data.
    /// This is an approximation of the actual storage cost of the entity,
    /// as the measurement includes the overhead of various data structures
    /// we use in the database.
    /// It is imprecise, because it does not account for every possible place
    /// someone may be storing something related to the entity, only most of
    /// what is accessible inside this chunk store.
    ///
    /// ⚠ This does not return the _total_ size of the entity and all its children!
    /// For that, use `entity_db.approx_size_of_subtree_on_timeline`.
    pub fn approx_size_of_entity_on_timeline(
        &self,
        timeline: &Timeline,
        entity_path: &EntityPath,
    ) -> u64 {
        re_tracing::profile_function!();

        self.query_id.fetch_add(1, Ordering::Relaxed);

        let static_data_size_bytes = self.static_chunk_ids_per_entity.get(entity_path).map_or(
            0,
            |static_chunks_per_component| {
                static_chunks_per_component
                    .values()
                    .filter_map(|id| self.chunks_per_chunk_id.get(id))
                    .map(|chunk| Chunk::total_size_bytes(chunk))
                    .sum()
            },
        );

        let temporal_data_size_bytes = self
            .temporal_chunk_ids_per_entity
            .get(entity_path)
            .and_then(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline.get(timeline)
            })
            .map_or(0, |chunk_id_sets| {
                chunk_id_sets
                    .per_start_time
                    .values()
                    .flat_map(|chunk_ids| chunk_ids.iter())
                    .filter_map(|id| self.chunks_per_chunk_id.get(id))
                    .map(|chunk| Chunk::total_size_bytes(chunk))
                    .sum()
            });

        static_data_size_bytes + temporal_data_size_bytes
    }
}
