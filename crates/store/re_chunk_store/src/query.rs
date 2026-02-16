use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use itertools::{Either, Itertools as _};
use nohash_hasher::IntSet;
use re_log::debug_assert;
use saturating_cast::SaturatingCast as _;

use re_chunk::{Chunk, ChunkId, ComponentIdentifier, LatestAtQuery, RangeQuery, TimelineName};
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt, Timeline};
use re_types_core::{ComponentDescriptor, ComponentSet, UnorderedComponentSet};

use crate::{ChunkStore, ChunkTrackingMode};
// Used all over in docstrings.
#[expect(unused_imports)]
use crate::RowId;
use crate::store::ChunkIdSetPerTime;

// ---

// These APIs often have `temporal` and `static` variants.
// It is sometimes useful to be able to separately query either,
// such as when we want to tell the user that they logged a component
// as both static and temporal, which is probably wrong.

// Meta queries
impl ChunkStore {
    /// Retrieve all [`Timeline`]s in the store.
    #[inline]
    pub fn timelines(&self) -> BTreeMap<TimelineName, Timeline> {
        self.time_type_registry
            .iter()
            .map(|(name, typ)| (*name, Timeline::new(*name, *typ)))
            .collect()
    }

    /// Retrieve all [`EntityPath`]s in the store.
    #[inline]
    pub fn all_entities(&self) -> IntSet<EntityPath> {
        self.static_chunk_ids_per_entity
            .keys()
            .cloned()
            .chain(self.temporal_chunk_ids_per_entity.keys().cloned())
            .collect()
    }

    /// Returns a vector with all the chunks in this store, sorted in descending order relative to
    /// their distance from the given `(timeline, time)` cursor.
    pub fn find_temporal_chunks_furthest_from(
        &self,
        timeline: &TimelineName,
        time: TimeInt,
    ) -> Vec<Arc<Chunk>> {
        re_tracing::profile_function!();

        self.chunks_per_chunk_id
            .values()
            .filter_map(|chunk| {
                let times = chunk.timelines().get(timeline)?;

                let min_dist = if times.is_sorted() {
                    let pivot = times.times_raw().partition_point(|t| *t < time.as_i64());
                    let min_value1 = times
                        .times_raw()
                        .get(pivot.saturating_sub(1))
                        .map(|t| t.abs_diff(time.as_i64()));
                    let min_value2 = times
                        .times_raw()
                        .get(pivot)
                        .map(|t| t.abs_diff(time.as_i64()));

                    // NOTE: Do *not* compare these options directly, if any of them turns out to
                    // be None, it'll be a disaster.
                    // min_value1.min(min_value2);
                    [min_value1, min_value2].into_iter().flatten().min()
                } else {
                    times
                        .times()
                        .map(|t| t.as_i64().abs_diff(time.as_i64()))
                        .min()
                };

                min_dist.map(|max_dist| (chunk, max_dist))
            })
            .sorted_by(|(_chunk1, dist1), (_chunk2, dist2)| std::cmp::Ord::cmp(dist2, dist1)) // descending
            .map(|(chunk, _dist)| chunk.clone())
            .collect_vec()
    }

    /// An implementation of `find_temporal_chunk_furthest_from` that focuses solely on correctness.
    ///
    /// Used to compare with results obtained from the optimized implementation.
    pub(crate) fn find_temporal_chunks_furthest_from_slow(
        &self,
        timeline: &TimelineName,
        time: TimeInt,
    ) -> Vec<Arc<Chunk>> {
        re_tracing::profile_function!();

        self.chunks_per_chunk_id
            .values()
            .filter_map(|chunk| {
                let times = chunk.timelines().get(timeline)?;

                let min_dist = times
                    .times()
                    .map(|t| t.as_i64().abs_diff(time.as_i64()))
                    .min();

                min_dist.map(|max_dist| (chunk, max_dist))
            })
            .sorted_by(|(_chunk1, dist1), (_chunk2, dist2)| std::cmp::Ord::cmp(dist2, dist1)) // descending
            .map(|(chunk, _dist)| chunk.clone())
            .collect_vec()
    }

    /// Retrieve all [`EntityPath`]s in the store.
    #[inline]
    pub fn all_entities_sorted(&self) -> BTreeSet<EntityPath> {
        self.static_chunk_ids_per_entity
            .keys()
            .cloned()
            .chain(self.temporal_chunk_ids_per_entity.keys().cloned())
            .collect()
    }

    /// Retrieve all [`ComponentIdentifier`]s in the store.
    ///
    /// See also [`Self::all_components_sorted`].
    pub fn all_components(&self) -> UnorderedComponentSet {
        self.static_chunk_ids_per_entity
            .values()
            .flat_map(|static_chunks_per_component| static_chunks_per_component.keys())
            .chain(
                self.temporal_chunk_ids_per_entity_per_component
                    .values()
                    .flat_map(|temporal_chunk_ids_per_timeline| {
                        temporal_chunk_ids_per_timeline.values().flat_map(
                            |temporal_chunk_ids_per_component| {
                                temporal_chunk_ids_per_component.keys()
                            },
                        )
                    }),
            )
            .copied()
            .collect()
    }

    /// Retrieve all [`ComponentIdentifier`]s in the store.
    ///
    /// See also [`Self::all_components`].
    pub fn all_components_sorted(&self) -> ComponentSet {
        self.static_chunk_ids_per_entity
            .values()
            .flat_map(|static_chunks_per_component| static_chunks_per_component.keys())
            .chain(
                self.temporal_chunk_ids_per_entity_per_component
                    .values()
                    .flat_map(|temporal_chunk_ids_per_timeline| {
                        temporal_chunk_ids_per_timeline.values().flat_map(
                            |temporal_chunk_ids_per_component| {
                                temporal_chunk_ids_per_component.keys()
                            },
                        )
                    }),
            )
            .copied()
            .collect()
    }

    /// Retrieve all the [`ComponentIdentifier`]s that have been written to for a given [`EntityPath`] on
    /// the specified [`Timeline`].
    ///
    /// Static components are always included in the results.
    ///
    /// Returns `None` if the entity doesn't exist at all on this `timeline`.
    pub fn all_components_on_timeline(
        &self,
        timeline: &TimelineName,
        entity_path: &EntityPath,
    ) -> Option<UnorderedComponentSet> {
        re_tracing::profile_function!();

        let static_components: Option<UnorderedComponentSet> = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .map(|static_chunks_per_component| {
                static_chunks_per_component
                    .keys()
                    .copied()
                    .collect::<UnorderedComponentSet>()
            })
            .filter(|names| !names.is_empty());

        let temporal_components: Option<UnorderedComponentSet> = self
            .temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .map(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline
                    .get(timeline)
                    .map(|temporal_chunk_ids_per_component| {
                        temporal_chunk_ids_per_component
                            .keys()
                            .copied()
                            .collect::<UnorderedComponentSet>()
                    })
                    .unwrap_or_default()
            })
            .filter(|names| !names.is_empty());

        match (static_components, temporal_components) {
            (None, None) => None,
            (None, Some(comps)) | (Some(comps), None) => Some(comps),
            (Some(static_comps), Some(temporal_comps)) => {
                Some(static_comps.into_iter().chain(temporal_comps).collect())
            }
        }
    }

    /// Retrieve all the [`ComponentIdentifier`]s that have been written to for a given [`EntityPath`] on
    /// the specified [`Timeline`].
    ///
    /// Static components are always included in the results.
    ///
    /// Returns `None` if the entity doesn't exist at all on this `timeline`.
    pub fn all_components_on_timeline_sorted(
        &self,
        timeline: &TimelineName,
        entity_path: &EntityPath,
    ) -> Option<ComponentSet> {
        re_tracing::profile_function!();

        let static_components: Option<ComponentSet> = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .map(|static_chunks_per_component| {
                static_chunks_per_component
                    .keys()
                    .copied()
                    .collect::<ComponentSet>()
            })
            .filter(|names| !names.is_empty());

        let temporal_components: Option<ComponentSet> = self
            .temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .map(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline
                    .get(timeline)
                    .map(|temporal_chunk_ids_per_component| {
                        temporal_chunk_ids_per_component
                            .keys()
                            .copied()
                            .collect::<ComponentSet>()
                    })
                    .unwrap_or_default()
            })
            .filter(|names| !names.is_empty());

        match (static_components, temporal_components) {
            (None, None) => None,
            (None, Some(comps)) | (Some(comps), None) => Some(comps),
            (Some(static_comps), Some(temporal_comps)) => {
                Some(static_comps.into_iter().chain(temporal_comps).collect())
            }
        }
    }

    /// Retrieve all the [`ComponentIdentifier`]s that have been written to for a given [`EntityPath`].
    ///
    /// Static components are always included in the results.
    ///
    /// Returns `None` if the entity has never had any data logged to it.
    pub fn all_components_for_entity(
        &self,
        entity_path: &EntityPath,
    ) -> Option<UnorderedComponentSet> {
        re_tracing::profile_function!();

        let static_components: Option<UnorderedComponentSet> = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .map(|static_chunks_per_component| {
                static_chunks_per_component.keys().copied().collect()
            });

        let temporal_components: Option<UnorderedComponentSet> = self
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

    /// Retrieve all the [`ComponentIdentifier`]s that have been written to for a given [`EntityPath`].
    ///
    /// Static components are always included in the results.
    ///
    /// Returns `None` if the entity has never had any data logged to it.
    pub fn all_components_for_entity_sorted(
        &self,
        entity_path: &EntityPath,
    ) -> Option<ComponentSet> {
        re_tracing::profile_function!();

        let static_components: Option<ComponentSet> = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .map(|static_chunks_per_component| {
                static_chunks_per_component.keys().copied().collect()
            });

        let temporal_components: Option<ComponentSet> = self
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

    /// Retrieves the [`ComponentDescriptor`] at a given [`EntityPath`] that has a certain [`ComponentIdentifier`].
    // TODO(andreas): The descriptor for a given identifier should never change within a recording.
    pub fn entity_component_descriptor(
        &self,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
    ) -> Option<ComponentDescriptor> {
        self.per_column_metadata
            .get(entity_path)
            .and_then(|per_identifier| per_identifier.get(&component))
            .map(|(component_descr, _, _)| component_descr.clone())
    }

    /// Check whether an entity has a static component or a temporal component on the specified timeline.
    ///
    /// This does _not_ check if the entity actually currently holds any data for that component.
    #[inline]
    pub fn entity_has_component_on_timeline(
        &self,
        timeline: &TimelineName,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
    ) -> bool {
        // re_tracing::profile_function!(); // This function is too fast; profiling will only add overhead

        self.entity_has_static_component(entity_path, component)
            || self.entity_has_temporal_component_on_timeline(timeline, entity_path, component)
    }

    /// Check whether an entity has a static component or a temporal component on any timeline.
    ///
    /// This does _not_ check if the entity actually currently holds any data for that component.
    pub fn entity_has_component(
        &self,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
    ) -> bool {
        // re_tracing::profile_function!(); // This function is too fast; profiling will only add overhead

        self.entity_has_static_component(entity_path, component)
            || self.entity_has_temporal_component(entity_path, component)
    }

    /// Check whether an entity has a specific static component.
    ///
    /// This does _not_ check if the entity actually currently holds any data for that component.
    #[inline]
    pub fn entity_has_static_component(
        &self,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
    ) -> bool {
        // re_tracing::profile_function!(); // This function is too fast; profiling will only add overhead

        self.static_chunk_ids_per_entity
            .get(entity_path)
            .is_some_and(|static_chunk_ids_per_component| {
                static_chunk_ids_per_component.contains_key(&component)
            })
    }

    /// Check whether an entity has a temporal component on any timeline.
    ///
    /// This does _not_ check if the entity actually currently holds any data for that component.
    #[inline]
    pub fn entity_has_temporal_component(
        &self,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
    ) -> bool {
        // re_tracing::profile_function!(); // This function is too fast; profiling will only add overhead

        self.temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .iter()
            .flat_map(|temporal_chunk_ids_per_timeline| temporal_chunk_ids_per_timeline.values())
            .any(|temporal_chunk_ids_per_component| {
                temporal_chunk_ids_per_component.contains_key(&component)
            })
    }

    /// Check whether an entity has a temporal component on a specific timeline.
    ///
    /// This does _not_ check if the entity actually currently holds any data for that component.
    #[inline]
    pub fn entity_has_temporal_component_on_timeline(
        &self,
        timeline: &TimelineName,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
    ) -> bool {
        // re_tracing::profile_function!(); // This function is too fast; profiling will only add overhead

        self.temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .iter()
            .filter_map(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline.get(timeline)
            })
            .any(|temporal_chunk_ids_per_component| {
                temporal_chunk_ids_per_component.contains_key(&component)
            })
    }

    /// Check whether an entity has any data on a specific timeline, or any static data.
    ///
    /// This is different from checking if the entity has any component, it also ensures
    /// that some _data_ currently exists in the store for this entity.
    #[inline]
    pub fn entity_has_data_on_timeline(
        &self,
        timeline: &TimelineName,
        entity_path: &EntityPath,
    ) -> bool {
        // re_tracing::profile_function!(); // This function is too fast; profiling will only add overhead

        self.entity_has_static_data(entity_path)
            || self.entity_has_temporal_data_on_timeline(timeline, entity_path)
    }

    /// Check whether an entity has any static data or any temporal data on any timeline.
    ///
    /// This is different from checking if the entity has any component, it also ensures
    /// that some _data_ currently exists in the store for this entity.
    #[inline]
    pub fn entity_has_data(&self, entity_path: &EntityPath) -> bool {
        // re_tracing::profile_function!(); // This function is too fast; profiling will only add overhead

        self.entity_has_static_data(entity_path) || self.entity_has_temporal_data(entity_path)
    }

    /// Check whether an entity has any static data.
    ///
    /// This is different from checking if the entity has any component, it also ensures
    /// that some _data_ currently exists in the store for this entity.
    #[inline]
    pub fn entity_has_static_data(&self, entity_path: &EntityPath) -> bool {
        // re_tracing::profile_function!(); // This function is too fast; profiling will only add overhead

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
        // re_tracing::profile_function!(); // This function is too fast; profiling will only add overhead

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
        timeline: &TimelineName,
        entity_path: &EntityPath,
    ) -> bool {
        // re_tracing::profile_function!(); // This function is too fast; profiling will only add overhead

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
        timeline: &TimelineName,
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
        timeline: &TimelineName,
        entity_path: &EntityPath,
    ) -> Option<AbsoluteTimeRange> {
        re_tracing::profile_function!();

        let temporal_chunk_ids_per_timeline =
            self.temporal_chunk_ids_per_entity.get(entity_path)?;
        let chunk_id_sets = temporal_chunk_ids_per_timeline.get(timeline)?;

        let start = chunk_id_sets.per_start_time.first_key_value()?.0;
        let end = chunk_id_sets.per_end_time.last_key_value()?.0;

        Some(AbsoluteTimeRange::new(*start, *end))
    }

    /// Returns the min and max times at which data was logged on a specific timeline, considering
    /// all entities.
    ///
    /// This ignores static data.
    pub fn time_range(&self, timeline: &TimelineName) -> Option<AbsoluteTimeRange> {
        re_tracing::profile_function!();

        self.temporal_chunk_ids_per_entity
            .values()
            .filter_map(|temporal_chunk_ids_per_timeline| {
                let per_time = temporal_chunk_ids_per_timeline.get(timeline)?;
                let start = per_time.per_start_time.first_key_value()?.0;
                let end = per_time.per_end_time.last_key_value()?.0;
                Some(AbsoluteTimeRange::new(*start, *end))
            })
            .reduce(|r1, r2| r1.union(r2))
    }
}

// ---

/// The results of a latest-at and/or range relevancy query.
///
/// Since the introduction of virtual/offloaded chunks, it is possible for a query to detect that
/// it is missing some data in order to compute accurate results.
/// This lack of data is communicated using a non-empty [`QueryResults::missing_virtual`] field.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryResults {
    /// The relevant *physical* chunks that were found for this query.
    ///
    /// If [`Self::missing_virtual`] is non-empty, then these chunks are not enough to compute accurate query results.
    pub chunks: Vec<Arc<Chunk>>,

    /// The relevant *virtual* chunks that were found for this query.
    ///
    /// Until these chunks have been fetched and inserted into the appropriate [`ChunkStore`], the
    /// results of this query cannot accurately be computed.
    ///
    /// Note, these are NOT necessarily _root_ chunks.
    /// Use [`ChunkStore::find_root_chunks`] to get those.
    //
    // TODO(cmc): Once lineage tracking is in place, make sure that this only reports missing
    // chunks using their root-level IDs, so downstream consumers don't have to redundantly build
    // their own tracking. And document it so.
    pub missing_virtual: Vec<ChunkId>,
}

impl std::fmt::Display for QueryResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            chunks,
            missing_virtual,
        } = self;

        let chunk_ids = chunks.iter().map(|c| c.id().to_string()).join(",");

        if self.is_partial() {
            let missing_ids = missing_virtual.iter().map(|id| id.to_string()).join(",");
            f.write_fmt(format_args!("chunks:[{chunk_ids}] missing:[{missing_ids}]"))
        } else {
            f.write_fmt(format_args!("chunks:[{chunk_ids}]"))
        }
    }
}

impl QueryResults {
    fn from_chunk_ids(
        store: &ChunkStore,
        report_mode: ChunkTrackingMode,
        chunk_ids: impl Iterator<Item = ChunkId>,
    ) -> Self {
        let mut this = Self {
            chunks: vec![],
            missing_virtual: vec![],
        };

        for chunk_id in chunk_ids {
            if let Some(chunk) = store.chunks_per_chunk_id.get(&chunk_id) {
                this.chunks.push(chunk.clone());
            } else {
                match report_mode {
                    ChunkTrackingMode::Ignore => {}
                    ChunkTrackingMode::Report => {
                        this.missing_virtual.push(chunk_id);
                    }
                    ChunkTrackingMode::PanicOnMissing => {
                        panic!("ChunkStore is missing chunk ID: {chunk_id}");
                    }
                }
            }
        }

        if report_mode == ChunkTrackingMode::Report {
            let mut tracker = store.queried_chunk_id_tracker.write();

            for chunk_id in &this.missing_virtual {
                debug_assert!(
                    store.chunks_lineage.contains_key(chunk_id),
                    "A chunk was reported missing, with no known lineage: {chunk_id}"
                );
                if !store.split_on_ingest.contains(chunk_id) {
                    if cfg!(debug_assertions) {
                        re_log::warn_once!(
                            "Tried to report a chunk missing that was the source of a split"
                        );
                    }
                    re_log::debug_once!(
                        "Tried to report a chunk missing that was the source of a split: {chunk_id}"
                    );
                }
            }

            tracker
                .missing_virtual
                .extend(this.missing_virtual.iter().copied());

            tracker
                .used_physical
                .extend(this.chunks.iter().map(|c| c.id()));
        }

        debug_assert!(
            this.chunks
                .iter()
                .map(|chunk| chunk.id())
                .chain(this.missing_virtual.iter().copied())
                .all_unique()
        );

        this
    }
}

impl QueryResults {
    /// Returns true if these are partial results.
    ///
    /// Partial results happen when some of the chunks required to accurately compute the query are
    /// currently missing/offloaded.
    /// It is then the responsibility of the caller to look into the [missing chunk IDs], fetch
    /// them, load them, and then try the query again.
    ///
    /// [missing chunk IDs]: `Self::missing_virtual`
    pub fn is_partial(&self) -> bool {
        !self.missing_virtual.is_empty()
    }

    /// Returns true if the results are *completely* empty.
    ///
    /// I.e. neither physical/loaded nor virtual/offloaded chunks could be found.
    pub fn is_empty(&self) -> bool {
        let Self {
            chunks,
            missing_virtual,
        } = self;
        chunks.is_empty() && missing_virtual.is_empty()
    }

    /// Attempts to iterate over the returned chunks.
    ///
    /// If the results contain partial data, returns `None`.
    /// It is then the responsibility of the caller to look into the [missing chunk IDs], fetch
    /// them, load them, and then try the query again.
    ///
    /// [missing chunk IDs]: `Self::missing_virtual`
    pub fn to_iter(&self) -> Option<impl Iterator<Item = &Arc<Chunk>>> {
        if self.missing_virtual.is_empty() {
            return Some(self.chunks.iter());
        }

        None
    }

    /// Attempts to iterate over the returned chunks.
    ///
    /// If the results contain partial data:
    /// * prints a debug log in release builds.
    /// * prints a warning in debug builds.
    ///
    /// It is the responsibility of the caller to look into the [missing chunk IDs], fetch
    /// them, load them, and then try the query again.
    ///
    /// [missing chunk IDs]: `Self::missing_virtual`
    //
    // TODO(RR-3295): this should ultimately not exist once all callsite have been updated to
    // the do whatever happens to be "the right thing" in their respective context.
    #[track_caller]
    pub fn into_iter_verbose(self) -> impl Iterator<Item = Arc<Chunk>> {
        if self.is_partial() {
            let location = std::panic::Location::caller();
            re_log::debug_warn_once!(
                "{}:{}: iterating partial query results: some data has been silently discarded",
                location.file(),
                location.line()
            );
        }

        self.chunks.into_iter()
    }
}

// LatestAt
impl ChunkStore {
    /// Returns the most-relevant chunk(s) for the given [`LatestAtQuery`] and [`ComponentIdentifier`].
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
        report_mode: ChunkTrackingMode,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
    ) -> QueryResults {
        // Don't do a profile scope here, this can have a lot of overhead when executing many small queries.
        //re_tracing::profile_function!(format!("{query:?}"));

        // Reminder: if a chunk has been indexed for a given component, then it must contain at
        // least one non-null value for that column.

        if let Some(static_chunk_id) = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .and_then(|static_chunks_per_component| static_chunks_per_component.get(&component))
        {
            return QueryResults::from_chunk_ids(
                self,
                report_mode,
                std::iter::once(*static_chunk_id),
            );
        }

        let chunk_ids = self
            .temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .and_then(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline.get(&query.timeline())
            })
            .and_then(|temporal_chunk_ids_per_component| {
                temporal_chunk_ids_per_component.get(&component)
            })
            .and_then(|temporal_chunk_ids_per_time| {
                Self::latest_at(query, temporal_chunk_ids_per_time)
            })
            .unwrap_or_default();

        QueryResults::from_chunk_ids(self, report_mode, chunk_ids.into_iter())
    }

    /// Returns the most-relevant chunk(s) for the given [`LatestAtQuery`].
    ///
    /// Optionally include static data.
    ///
    /// The [`ChunkStore`] always work at the [`Chunk`] level (as opposed to the row level): it is
    /// oblivious to the data therein.
    /// For that reason, and because [`Chunk`]s are allowed to temporally overlap, it is possible
    /// that a query has more than one relevant chunk.
    ///
    /// The returned vector is free of duplicates.
    ///
    /// The caller should filter the returned chunks further (see [`Chunk::latest_at`]) in order to
    /// determine what exact row contains the final result.
    pub fn latest_at_relevant_chunks_for_all_components(
        &self,
        report_mode: ChunkTrackingMode,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        include_static: bool,
    ) -> QueryResults {
        re_tracing::profile_function!(format!("{query:?}"));

        let chunk_ids = if include_static {
            let empty = Default::default();
            let static_chunks_per_component = self
                .static_chunk_ids_per_entity
                .get(entity_path)
                .unwrap_or(&empty);

            // All static chunk IDs for the given entity
            let static_chunk_ids = static_chunks_per_component.values().copied();

            // All temporal chunk IDs for the given entity, filtered by components
            // for which we already have static chunk IDs.
            let temporal_chunk_ids = self
                .temporal_chunk_ids_per_entity_per_component
                .get(entity_path)
                .and_then(|temporal_chunk_ids_per_timeline_per_component| {
                    temporal_chunk_ids_per_timeline_per_component.get(&query.timeline())
                })
                .map(|temporal_chunk_ids_per_component| {
                    temporal_chunk_ids_per_component
                        .iter()
                        .filter(|(component_type, _)| {
                            !static_chunks_per_component.contains_key(component_type)
                        })
                        .map(|(_, chunk_id_set)| chunk_id_set)
                })
                .into_iter()
                .flatten()
                .filter_map(|temporal_chunk_ids_per_time| {
                    Self::latest_at(query, temporal_chunk_ids_per_time)
                })
                .flatten();

            static_chunk_ids
                .chain(temporal_chunk_ids)
                // Deduplicate before passing it along.
                // Both temporal and static chunk "sets" here may have duplicates in them,
                // so we de-duplicate them together to reduce the number of allocations.
                .unique()
                .collect_vec()
        } else {
            // This cannot yield duplicates by definition.
            self.temporal_chunk_ids_per_entity
                .get(entity_path)
                .and_then(|temporal_chunk_ids_per_timeline| {
                    temporal_chunk_ids_per_timeline.get(&query.timeline())
                })
                .and_then(|temporal_chunk_ids_per_time| {
                    Self::latest_at(query, temporal_chunk_ids_per_time)
                })
                .unwrap_or_default()
        };

        QueryResults::from_chunk_ids(self, report_mode, chunk_ids.into_iter())
    }

    fn latest_at(
        query: &LatestAtQuery,
        temporal_chunk_ids_per_time: &ChunkIdSetPerTime,
    ) -> Option<Vec<ChunkId>> {
        // Don't do a profile scope here, this can have a lot of overhead when executing many small queries.
        //re_tracing::profile_function!();

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
        let lower_bound = upper_bound.as_i64().saturating_sub(
            temporal_chunk_ids_per_time
                .max_interval_length
                .saturating_cast(),
        );

        let temporal_chunk_ids = temporal_chunk_ids_per_time
            .per_start_time
            .range(..=query.at())
            .rev()
            .take_while(|(time, _)| time.as_i64() >= lower_bound)
            .flat_map(|(_time, chunk_ids)| chunk_ids.iter())
            .copied()
            .collect_vec();

        Some(temporal_chunk_ids)
    }
}

// Range
impl ChunkStore {
    /// Returns the most-relevant chunk(s) for the given [`RangeQuery`] and [`ComponentIdentifier`].
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
        report_mode: ChunkTrackingMode,
        query: &RangeQuery,
        entity_path: &EntityPath,
        component: ComponentIdentifier,
    ) -> QueryResults {
        re_tracing::profile_function!(format!("{query:?}"));

        if let Some(static_chunk_id) = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .and_then(|static_chunks_per_component| static_chunks_per_component.get(&component))
        {
            return QueryResults::from_chunk_ids(
                self,
                report_mode,
                std::iter::once(*static_chunk_id),
            );
        }

        let chunk_ids = Self::range(
            query,
            self.temporal_chunk_ids_per_entity_per_component
                .get(entity_path)
                .and_then(|temporal_chunk_ids_per_timeline| {
                    temporal_chunk_ids_per_timeline.get(query.timeline())
                })
                .and_then(|temporal_chunk_ids_per_component| {
                    temporal_chunk_ids_per_component.get(&component)
                })
                .into_iter(),
        );

        let mut results = QueryResults::from_chunk_ids(self, report_mode, chunk_ids.into_iter());
        results.chunks = results
            .chunks
            .into_iter()
            // Post-processing: `Self::range` doesn't have access to the chunk metadata, so now we
            // need to make sure that the resulting chunks' per-component time range intersects with the
            // time range of the query itself.
            .filter(|chunk| {
                chunk
                    .timelines()
                    .get(query.timeline())
                    .is_some_and(|time_column| {
                        time_column
                            .time_range_per_component(chunk.components())
                            .get(&component)
                            .is_some_and(|time_range| time_range.intersects(query.range()))
                    })
            })
            .collect_vec();

        results
    }

    /// Returns the most-relevant chunk(s) for the given [`RangeQuery`].
    ///
    /// The criteria for returning a chunk is only that it may contain data that overlaps with
    /// the queried range, or that it is static.
    ///
    /// The returned vector is free of duplicates.
    ///
    /// The caller should filter the returned chunks further (see [`Chunk::range`]) in order to
    /// determine how exactly each row of data fit with the rest.
    pub fn range_relevant_chunks_for_all_components(
        &self,
        report_mode: ChunkTrackingMode,
        query: &RangeQuery,
        entity_path: &EntityPath,
        include_static: bool,
    ) -> QueryResults {
        re_tracing::profile_function!(format!("{query:?}"));

        let empty = Default::default();
        let chunk_ids = if include_static {
            let static_chunks_per_component = self
                .static_chunk_ids_per_entity
                .get(entity_path)
                .unwrap_or(&empty);

            // All static chunk IDs for the given entity
            let static_chunk_ids = static_chunks_per_component.values().copied();

            // All temporal chunk IDs for the given entity, filtered by components for which we
            // already have static chunk IDs.
            let temporal_chunk_ids = Self::range(
                query,
                self.temporal_chunk_ids_per_entity_per_component
                    .get(entity_path)
                    .and_then(|temporal_chunk_ids_per_timeline_per_component| {
                        temporal_chunk_ids_per_timeline_per_component.get(query.timeline())
                    })
                    .map(|temporal_chunk_ids_per_component| {
                        temporal_chunk_ids_per_component
                            .iter()
                            .filter(|(component_type, _)| {
                                !static_chunks_per_component.contains_key(component_type)
                            })
                            .map(|(_, chunk_id_set)| chunk_id_set)
                    })
                    .into_iter()
                    .flatten(),
            )
            .into_iter();

            Either::Left(
                static_chunk_ids
                    .chain(temporal_chunk_ids)
                    // Deduplicate before passing it along.
                    // Both temporal and static chunk "sets" here may have duplicates in them,
                    // so we de-duplicate them together to reduce the number of allocations.
                    .unique(),
            )
        } else {
            // This cannot yield duplicates by definition.
            Either::Right(Self::range(
                query,
                self.temporal_chunk_ids_per_entity
                    .get(entity_path)
                    .and_then(|temporal_chunk_ids_per_timeline| {
                        temporal_chunk_ids_per_timeline.get(query.timeline())
                    })
                    .into_iter(),
            ))
        };

        let mut results = QueryResults::from_chunk_ids(self, report_mode, chunk_ids.into_iter());
        results.chunks = results
            .chunks
            .into_iter()
            // Post-processing: `Self::range` doesn't have access to the chunk metadata, so now we
            // need to make sure that the resulting chunks' per-component time range intersects with the
            // time range of the query itself.
            .filter(|chunk| {
                (chunk.is_static() && include_static) || {
                    chunk
                        .timelines()
                        .get(query.timeline())
                        .is_some_and(|time_column| {
                            time_column.time_range().intersects(query.range())
                        })
                }
            })
            .collect_vec();

        results
    }

    fn range<'a>(
        query: &RangeQuery,
        temporal_chunk_ids_per_times: impl Iterator<Item = &'a ChunkIdSetPerTime>,
    ) -> Vec<ChunkId> {
        // Too small & frequent for profiling scopes.
        //re_tracing::profile_function!();

        temporal_chunk_ids_per_times
            .map(|temporal_chunk_ids_per_time| {
                // See `RangeQueryOptions::include_extended_bounds` for more information.
                let query_min = if query.options().include_extended_bounds {
                    re_log_types::TimeInt::new_temporal(
                        query.range.min().as_i64().saturating_sub(1),
                    )
                } else {
                    query.range.min()
                };
                let query_max = if query.options().include_extended_bounds {
                    re_log_types::TimeInt::new_temporal(
                        query.range.max().as_i64().saturating_add(1),
                    )
                } else {
                    query.range.max()
                };

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
                let query_min = TimeInt::new_temporal(
                    query_min.as_i64().saturating_sub(
                        temporal_chunk_ids_per_time
                            .max_interval_length
                            .saturating_cast(),
                    ),
                );

                let start_time = temporal_chunk_ids_per_time
                    .per_start_time
                    .range(..=query_min)
                    .next_back()
                    .map_or(TimeInt::MIN, |(&time, _)| time);

                let end_time = temporal_chunk_ids_per_time
                    .per_start_time
                    .range(..=query_max)
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
            .flatten()
            .copied()
            .collect()
    }
}

#[cfg(test)]
#[expect(clippy::bool_assert_comparison)] // I like it that way, sue me
mod tests {
    use std::sync::Arc;

    use re_chunk::{Chunk, RowId};
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::external::re_tuid::Tuid;
    use re_log_types::{EntityPath, TimePoint, Timeline};

    use super::*;

    // Make sure queries yield partial results when we expect them to.
    #[test]
    fn partial_data_basics() {
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            crate::ChunkStoreConfig::ALL_DISABLED,
        );

        let entity_path: EntityPath = "some_entity".into();

        let timeline_frame = Timeline::new_sequence("frame");
        let timepoint1 = TimePoint::from_iter([(timeline_frame, 1)]);
        let timepoint2 = TimePoint::from_iter([(timeline_frame, 2)]);
        let timepoint3 = TimePoint::from_iter([(timeline_frame, 3)]);

        let point1 = MyPoint::new(1.0, 1.0);
        let point2 = MyPoint::new(2.0, 2.0);
        let point3 = MyPoint::new(3.0, 3.0);

        let mut next_chunk_id = next_chunk_id_generator(0x1337);

        let chunk1 = create_chunk_with_point(
            next_chunk_id(),
            entity_path.clone(),
            timepoint1.clone(),
            point1,
        );
        let chunk2 = create_chunk_with_point(
            next_chunk_id(),
            entity_path.clone(),
            timepoint2.clone(),
            point2,
        );
        let chunk3 = create_chunk_with_point(
            next_chunk_id(),
            entity_path.clone(),
            timepoint3.clone(),
            point3,
        );

        // We haven't inserted anything yet, so we just expect empty results across the board.
        {
            let results = store.latest_at_relevant_chunks(
                ChunkTrackingMode::Report,
                &LatestAtQuery::new(*timeline_frame.name(), 3),
                &entity_path,
                MyPoints::descriptor_points().component,
            );
            assert!(results.is_empty());

            let results = store.range_relevant_chunks(
                ChunkTrackingMode::Report,
                &RangeQuery::new(*timeline_frame.name(), AbsoluteTimeRange::new(0, 3)),
                &entity_path,
                MyPoints::descriptor_points().component,
            );
            assert!(results.is_empty());

            assert!(store.take_tracked_chunk_ids().missing_virtual.is_empty());
        }

        store.insert_chunk(&chunk1).unwrap();
        store.insert_chunk(&chunk2).unwrap();
        store.insert_chunk(&chunk3).unwrap();

        // Now we've inserted everything, so we expect complete results across the board.
        {
            let results = store.latest_at_relevant_chunks(
                ChunkTrackingMode::Report,
                &LatestAtQuery::new(*timeline_frame.name(), 3),
                &entity_path,
                MyPoints::descriptor_points().component,
            );
            let expected = QueryResults {
                chunks: vec![chunk3.clone()],
                missing_virtual: vec![],
            };
            assert_eq!(false, results.is_partial());
            assert_eq!(expected, results);

            let results = store.range_relevant_chunks(
                ChunkTrackingMode::Report,
                &RangeQuery::new(*timeline_frame.name(), AbsoluteTimeRange::new(0, 3)),
                &entity_path,
                MyPoints::descriptor_points().component,
            );
            let expected = QueryResults {
                chunks: vec![chunk1.clone(), chunk2.clone(), chunk3.clone()],
                missing_virtual: vec![],
            };
            assert_eq!(false, results.is_partial());
            assert_eq!(expected, results);

            assert!(store.take_tracked_chunk_ids().missing_virtual.is_empty());
        }

        store.gc(&crate::GarbageCollectionOptions {
            target: crate::GarbageCollectionTarget::Everything,
            time_budget: std::time::Duration::MAX,
            protect_latest: 1,
            protected_time_ranges: Default::default(),
            protected_chunks: Default::default(),
            furthest_from: None,
            perform_deep_deletions: false,
        });

        // We've GC'd the past-most half of the store:
        // * latest-at results should still be complete
        // * range results should now be partial
        {
            let results_latest_at = store.latest_at_relevant_chunks(
                ChunkTrackingMode::Report,
                &LatestAtQuery::new(*timeline_frame.name(), 3),
                &entity_path,
                MyPoints::descriptor_points().component,
            );
            let expected = QueryResults {
                chunks: vec![chunk3.clone()],
                missing_virtual: vec![],
            };
            assert_eq!(false, results_latest_at.is_partial());
            assert_eq!(expected, results_latest_at);

            let results_range = store.range_relevant_chunks(
                ChunkTrackingMode::Report,
                &RangeQuery::new(*timeline_frame.name(), AbsoluteTimeRange::new(0, 3)),
                &entity_path,
                MyPoints::descriptor_points().component,
            );
            let expected = QueryResults {
                chunks: vec![chunk3.clone()],
                missing_virtual: vec![chunk1.id(), chunk2.id()],
            };
            assert_eq!(true, results_range.is_partial());
            assert_eq!(expected, results_range);

            assert_eq!(
                store.take_tracked_chunk_ids().missing_virtual,
                itertools::chain!(
                    results_latest_at.missing_virtual,
                    results_range.missing_virtual
                )
                .collect()
            );
        }

        store.gc(&crate::GarbageCollectionOptions::gc_everything());

        // Now we've GC'd absolutely everything: we should only get partial results.
        {
            let results_latest_at = store.latest_at_relevant_chunks(
                ChunkTrackingMode::Report,
                &LatestAtQuery::new(*timeline_frame.name(), 3),
                &entity_path,
                MyPoints::descriptor_points().component,
            );
            let expected = QueryResults {
                chunks: vec![],
                missing_virtual: vec![chunk3.id()],
            };
            assert_eq!(true, results_latest_at.is_partial());
            assert_eq!(expected, results_latest_at);

            let results_range = store.range_relevant_chunks(
                ChunkTrackingMode::Report,
                &RangeQuery::new(*timeline_frame.name(), AbsoluteTimeRange::new(0, 3)),
                &entity_path,
                MyPoints::descriptor_points().component,
            );
            let expected = QueryResults {
                chunks: vec![],
                missing_virtual: vec![chunk1.id(), chunk2.id(), chunk3.id()],
            };
            assert_eq!(true, results_range.is_partial());
            assert_eq!(expected, results_range);

            assert_eq!(
                store.take_tracked_chunk_ids().missing_virtual,
                itertools::chain!(
                    results_latest_at.missing_virtual,
                    results_range.missing_virtual
                )
                .collect()
            );
        }

        store.insert_chunk(&chunk1).unwrap();
        store.insert_chunk(&chunk2).unwrap();
        store.insert_chunk(&chunk3).unwrap();

        // We've inserted everything back: all results should be complete once again.
        {
            let results = store.latest_at_relevant_chunks(
                ChunkTrackingMode::Report,
                &LatestAtQuery::new(*timeline_frame.name(), 3),
                &entity_path,
                MyPoints::descriptor_points().component,
            );
            let expected = QueryResults {
                chunks: vec![chunk3.clone()],
                missing_virtual: vec![],
            };
            assert_eq!(false, results.is_partial());
            assert_eq!(expected, results);

            let results = store.range_relevant_chunks(
                ChunkTrackingMode::Report,
                &RangeQuery::new(*timeline_frame.name(), AbsoluteTimeRange::new(0, 3)),
                &entity_path,
                MyPoints::descriptor_points().component,
            );
            let expected = QueryResults {
                chunks: vec![chunk1.clone(), chunk2.clone(), chunk3.clone()],
                missing_virtual: vec![],
            };
            assert_eq!(false, results.is_partial());
            assert_eq!(expected, results);

            assert!(store.take_tracked_chunk_ids().missing_virtual.is_empty());
        }
    }

    // Make sure compacted chunks don't linger on in virtual indices, leading to false partial result positives.
    #[test]
    fn partial_data_compaction() {
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            crate::ChunkStoreConfig::default(), // with compaction!
        );

        let entity_path: EntityPath = "some_entity".into();

        let timeline_frame = Timeline::new_sequence("frame");
        let timepoint1 = TimePoint::from_iter([(timeline_frame, 1)]);
        let timepoint2 = TimePoint::from_iter([(timeline_frame, 2)]);
        let timepoint3 = TimePoint::from_iter([(timeline_frame, 3)]);

        let point1 = MyPoint::new(1.0, 1.0);
        let point2 = MyPoint::new(2.0, 2.0);
        let point3 = MyPoint::new(3.0, 3.0);

        let mut next_chunk_id = next_chunk_id_generator(0x1337);

        let chunk1 = create_chunk_with_point(
            next_chunk_id(),
            entity_path.clone(),
            timepoint1.clone(),
            point1,
        );
        let chunk2 = create_chunk_with_point(
            next_chunk_id(),
            entity_path.clone(),
            timepoint2.clone(),
            point2,
        );
        let chunk3 = create_chunk_with_point(
            next_chunk_id(),
            entity_path.clone(),
            timepoint3.clone(),
            point3,
        );

        {
            let results = store.latest_at_relevant_chunks(
                ChunkTrackingMode::Report,
                &LatestAtQuery::new(*timeline_frame.name(), 3),
                &entity_path,
                MyPoints::descriptor_points().component,
            );
            assert!(results.is_empty());

            let results = store.range_relevant_chunks(
                ChunkTrackingMode::Report,
                &RangeQuery::new(*timeline_frame.name(), AbsoluteTimeRange::new(0, 3)),
                &entity_path,
                MyPoints::descriptor_points().component,
            );
            assert!(results.is_empty());
        }

        store.insert_chunk(&chunk1).unwrap();
        store.insert_chunk(&chunk2).unwrap();
        store.insert_chunk(&chunk3).unwrap();

        // We cannot possibly know what to expect since the IDs will depend on the result of running
        // compaction, but we definitely know that all results should be complete at this point.
        //
        // This used to fail because the compacted IDs would linger on in the internal virtual indices.
        {
            let results = store.latest_at_relevant_chunks(
                ChunkTrackingMode::Report,
                &LatestAtQuery::new(*timeline_frame.name(), 3),
                &entity_path,
                MyPoints::descriptor_points().component,
            );
            assert_eq!(false, results.is_partial());

            let results = store.range_relevant_chunks(
                ChunkTrackingMode::Report,
                &RangeQuery::new(*timeline_frame.name(), AbsoluteTimeRange::new(0, 3)),
                &entity_path,
                MyPoints::descriptor_points().component,
            );
            assert_eq!(false, results.is_partial());
        }
    }

    fn next_chunk_id_generator(prefix: u64) -> impl FnMut() -> re_chunk::ChunkId {
        let mut chunk_id = re_chunk::ChunkId::from_tuid(Tuid::from_nanos_and_inc(prefix, 0));
        move || {
            chunk_id = chunk_id.next();
            chunk_id
        }
    }

    fn create_chunk_with_point(
        chunk_id: ChunkId,
        entity_path: EntityPath,
        timepoint: TimePoint,
        point: MyPoint,
    ) -> Arc<Chunk> {
        Arc::new(
            Chunk::builder_with_id(chunk_id, entity_path)
                .with_component_batch(
                    RowId::new(),
                    timepoint,
                    (MyPoints::descriptor_points(), &[point]),
                )
                .build()
                .unwrap(),
        )
    }
}
