use std::collections::hash_map::Entry;
use std::sync::Arc;

use ahash::HashMap;
use bit_vec::BitVec;
use nohash_hasher::{IntMap, IntSet};
use re_arrow_combinators::extract_nested_fields;
use re_chunk::{ArchetypeName, ComponentIdentifier, ComponentType};
use re_chunk_store::{ChunkStoreEvent, ChunkStoreSubscriber};
use re_log::{debug_assert, debug_panic};
use re_log_types::{EntityPath, EntityPathHash, StoreId};
use re_sdk_types::ComponentSet;

use crate::typed_entity_collections::DatatypeMatch;
use crate::view::visualizer_system::{AnyPhysicalDatatypeRequirement, DatatypeSet};
use crate::{
    IdentifiedViewSystem, IndicatedEntities, RequiredComponents, ViewSystemIdentifier,
    VisualizableEntities, VisualizerSystem, typed_entity_collections::VisualizableReason,
};

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

    /// Lists all known builtin enums components.
    ///
    /// Used by [`Requirement::AnyPhysicalDatatype`] to skip physical-only matches
    /// for enum types (which should only match via native semantics).
    // TODO(andreas): It would be great if we could just always access the latest reflection data, but this is really hard to pipe through to a store subscriber.
    known_builtin_enum_components: Arc<IntSet<ComponentType>>,

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

impl From<RequiredComponents> for Requirement {
    fn from(value: RequiredComponents) -> Self {
        match value {
            RequiredComponents::None => Self::None,
            RequiredComponents::AllComponents(components) => Self::AllComponents(components.into()),
            RequiredComponents::AnyComponent(components) => Self::AnyComponent(components.into()),
            RequiredComponents::AnyPhysicalDatatype(requirement) => {
                Self::AnyPhysicalDatatype(requirement)
            }
        }
    }
}

impl VisualizerEntitySubscriber {
    pub fn new<T: IdentifiedViewSystem + VisualizerSystem>(
        visualizer: &T,
        known_builtin_enum_components: Arc<IntSet<ComponentType>>,
        app_options: &crate::AppOptions,
    ) -> Self {
        let visualizer_query_info = visualizer.visualizer_query_info(app_options);

        Self {
            visualizer: T::identifier(),
            relevant_archetype: visualizer_query_info.relevant_archetype,
            requirement: visualizer_query_info.required.into(),
            known_builtin_enum_components,
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

/// Process a single entity's components and update the visualizer entity mapping.
///
/// This is the shared core logic between physical chunk additions and virtual manifest additions.
fn process_entity_components(
    relevant_archetype: Option<ArchetypeName>,
    requirement: &Requirement,
    visualizer: &ViewSystemIdentifier,
    known_enum_types: &IntSet<ComponentType>,
    store_mapping: &mut VisualizerEntityMapping,
    store_id: &StoreId,
    re_chunk_store::ChunkMeta {
        entity_path,
        components,
    }: re_chunk_store::ChunkMeta,
) {
    // Update indicated_entities.
    if relevant_archetype.is_none()
        || relevant_archetype.is_some_and(|archetype| {
            components
                .iter()
                .any(|c| c.descriptor.archetype == Some(archetype))
        })
    {
        store_mapping
            .indicated_entities
            .0
            .insert(entity_path.clone());
    }

    // Check component requirements.
    match requirement {
        Requirement::None => {
            re_log::trace!(
                "Entity {entity_path:?} in store {store_id:?} may now be visualizable by {visualizer:?} (no requirements)",
            );

            store_mapping
                .visualizable_entities
                .0
                .insert(entity_path.clone(), VisualizableReason::Always);
        }

        Requirement::AllComponents(AllComponentsRequirement {
            required_components_indices,
        }) => {
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
                return;
            }

            for c in components {
                if let Some(index) = required_components_indices.get(&c.descriptor.component)
                    && c.has_data
                {
                    required_components_bitmap.set(*index, true);
                }
            }

            if required_components_bitmap.all() {
                re_log::trace!(
                    "Entity {entity_path:?} in store {store_id:?} may now be visualizable by {visualizer:?}",
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
            let has_any_component = components
                .iter()
                .any(|c| relevant_components.contains(&c.descriptor.component) && c.has_data);

            if has_any_component {
                re_log::trace!(
                    "Entity {entity_path:?} in store {store_id:?} may now be visualizable by {visualizer:?} (has any required component)",
                );

                store_mapping
                    .visualizable_entities
                    .0
                    .insert(entity_path.clone(), VisualizableReason::ExactMatchAny);
            }
        }

        Requirement::AnyPhysicalDatatype(AnyPhysicalDatatypeRequirement {
            target_component,
            semantic_type,
            physical_types,
            allow_static_data,
        }) => {
            let mut has_any_datatype = false;

            for c in components {
                if !allow_static_data && c.is_static_only {
                    continue;
                }

                let Some(arrow_datatype) = &c.inner_arrow_datatype else {
                    continue;
                };

                if let Some(match_info) = check_datatype_match(
                    known_enum_types,
                    arrow_datatype,
                    c.descriptor.component_type,
                    semantic_type,
                    physical_types,
                    c.descriptor.component,
                ) && c.has_data
                {
                    has_any_datatype = true;
                    insert_datatype_match(
                        &mut store_mapping.visualizable_entities,
                        &entity_path,
                        c.descriptor.component,
                        *target_component,
                        match_info,
                        visualizer,
                    );
                }
            }

            if has_any_datatype {
                re_log::trace!(
                    "Entity {entity_path:?} in store {store_id:?} may now be visualizable by {visualizer:?} (has any required datatype)",
                );
            }
        }
    }
}

/// Check if an Arrow datatype matches the physical/semantic requirements.
fn check_datatype_match(
    known_enum_types: &IntSet<ComponentType>,
    arrow_datatype: &arrow::datatypes::DataType,
    component_type: Option<ComponentType>,
    semantic_type: &ComponentType,
    physical_types: &DatatypeSet,
    component: ComponentIdentifier,
) -> Option<DatatypeMatch> {
    let is_physical_match = physical_types.contains(arrow_datatype);
    let is_semantic_match = component_type == Some(*semantic_type);

    // Builtin enum types (registered in the reflection) should only
    // match via native semantics, never via physical datatype alone.
    // This prevents e.g. a `rerun.components.FillMode` (UInt8) from
    // being picked up by a visualizer that happens to accept UInt8 data.
    let is_known_enum = component_type.is_some_and(|ct| known_enum_types.contains(&ct));
    if is_known_enum && !is_semantic_match {
        return None;
    }

    match (is_physical_match, is_semantic_match) {
        (false, false) => {
            // No direct match - try nested field access
            extract_nested_fields(arrow_datatype, |dt| physical_types.contains(dt)).map(
                |selectors| DatatypeMatch::PhysicalDatatypeOnly {
                    arrow_datatype: arrow_datatype.clone(),
                    component_type,
                    selectors: selectors.into(),
                },
            )
        }

        (true, false) => Some(DatatypeMatch::PhysicalDatatypeOnly {
            arrow_datatype: arrow_datatype.clone(),
            component_type,
            selectors: Vec::new(),
        }),

        (true, true) => Some(DatatypeMatch::NativeSemantics {
            arrow_datatype: arrow_datatype.clone(),
            component_type,
        }),

        (false, true) => {
            re_log::warn_once!(
                "Component {component:?} matched semantic type {semantic_type:?} but none of the expected physical arrow types {arrow_datatype:?} for this semantic type.",
            );
            None
        }
    }
}

/// Insert a datatype match for an entity into the visualizable entities map.
fn insert_datatype_match(
    visualizable_entities: &mut VisualizableEntities,
    entity_path: &EntityPath,
    component: ComponentIdentifier,
    target_component: ComponentIdentifier,
    match_info: DatatypeMatch,
    visualizer: &ViewSystemIdentifier,
) {
    match visualizable_entities.0.entry(entity_path.clone()) {
        Entry::Occupied(mut occupied_entry) => {
            if let VisualizableReason::DatatypeMatchAny {
                matches,
                target_component: previous_target,
            } = occupied_entry.get_mut()
            {
                re_log::debug_assert_eq!(&target_component, previous_target);
                matches.insert(component, match_info);
            } else {
                debug_panic!(
                    "entity {entity_path:?} already marked visualizable for visualizer {visualizer:?} with a different reason than `DatatypeMatchAny`",
                );
            }
        }

        Entry::Vacant(vacant_entry) => {
            vacant_entry.insert(VisualizableReason::DatatypeMatchAny {
                target_component,
                matches: std::iter::once((component, match_info)).collect(),
            });
        }
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
        //                These removals also need to keep in mind that things from the rrd manifest
        //                shouldn't be removed.

        for event in events {
            let store_mapping = self
                .per_store_mapping
                .entry(event.store_id.clone())
                .or_default();

            match &event.diff {
                re_chunk_store::ChunkStoreDiff::Addition(add) => {
                    // This is a purely additive datastructure, and it doesn't keep track of actual chunks,
                    // just the bits of data that are of actual interest.
                    // Therefore, the meta of the delta chunk is all we need, always.

                    process_entity_components(
                        self.relevant_archetype,
                        &self.requirement,
                        &self.visualizer,
                        &self.known_builtin_enum_components,
                        store_mapping,
                        &event.store_id,
                        add.chunk_meta(),
                    );
                }
                re_chunk_store::ChunkStoreDiff::VirtualAddition(virtual_add) => {
                    for meta in virtual_add.chunk_metas() {
                        process_entity_components(
                            self.relevant_archetype,
                            &self.requirement,
                            &self.visualizer,
                            &self.known_builtin_enum_components,
                            store_mapping,
                            &event.store_id,
                            meta,
                        );
                    }
                }
                re_chunk_store::ChunkStoreDiff::Deletion(_) => {
                    // Not handling deletions here yet.
                }
            }
        }
    }
}
