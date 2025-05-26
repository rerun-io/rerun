use ahash::HashMap;
use bit_vec::BitVec;
use nohash_hasher::IntMap;

use re_chunk_store::{ChunkStoreDiffKind, ChunkStoreEvent, ChunkStoreSubscriber};
use re_log_types::{EntityPathHash, StoreId};
use re_types::{ComponentDescriptor, ComponentDescriptorSet};

use crate::{
    IdentifiedViewSystem, IndicatedEntities, MaybeVisualizableEntities, ViewSystemIdentifier,
    VisualizerSystem,
};

/// A store subscriber that keep track which entities in a store can be
/// processed by a single given visualizer type.
///
/// The list of entities is additive:
/// If an entity was at any point in time passes the "maybe visualizable" filter for the visualizer, it will be
/// kept in the list of entities.
///
/// "maybe visualizable" is determined by..
/// * set of required components
/// * additional custom data based criteria a visualizer may set
///
/// There's only a single entity subscriber per visualizer *type*.
/// This means that if the same visualizer is used in multiple views, only a single
/// `VisualizerEntitySubscriber` is created for all of them.
pub struct VisualizerEntitySubscriber {
    /// Visualizer type this subscriber is associated with.
    visualizer: ViewSystemIdentifier,

    /// See [`crate::VisualizerQueryInfo::indicators`]
    indicator_components: ComponentDescriptorSet,

    /// Assigns each required component an index.
    required_components_indices: IntMap<ComponentDescriptor, usize>,

    per_store_mapping: HashMap<StoreId, VisualizerEntityMapping>,

    /// Additional filter for visualizability.
    additional_filter: Box<dyn DataBasedVisualizabilityFilter>,
}

// TODO(#6889): Create writeup for things that changed and an issue for how things should move forward (i.e. descriptor overrides).
/// Additional filter for visualizability on top of the default check for required components.
///
/// This is part of the "maybe visualizable" criteria.
/// I.e. if this (and the required components) are passed, an entity is deemed "maybe visualizable"
/// on all timelines & time points.
/// However, there might be additional view instance based filters that prune this set further to the final
/// "visualizable" set.
pub trait DataBasedVisualizabilityFilter: Send + Sync {
    /// Updates the internal visualizability filter state based on the given events.
    ///
    /// Called for every update no matter whether the entity is already has all required components or not.
    ///
    /// Returns true if the entity changed in the event is now visualizable to the visualizer (bar any view dependent restrictions), false otherwise.
    /// Once a entity passes this filter, it can never go back to being filtered out.
    /// **This implies that the filter does not _need_ to be stateful.**
    /// It is perfectly fine to return `true` only if some aspect in the diff is regarded as visualizable and false otherwise.
    /// (However, if necessary, the filter *can* keep track of state.)
    fn update_visualizability(&mut self, _event: &ChunkStoreEvent) -> bool;
}

struct DefaultVisualizabilityFilter;

impl DataBasedVisualizabilityFilter for DefaultVisualizabilityFilter {
    #[inline]
    fn update_visualizability(&mut self, _event: &ChunkStoreEvent) -> bool {
        true
    }
}

#[derive(Default)]
struct VisualizerEntityMapping {
    /// For each entity, which of the required components are present.
    ///
    /// Last bit is used for the data-based-visualizability filter.
    ///
    /// In order of `required_components`.
    /// If all bits are set, the entity is "maybe visualizable" to the visualizer.
    // TODO(andreas): We could just limit the number of required components to 32 or 64 and
    // then use a single u32/u64 as a bitmap.
    required_component_and_filter_bitmap_per_entity: IntMap<EntityPathHash, BitVec>,

    /// Which entities the visualizer can be applied to.
    maybe_visualizable_entities: MaybeVisualizableEntities,

    /// List of all entities in this store that at some point in time had any of the indicator components.
    ///
    /// Special case:
    /// If the visualizer has no indicator components, this list will contain all entities in the store.
    indicated_entities: IndicatedEntities,
}

impl VisualizerEntitySubscriber {
    pub fn new<T: IdentifiedViewSystem + VisualizerSystem>(visualizer: &T) -> Self {
        let visualizer_query_info = visualizer.visualizer_query_info();

        Self {
            visualizer: T::identifier(),
            indicator_components: visualizer_query_info.indicators,
            required_components_indices: visualizer_query_info
                .required
                .into_iter()
                .enumerate()
                .map(|(i, name)| (name, i))
                .collect(),
            per_store_mapping: Default::default(),
            additional_filter: visualizer
                .data_based_visualizability_filter()
                .unwrap_or_else(|| Box::new(DefaultVisualizabilityFilter)),
        }
    }

    /// List of entities that are may be visualizable by the visualizer.
    #[inline]
    pub fn maybe_visualizable_entities(
        &self,
        store: &StoreId,
    ) -> Option<&MaybeVisualizableEntities> {
        self.per_store_mapping
            .get(store)
            .map(|mapping| &mapping.maybe_visualizable_entities)
    }

    /// List of entities that at some point in time had any of the indicator components advertised by this visualizer.
    ///
    /// Useful for quickly evaluating basic "should this visualizer apply by default"-heuristic.
    /// Does *not* imply that any of the given entities is also in the (maybe-)visualizable-set!
    ///
    /// If the visualizer has no indicator components, this list will contain all entities in the store.
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

            // Update indicator component tracking:
            if self.indicator_components.is_empty()
                || self.indicator_components.iter().any(|component_descr| {
                    event
                        .diff
                        .chunk
                        .components()
                        .contains_component(component_descr)
                })
            {
                store_mapping
                    .indicated_entities
                    .0
                    .insert(entity_path.clone());
            }

            // Update required component tracking:
            let required_components_bitmap = store_mapping
                .required_component_and_filter_bitmap_per_entity
                .entry(entity_path.hash())
                .or_insert_with(|| {
                    BitVec::from_elem(self.required_components_indices.len() + 1, false)
                });

            if required_components_bitmap.all() {
                // We already know that this entity is visualizable to the visualizer.
                continue;
            }

            for (component_desc, list_array) in event.diff.chunk.components().iter() {
                if let Some(index) = self.required_components_indices.get(component_desc) {
                    // The component might be present, but logged completely empty.
                    // That shouldn't count towards filling "having the required component present"!
                    // (Note: This happens frequently now with `Transform3D`'s component which always get logged, thus tripping of the `AxisLengthDetector`!)` )
                    if !list_array.values().is_empty() {
                        required_components_bitmap.set(*index, true);
                    }
                }
            }

            let bit_index_for_filter = self.required_components_indices.len();
            let custom_filter = required_components_bitmap[bit_index_for_filter];
            if !custom_filter {
                required_components_bitmap.set(
                    bit_index_for_filter,
                    self.additional_filter.update_visualizability(event),
                );
            }

            if required_components_bitmap.all() {
                re_log::trace!(
                    "Entity {:?} in store {:?} may now be visualizable by {:?}",
                    entity_path,
                    event.store_id,
                    self.visualizer
                );

                store_mapping
                    .maybe_visualizable_entities
                    .0
                    .insert(entity_path.clone());
            }
        }
    }
}
