use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use itertools::{Either, Itertools as _};
use nohash_hasher::IntSet;

use re_chunk::{
    ArchetypeName, Chunk, ComponentIdentifier, LatestAtQuery, RangeQuery, TimelineName,
};
use re_log_types::ResolvedTimeRange;
use re_log_types::{EntityPath, TimeInt, Timeline};
use re_types_core::{
    ComponentDescriptor, ComponentDescriptorSet, ComponentType, UnorderedComponentDescriptorSet,
};

use crate::{ChunkStore, store::ChunkIdSetPerTime};

// Used all over in docstrings.
#[allow(unused_imports)]
use crate::RowId;

// ---

// These APIs often have `temporal` and `static` variants.
// It is sometimes useful to be able to separately query either,
// such as when we want to tell the user that they logged a component
// as both static and temporal, which is probably wrong.

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

    /// Retrieve all [`EntityPath`]s in the store.
    #[inline]
    pub fn all_entities_sorted(&self) -> BTreeSet<EntityPath> {
        self.static_chunk_ids_per_entity
            .keys()
            .cloned()
            .chain(self.temporal_chunk_ids_per_entity.keys().cloned())
            .collect()
    }

    /// Retrieve all [`ComponentDescriptor`]s in the store.
    pub fn all_components(&self) -> IntSet<ComponentDescriptor> {
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
            .cloned()
            .collect()
    }

    /// Retrieve all [`ComponentDescriptor`]s in the store.
    pub fn all_components_sorted(&self) -> ComponentDescriptorSet {
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
            .cloned()
            .collect()
    }

    /// Retrieve all the [`ComponentDescriptor`]s that have been written to for a given [`EntityPath`] on
    /// the specified [`Timeline`].
    ///
    /// Static components are always included in the results.
    ///
    /// Returns `None` if the entity doesn't exist at all on this `timeline`.
    pub fn all_components_on_timeline(
        &self,
        timeline: &TimelineName,
        entity_path: &EntityPath,
    ) -> Option<UnorderedComponentDescriptorSet> {
        re_tracing::profile_function!();

        let static_components: Option<UnorderedComponentDescriptorSet> = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .map(|static_chunks_per_component| {
                static_chunks_per_component
                    .keys()
                    .cloned()
                    .collect::<UnorderedComponentDescriptorSet>()
            })
            .filter(|names| !names.is_empty());

        let temporal_components: Option<UnorderedComponentDescriptorSet> = self
            .temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .map(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline
                    .get(timeline)
                    .map(|temporal_chunk_ids_per_component| {
                        temporal_chunk_ids_per_component
                            .keys()
                            .cloned()
                            .collect::<UnorderedComponentDescriptorSet>()
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

    /// Retrieve all the [`ComponentDescriptor`]s that have been written to for a given [`EntityPath`] on
    /// the specified [`Timeline`].
    ///
    /// Static components are always included in the results.
    ///
    /// Returns `None` if the entity doesn't exist at all on this `timeline`.
    pub fn all_components_on_timeline_sorted(
        &self,
        timeline: &TimelineName,
        entity_path: &EntityPath,
    ) -> Option<ComponentDescriptorSet> {
        re_tracing::profile_function!();

        let static_components: Option<ComponentDescriptorSet> = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .map(|static_chunks_per_component| {
                static_chunks_per_component
                    .keys()
                    .cloned()
                    .collect::<ComponentDescriptorSet>()
            })
            .filter(|names| !names.is_empty());

        let temporal_components: Option<ComponentDescriptorSet> = self
            .temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .map(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline
                    .get(timeline)
                    .map(|temporal_chunk_ids_per_component| {
                        temporal_chunk_ids_per_component
                            .keys()
                            .cloned()
                            .collect::<ComponentDescriptorSet>()
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

    /// Retrieve all the [`ComponentDescriptor`]s that have been written to for a given [`EntityPath`].
    ///
    /// Static components are always included in the results.
    ///
    /// Returns `None` if the entity has never had any data logged to it.
    pub fn all_components_for_entity(
        &self,
        entity_path: &EntityPath,
    ) -> Option<UnorderedComponentDescriptorSet> {
        re_tracing::profile_function!();

        let static_components: Option<UnorderedComponentDescriptorSet> = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .map(|static_chunks_per_component| {
                static_chunks_per_component.keys().cloned().collect()
            });

        let temporal_components: Option<UnorderedComponentDescriptorSet> = self
            .temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .map(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline
                    .iter()
                    .flat_map(|(_, temporal_chunk_ids_per_component)| {
                        temporal_chunk_ids_per_component.keys().cloned()
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

    /// Retrieve all the [`ComponentDescriptor`]s that have been written to for a given [`EntityPath`].
    ///
    /// Static components are always included in the results.
    ///
    /// Returns `None` if the entity has never had any data logged to it.
    pub fn all_components_for_entity_sorted(
        &self,
        entity_path: &EntityPath,
    ) -> Option<ComponentDescriptorSet> {
        re_tracing::profile_function!();

        let static_components: Option<ComponentDescriptorSet> = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .map(|static_chunks_per_component| {
                static_chunks_per_component.keys().cloned().collect()
            });

        let temporal_components: Option<ComponentDescriptorSet> = self
            .temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .map(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline
                    .iter()
                    .flat_map(|(_, temporal_chunk_ids_per_component)| {
                        temporal_chunk_ids_per_component.keys().cloned()
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
    pub fn entity_component_descriptor(
        &self,
        entity_path: &EntityPath,
        archetype_name: Option<ArchetypeName>,
        identifier: ComponentIdentifier,
    ) -> Option<ComponentDescriptor> {
        let matches = |descr: &&ComponentDescriptor| {
            descr.component == identifier && descr.archetype_name == archetype_name
        };

        let static_chunks = self.static_chunk_ids_per_entity.get(entity_path);
        let static_components_descr =
            static_chunks
                .iter()
                .flat_map(|static_chunks_per_component| {
                    static_chunks_per_component.keys().filter(matches)
                });

        let temporal_chunks = self
            .temporal_chunk_ids_per_entity_per_component
            .get(entity_path);
        let temporal_components_descr =
            temporal_chunks
                .iter()
                .flat_map(|temporal_chunk_ids_per_timeline| {
                    temporal_chunk_ids_per_timeline.iter().flat_map(
                        |(_, temporal_chunk_ids_per_component)| {
                            temporal_chunk_ids_per_component.keys().filter(matches)
                        },
                    )
                });

        let mut iter = static_components_descr.chain(temporal_components_descr);
        let result = iter.next();

        if cfg!(debug_assertions) {
            for other in iter {
                if result != Some(other) {
                    re_log::error!("column is not unique: expected {result:?} found {other:?}");
                }
            }
        }

        result.cloned()
    }

    /// Lists all [`ComponentDescriptor`]s at a given [`EntityPath`] that use a certain [`ComponentType`].
    ///
    /// [`ComponentType`] alone does not uniquely identify a component, which is why this method returns a set
    /// of [`ComponentDescriptor`]s. If you want to retrieve a specific component it's better to use
    /// [`ChunkStore::entity_component_descriptor`].
    pub fn entity_component_descriptors_with_type(
        &self,
        entity_path: &EntityPath,
        component_type: ComponentType,
    ) -> IntSet<ComponentDescriptor> {
        let static_chunks = self.static_chunk_ids_per_entity.get(entity_path);
        let static_components_descr =
            static_chunks
                .iter()
                .flat_map(|static_chunks_per_component| {
                    static_chunks_per_component
                        .keys()
                        .filter(|descr| descr.component_type == Some(component_type))
                });

        let temporal_chunks = self
            .temporal_chunk_ids_per_entity_per_component
            .get(entity_path);
        let temporal_components_descr =
            temporal_chunks
                .iter()
                .flat_map(|temporal_chunk_ids_per_timeline| {
                    temporal_chunk_ids_per_timeline.iter().flat_map(
                        |(_, temporal_chunk_ids_per_component)| {
                            temporal_chunk_ids_per_component
                                .keys()
                                .filter(|descr| descr.component_type == Some(component_type))
                        },
                    )
                });

        static_components_descr
            .chain(temporal_components_descr)
            .cloned()
            .collect()
    }

    /// Check whether an entity has a static component or a temporal component on the specified timeline.
    ///
    /// This does _not_ check if the entity actually currently holds any data for that component.
    #[inline]
    pub fn entity_has_component_on_timeline(
        &self,
        timeline: &TimelineName,
        entity_path: &EntityPath,
        component_descr: &ComponentDescriptor,
    ) -> bool {
        // re_tracing::profile_function!(); // This function is too fast; profiling will only add overhead

        self.entity_has_static_component(entity_path, component_descr)
            || self.entity_has_temporal_component_on_timeline(
                timeline,
                entity_path,
                component_descr,
            )
    }

    /// Check whether an entity has a static component or a temporal component on any timeline.
    ///
    /// This does _not_ check if the entity actually currently holds any data for that component.
    pub fn entity_has_component(
        &self,
        entity_path: &EntityPath,
        component_descr: &ComponentDescriptor,
    ) -> bool {
        // re_tracing::profile_function!(); // This function is too fast; profiling will only add overhead

        self.entity_has_static_component(entity_path, component_descr)
            || self.entity_has_temporal_component(entity_path, component_descr)
    }

    /// Check whether an entity has a specific static component.
    ///
    /// This does _not_ check if the entity actually currently holds any data for that component.
    #[inline]
    pub fn entity_has_static_component(
        &self,
        entity_path: &EntityPath,
        component_descr: &ComponentDescriptor,
    ) -> bool {
        // re_tracing::profile_function!(); // This function is too fast; profiling will only add overhead

        self.static_chunk_ids_per_entity
            .get(entity_path)
            .is_some_and(|static_chunk_ids_per_component| {
                static_chunk_ids_per_component.contains_key(component_descr)
            })
    }

    /// Check whether an entity has a temporal component on any timeline.
    ///
    /// This does _not_ check if the entity actually currently holds any data for that component.
    #[inline]
    pub fn entity_has_temporal_component(
        &self,
        entity_path: &EntityPath,
        component_descr: &ComponentDescriptor,
    ) -> bool {
        // re_tracing::profile_function!(); // This function is too fast; profiling will only add overhead

        self.temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .iter()
            .flat_map(|temporal_chunk_ids_per_timeline| temporal_chunk_ids_per_timeline.values())
            .any(|temporal_chunk_ids_per_component| {
                temporal_chunk_ids_per_component.contains_key(component_descr)
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
        component_descr: &ComponentDescriptor,
    ) -> bool {
        // re_tracing::profile_function!(); // This function is too fast; profiling will only add overhead

        self.temporal_chunk_ids_per_entity_per_component
            .get(entity_path)
            .iter()
            .filter_map(|temporal_chunk_ids_per_timeline| {
                temporal_chunk_ids_per_timeline.get(timeline)
            })
            .any(|temporal_chunk_ids_per_component| {
                temporal_chunk_ids_per_component.contains_key(component_descr)
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
    ) -> Option<ResolvedTimeRange> {
        re_tracing::profile_function!();

        let temporal_chunk_ids_per_timeline =
            self.temporal_chunk_ids_per_entity.get(entity_path)?;
        let chunk_id_sets = temporal_chunk_ids_per_timeline.get(timeline)?;

        let start = chunk_id_sets.per_start_time.first_key_value()?.0;
        let end = chunk_id_sets.per_end_time.last_key_value()?.0;

        Some(ResolvedTimeRange::new(*start, *end))
    }

    /// Returns the min and max times at which data was logged on a specific timeline, considering
    /// all entities.
    ///
    /// This ignores static data.
    pub fn time_range(&self, timeline: &TimelineName) -> Option<ResolvedTimeRange> {
        re_tracing::profile_function!();

        self.temporal_chunk_ids_per_entity
            .values()
            .filter_map(|temporal_chunk_ids_per_timeline| {
                let per_time = temporal_chunk_ids_per_timeline.get(timeline)?;
                let start = per_time.per_start_time.first_key_value()?.0;
                let end = per_time.per_end_time.last_key_value()?.0;
                Some(ResolvedTimeRange::new(*start, *end))
            })
            .reduce(|r1, r2| r1.union(r2))
    }
}

// LatestAt
impl ChunkStore {
    /// Returns the most-relevant chunk(s) for the given [`LatestAtQuery`] and [`ComponentDescriptor`].
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
        component_descr: &ComponentDescriptor,
    ) -> Vec<Arc<Chunk>> {
        // Don't do a profile scope here, this can have a lot of overhead when executing many small queries.
        //re_tracing::profile_function!(format!("{query:?}"));

        // Reminder: if a chunk has been indexed for a given component, then it must contain at
        // least one non-null value for that column.

        if let Some(static_chunk) = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .and_then(|static_chunks_per_component| {
                static_chunks_per_component.get(component_descr)
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
                temporal_chunk_ids_per_component.get(component_descr)
            })
            .and_then(|temporal_chunk_ids_per_time| {
                self.latest_at(query, temporal_chunk_ids_per_time)
            })
            .unwrap_or_default();

        debug_assert!(
            chunks.iter().map(|chunk| chunk.id()).all_unique(),
            "{entity_path}:{component_descr} @ {query:?}",
        );

        chunks
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
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        include_static: bool,
    ) -> Vec<Arc<Chunk>> {
        re_tracing::profile_function!(format!("{query:?}"));

        let chunks = if include_static {
            let empty = Default::default();
            let static_chunks_per_component = self
                .static_chunk_ids_per_entity
                .get(entity_path)
                .unwrap_or(&empty);

            // All static chunks for the given entity
            let static_chunks = static_chunks_per_component
                .values()
                .filter_map(|chunk_id| self.chunks_per_chunk_id.get(chunk_id))
                .cloned();

            // All temporal chunks for the given entity, filtered by components
            // for which we already have static chunks.
            let temporal_chunks = self
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
                    self.latest_at(query, temporal_chunk_ids_per_time)
                })
                .flatten();

            static_chunks
                .chain(temporal_chunks)
                // Deduplicate before passing it along.
                // Both temporal and static chunk "sets" here may have duplicates in them,
                // so we de-duplicate them together to reduce the number of allocations.
                .unique_by(|chunk| chunk.id())
                .collect_vec()
        } else {
            // This cannot yield duplicates by definition.
            self.temporal_chunk_ids_per_entity
                .get(entity_path)
                .and_then(|temporal_chunk_ids_per_timeline| {
                    temporal_chunk_ids_per_timeline.get(&query.timeline())
                })
                .and_then(|temporal_chunk_ids_per_time| {
                    self.latest_at(query, temporal_chunk_ids_per_time)
                })
                .unwrap_or_default()
        };

        debug_assert!(chunks.iter().map(|chunk| chunk.id()).all_unique());

        chunks
    }

    fn latest_at(
        &self,
        query: &LatestAtQuery,
        temporal_chunk_ids_per_time: &ChunkIdSetPerTime,
    ) -> Option<Vec<Arc<Chunk>>> {
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
            .collect_vec();

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
    /// Returns the most-relevant chunk(s) for the given [`RangeQuery`] and [`ComponentDescriptor`].
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
        component_descr: &ComponentDescriptor,
    ) -> Vec<Arc<Chunk>> {
        re_tracing::profile_function!(format!("{query:?}"));

        if let Some(static_chunk) = self
            .static_chunk_ids_per_entity
            .get(entity_path)
            .and_then(|static_chunks_per_component| {
                static_chunks_per_component.get(component_descr)
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
                        temporal_chunk_ids_per_timeline.get(query.timeline())
                    })
                    .and_then(|temporal_chunk_ids_per_component| {
                        temporal_chunk_ids_per_component.get(component_descr)
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
                    .get(query.timeline())
                    .is_some_and(|time_column| {
                        time_column
                            .time_range_per_component(chunk.components())
                            .get(component_descr)
                            .is_some_and(|time_range| time_range.intersects(query.range()))
                    })
            })
            .collect_vec();

        debug_assert!(chunks.iter().map(|chunk| chunk.id()).all_unique());

        chunks
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
        query: &RangeQuery,
        entity_path: &EntityPath,
        include_static: bool,
    ) -> Vec<Arc<Chunk>> {
        re_tracing::profile_function!(format!("{query:?}"));

        let empty = Default::default();
        let chunks = if include_static {
            let static_chunks_per_component = self
                .static_chunk_ids_per_entity
                .get(entity_path)
                .unwrap_or(&empty);

            // All static chunks for the given entity
            let static_chunks = static_chunks_per_component
                .values()
                .filter_map(|chunk_id| self.chunks_per_chunk_id.get(chunk_id))
                .cloned();

            // All temporal chunks for the given entity, filtered by components
            // for which we already have static chunks.
            let temporal_chunks = self
                .range(
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
                static_chunks
                    .chain(temporal_chunks)
                    // Deduplicate before passing it along.
                    // Both temporal and static chunk "sets" here may have duplicates in them,
                    // so we de-duplicate them together to reduce the number of allocations.
                    .unique_by(|chunk| chunk.id()),
            )
        } else {
            // This cannot yield duplicates by definition.
            Either::Right(
                self.range(
                    query,
                    self.temporal_chunk_ids_per_entity
                        .get(entity_path)
                        .and_then(|temporal_chunk_ids_per_timeline| {
                            temporal_chunk_ids_per_timeline.get(query.timeline())
                        })
                        .into_iter(),
                ),
            )
        };

        // Post-processing: `Self::range` doesn't have access to the chunk metadata, so now we
        // need to make sure that the resulting chunks' global time ranges intersect with the
        // time range of the query itself.
        let chunks = chunks
            .into_iter()
            .filter(|chunk| {
                chunk
                    .timelines()
                    .get(query.timeline())
                    .is_some_and(|time_column| time_column.time_range().intersects(query.range()))
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
                    query_min
                        .as_i64()
                        .saturating_sub(temporal_chunk_ids_per_time.max_interval_length as _),
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
            .flat_map(|temporal_chunk_ids| {
                temporal_chunk_ids
                    .iter()
                    .filter_map(|chunk_id| self.chunks_per_chunk_id.get(chunk_id).cloned())
            })
            .collect()
    }
}
