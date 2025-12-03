use ahash::HashMap;
use bit_vec::BitVec;
use nohash_hasher::IntMap;
use re_chunk::{ArchetypeName, ComponentIdentifier};
use re_chunk_store::{ChunkStoreDiffKind, ChunkStoreEvent, ChunkStoreSubscriber};
use re_log_types::{EntityPathHash, StoreId};
use re_types_core::SerializedComponentColumn;

use crate::{
    IdentifiedViewSystem, IndicatedEntities, RequiredComponents, ViewSystemIdentifier,
    VisualizableEntities, VisualizerSystem,
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
    requirement_mode: RequiredComponentMode,

    /// Assigns each required component an index.
    ///
    /// The components stored in here, and how the are handled depends on [`Self::requirement_mode`].
    required_components_indices: IntMap<ComponentIdentifier, usize>,

    per_store_mapping: HashMap<StoreId, VisualizerEntityMapping>,
}

/// Internal representation of how to check required components.
///
/// Corresponds to [`RequiredComponents`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequiredComponentMode {
    /// All entities match.
    None,

    /// Entity must have all tracked components.
    All,

    /// Entity must have at least one component.
    Any,
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

impl VisualizerEntitySubscriber {
    pub fn new<T: IdentifiedViewSystem + VisualizerSystem>(visualizer: &T) -> Self {
        let visualizer_query_info = visualizer.visualizer_query_info();

        let (requirement_mode, required_components) = match visualizer_query_info.required {
            RequiredComponents::None => (RequiredComponentMode::None, Default::default()),
            RequiredComponents::All(components) => (RequiredComponentMode::All, components),
            RequiredComponents::Any(components) => (RequiredComponentMode::Any, components),
        };

        Self {
            visualizer: T::identifier(),
            relevant_archetype: visualizer_query_info.relevant_archetype,
            requirement_mode,
            required_components_indices: required_components
                .into_iter()
                .enumerate()
                .map(|(i, name)| (name, i))
                .collect(),
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

            let entity_path = event.diff.chunk.entity_path();

            // Update archetype tracking:
            if self.relevant_archetype.is_none()
                || self.relevant_archetype.is_some_and(|archetype| {
                    event
                        .diff
                        .chunk
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
            match self.requirement_mode {
                RequiredComponentMode::None => {
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
                        .insert(entity_path.clone());
                }
                RequiredComponentMode::All => {
                    // Entity must have all required components
                    let required_components_bitmap = store_mapping
                        .required_component_and_filter_bitmap_per_entity
                        .entry(entity_path.hash())
                        .or_insert_with(|| {
                            // An empty set would mean that all entities will never be "visualizable",
                            // because `.all()` is always false for an empty set.
                            debug_assert!(
                                !self.required_components_indices.is_empty(),
                                "[DEBUG ASSERT] encountered empty set of required components for `RequiredComponentMode::All`"
                            );
                            BitVec::from_elem(self.required_components_indices.len(), false)
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
                    } in event.diff.chunk.components().values()
                    {
                        if let Some(index) =
                            self.required_components_indices.get(&descriptor.component)
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
                            .insert(entity_path.clone());
                    }
                }
                RequiredComponentMode::Any => {
                    // Entity must have any of the required components
                    let mut has_any_component = false;

                    #[expect(clippy::iter_over_hash_type)]
                    for SerializedComponentColumn {
                        list_array,
                        descriptor,
                    } in event.diff.chunk.components().values()
                    {
                        if self
                            .required_components_indices
                            .contains_key(&descriptor.component)
                        {
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
                            .insert(entity_path.clone());
                    }
                }
            }
        }
    }
}
