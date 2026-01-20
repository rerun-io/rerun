use std::collections::hash_map::Entry;

use ahash::HashMap;
use bit_vec::BitVec;
use nohash_hasher::IntMap;
use re_chunk::{ArchetypeName, ArrowArray as _, ComponentIdentifier};
use re_chunk_store::{ChunkStoreDiffKind, ChunkStoreEvent, ChunkStoreSubscriber};
use re_log_types::{EntityPathHash, StoreId};
use re_sdk_types::ComponentSet;
use re_types_core::SerializedComponentColumn;
use vec1::smallvec_v1::SmallVec1;

use crate::{
    IdentifiedViewSystem, IndicatedEntities, RequiredComponents, ViewSystemIdentifier,
    VisualizableEntities, VisualizerSystem, typed_entity_collections::VisualizableReason,
};

use super::visualizer_system::DatatypeSet;

/// A store subscriber that keep track which entities in a store can be
/// processed by a single given visualizer type.
///
/// The list of entities is additive:
/// If an entity was at any point in time passes the "visualizable" filter for the visualizer, it will be
/// kept in the list of entities.
///
/// "visualizable" is determined by the set of required components
///
/// There's only a single entity subscriber per visualizer *type*.
/// This means that if the same visualizer is used in multiple views, only a single
/// `VisualizerEntitySubscriber` is created for all of them.
pub struct VisualizerEntitySubscriber {
    /// Visualizer type this subscriber is associated with.
    visualizer: ViewSystemIdentifier,

    /// See [`crate::VisualizerQueryInfo::relevant_archetype`]
    relevant_archetype: Option<ArchetypeName>,

    /// The mode for checking component requirements.
    ///
    /// See [`crate::VisualizerQueryInfo::required`]
    requirement: Requirement,

    per_store_mapping: HashMap<StoreId, VisualizerEntityMapping>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AllComponentsRequirement {
    /// Assigns each required component an index.
    required_components_indices: IntMap<ComponentIdentifier, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnyComponentRequirement {
    relevant_components: ComponentSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnyPhysicalDatatypeRequirement {
    relevant_datatypes: DatatypeSet,
}

/// Internal representation of how to check required components.
///
/// Corresponds to [`RequiredComponents`].
#[derive(Debug, Clone, PartialEq, Eq)]
enum Requirement {
    /// All entities match.
    None,

    /// Entity must have all tracked components.
    AllComponents(AllComponentsRequirement),

    /// Entity must have at least one component.
    AnyComponent(AnyComponentRequirement),

    /// Entity must have at least one compatible data type.
    AnyPhysicalDatatype(AnyPhysicalDatatypeRequirement),
}

#[derive(Default)]
struct VisualizerEntityMapping {
    /// For each entity, which of the required components are present.
    ///
    /// Last bit is used for the data-based-visualizability filter.
    ///
    /// In order of `required_components`.
    // TODO(andreas): We could just limit the number of required components to 32 or 64 and
    // then use a single u32/u64 as a bitmap.
    required_component_and_filter_bitmap_per_entity: IntMap<EntityPathHash, BitVec>,

    /// Which entities the visualizer can be applied to.
    visualizable_entities: VisualizableEntities,

    /// List of all entities in this store that at some point in time had any of the relevant archetypes.
    ///
    /// Special case:
    /// If the visualizer has no relevant archetypes, this list will contain all entities in the store.
    indicated_entities: IndicatedEntities,
}

impl From<ComponentSet> for AllComponentsRequirement {
    fn from(value: ComponentSet) -> Self {
        Self {
            required_components_indices: value
                .into_iter()
                .enumerate()
                .map(|(i, name)| (name, i))
                .collect(),
        }
    }
}

impl From<ComponentSet> for AnyComponentRequirement {
    fn from(value: ComponentSet) -> Self {
        Self {
            relevant_components: value,
        }
    }
}

impl From<DatatypeSet> for AnyPhysicalDatatypeRequirement {
    fn from(value: DatatypeSet) -> Self {
        Self {
            relevant_datatypes: value,
        }
    }
}

impl From<RequiredComponents> for Requirement {
    fn from(value: RequiredComponents) -> Self {
        match value {
            RequiredComponents::None => Self::None,
            RequiredComponents::AllComponents(components) => Self::AllComponents(components.into()),
            RequiredComponents::AnyComponent(components) => Self::AnyComponent(components.into()),
            RequiredComponents::AnyPhysicalDatatype(datatypes) => {
                Self::AnyPhysicalDatatype(datatypes.into())
            }
        }
    }
}

impl VisualizerEntitySubscriber {
    pub fn new<T: IdentifiedViewSystem + VisualizerSystem>(
        visualizer: &T,
        app_options: &crate::AppOptions,
    ) -> Self {
        let visualizer_query_info = visualizer.visualizer_query_info(app_options);

        Self {
            visualizer: T::identifier(),
            relevant_archetype: visualizer_query_info.relevant_archetype,
            requirement: visualizer_query_info.required.into(),
            per_store_mapping: Default::default(),
        }
    }

    /// List of entities that are visualizable by the visualizer.
    #[inline]
    pub fn visualizable_entities(&self, store: &StoreId) -> Option<&VisualizableEntities> {
        self.per_store_mapping
            .get(store)
            .map(|mapping| &mapping.visualizable_entities)
    }

    /// List of entities that at some point in time had a component of an archetypes matching the visualizer's query.
    ///
    /// Useful for quickly evaluating basic "should this visualizer apply by default"-heuristic.
    /// Does *not* imply that any of the given entities is also in the visualizable-set!
    ///
    /// If the visualizer has no archetypes, this list will contain all entities in the store.
    pub fn indicated_entities(&self, store: &StoreId) -> Option<&IndicatedEntities> {
        self.per_store_mapping
            .get(store)
            .map(|mapping| &mapping.indicated_entities)
    }
}

impl ChunkStoreSubscriber for VisualizerEntitySubscriber {
    #[inline]
    fn name(&self) -> String {
        self.visualizer.as_str().to_owned()
    }

    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn on_events(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!(self.visualizer);

        // TODO(andreas): Need to react to store removals as well. As of writing doesn't exist yet.

        for event in events {
            if event.diff.kind != ChunkStoreDiffKind::Addition {
                // Visualizability is only additive, don't care about removals.
                continue;
            }

            let store_mapping = self
                .per_store_mapping
                .entry(event.store_id.clone())
                .or_default();

            let entity_path = event.diff.chunk_before_processing.entity_path();

            // Update archetype tracking:
            if self.relevant_archetype.is_none()
                || self.relevant_archetype.is_some_and(|archetype| {
                    event
                        .diff
                        .chunk_before_processing
                        .components()
                        .component_descriptors()
                        .any(|component_descr| component_descr.archetype == Some(archetype))
                })
            {
                store_mapping
                    .indicated_entities
                    .0
                    .insert(entity_path.clone());
            }

            // Check component requirements based on mode
            match &self.requirement {
                Requirement::None => {
                    // No requirements means that all entities are candidates.
                    re_log::trace!(
                        "Entity {:?} in store {:?} may now be visualizable by {:?} (no requirements)",
                        entity_path,
                        event.store_id,
                        self.visualizer
                    );

                    store_mapping
                        .visualizable_entities
                        .0
                        .insert(entity_path.clone(), VisualizableReason::Always);
                }
                Requirement::AllComponents(AllComponentsRequirement {
                    required_components_indices,
                }) => {
                    // Entity must have all required components
                    let required_components_bitmap = store_mapping
                                    .required_component_and_filter_bitmap_per_entity
                                    .entry(entity_path.hash())
                                    .or_insert_with(|| {
                                        // An empty set would mean that all entities will never be "visualizable",
                                        // because `.all()` is always false for an empty set.
                                        debug_assert!(
                                            !required_components_indices.is_empty(),
                                            "[DEBUG ASSERT] encountered empty set of required components for `RequiredComponentMode::All`"
                                        );
                                        BitVec::from_elem(required_components_indices.len(), false)
                                    });

                    // Early-out: if all required components are already present, we already
                    // marked this entity as visualizable in a previous event.
                    if required_components_bitmap.all() {
                        continue;
                    }

                    #[expect(clippy::iter_over_hash_type)]
                    for SerializedComponentColumn {
                        list_array,
                        descriptor,
                    } in event.diff.chunk_before_processing.components().values()
                    {
                        if let Some(index) = required_components_indices.get(&descriptor.component)
                        {
                            // The component might be present, but logged completely empty.
                            // That shouldn't count towards having the component present!
                            if !list_array.values().is_empty() {
                                required_components_bitmap.set(*index, true);
                            }
                        }
                    }

                    // Check if all required components are now present
                    if required_components_bitmap.all() {
                        re_log::trace!(
                            "Entity {:?} in store {:?} may now be visualizable by {:?}",
                            entity_path,
                            event.store_id,
                            self.visualizer
                        );

                        store_mapping
                            .visualizable_entities
                            .0
                            .insert(entity_path.clone(), VisualizableReason::ExactMatchAll);
                    }
                }
                Requirement::AnyComponent(AnyComponentRequirement {
                    relevant_components,
                }) => {
                    // Entity must have any of the required components
                    let mut has_any_component = false;

                    #[expect(clippy::iter_over_hash_type)]
                    for SerializedComponentColumn {
                        list_array,
                        descriptor,
                    } in event.diff.chunk_before_processing.components().values()
                    {
                        if relevant_components.contains(&descriptor.component) {
                            // The component might be present, but logged completely empty.
                            if !list_array.values().is_empty() {
                                has_any_component = true;
                                break;
                            }
                        }
                    }

                    if has_any_component {
                        re_log::trace!(
                            "Entity {:?} in store {:?} may now be visualizable by {:?} (has any required component)",
                            entity_path,
                            event.store_id,
                            self.visualizer
                        );

                        store_mapping
                            .visualizable_entities
                            .0
                            .insert(entity_path.clone(), VisualizableReason::ExactMatchAny);
                    }
                }
                Requirement::AnyPhysicalDatatype(AnyPhysicalDatatypeRequirement {
                    relevant_datatypes,
                }) => {
                    // Entity must have any of the required components
                    let mut has_any_datatype = false;

                    #[expect(clippy::iter_over_hash_type)]
                    for SerializedComponentColumn {
                        list_array,
                        descriptor,
                    } in event.diff.chunk_before_processing.components().values()
                    {
                        if relevant_datatypes.contains(&list_array.value_type()) {
                            // The component might be present, but logged completely empty.
                            if !list_array.values().is_empty() {
                                has_any_datatype = true;

                                // Track the component that matched
                                match store_mapping
                                    .visualizable_entities
                                    .0
                                    .entry(entity_path.clone())
                                {
                                    Entry::Occupied(mut occupied_entry) => {
                                        if let VisualizableReason::DatatypeMatchAny { components } =
                                            occupied_entry.get_mut()
                                        {
                                            components.push(descriptor.component);
                                        }
                                    }
                                    Entry::Vacant(vacant_entry) => {
                                        vacant_entry.insert(VisualizableReason::DatatypeMatchAny {
                                            components: SmallVec1::new(descriptor.component),
                                        });
                                    }
                                }
                            }
                        }
                    }

                    if has_any_datatype {
                        re_log::trace!(
                            "Entity {:?} in store {:?} may now be visualizable by {:?} (has any required datatype)",
                            entity_path,
                            event.store_id,
                            self.visualizer
                        );
                    }
                }
            }
        }
    }
}
