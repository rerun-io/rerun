use ahash::HashMap;
use bit_vec::BitVec;
use itertools::Itertools;
use nohash_hasher::{IntMap, IntSet};

use re_arrow_store::StoreSubscriber;
use re_log_types::{EntityPath, EntityPathHash, StoreId};
use re_types::{ComponentName, ComponentNameSet};

use crate::{IdentifiedViewSystem, ViewPartSystem, ViewSystemIdentifier};

/// List of entities that are *applicable* to a given visualizer.
///
/// An entity is applicable if it at any point in time on any timeline has all required components.
#[derive(Default, Clone)]
pub struct VisualizerApplicableEntities(pub IntSet<EntityPath>);

impl std::ops::Deref for VisualizerApplicableEntities {
    type Target = IntSet<EntityPath>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for VisualizerApplicableEntities {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// List of entities that are applicable to each visualizer.
///
/// See [`VisualizerApplicableEntities`].
pub struct ApplicableEntitiesPerVisualizer(
    pub IntMap<ViewSystemIdentifier, VisualizerApplicableEntities>,
);

impl std::ops::Deref for ApplicableEntitiesPerVisualizer {
    type Target = IntMap<ViewSystemIdentifier, VisualizerApplicableEntities>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for ApplicableEntitiesPerVisualizer {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A store subscriber that keep track which entities in a store can be
/// processed by a single given visualizer type.
///
/// The list of entities is additive:
/// If an entity was at any point in time applicable to the visualizer, it will be
/// kept in the list of entities.
///
/// Applicability is determined by the visualizer's set of required components.
///
/// There's only a single entity subscriber per visualizer *type*.
/// This means that if the same visualizer is used in multiple space views, only a single
/// `VisualizerEntitySubscriber` is created for all of them.
pub struct VisualizerEntitySubscriber {
    /// Visualizer type this subscriber is associated with.
    visualizer: ViewSystemIdentifier,

    /// See [`ViewPartSystem::indicator_components`]
    indicator_components: ComponentNameSet,

    /// Assigns each required component an index.
    required_components_indices: IntMap<ComponentName, usize>,

    per_store_mapping: HashMap<StoreId, VisualizerEntityMapping>,

    /// Additional filter for applicability.
    applicability_filter: Box<dyn VisualizerAdditionalApplicabilityFilter>,
}

/// Additional filter for applicability on top of the default check for required components.
pub trait VisualizerAdditionalApplicabilityFilter: Send + Sync {
    /// Updates the internal applicability filter state based on the given events.
    ///
    /// Called for every update no matter whether the entity is already has all required components or not.
    ///
    /// Returns true if the entity changed in the event is now applicable to the visualizer, false otherwise.
    /// Once a entity passes this filter, it can never go back to being filtered out.
    /// **This implies that the filter does not _need_ to be stateful.**
    /// It is perfectly fine to return `true` only if something in the diff is regarded as applicable and false otherwise.
    /// (However, if necessary, the applicability filter *can* keep track of state.)
    fn update_applicability(&mut self, _event: &re_arrow_store::StoreEvent) -> bool;
}

struct DefaultVisualizerApplicabilityFilter;

impl VisualizerAdditionalApplicabilityFilter for DefaultVisualizerApplicabilityFilter {
    #[inline]
    fn update_applicability(&mut self, _event: &re_arrow_store::StoreEvent) -> bool {
        true
    }
}

#[derive(Default)]
struct VisualizerEntityMapping {
    /// For each entity, which of the required components are present.
    ///
    /// Last bit is used for the applicability filter.
    ///
    /// In order of `required_components`.
    /// If all bits are set, the entity is applicable to the visualizer.
    // TODO(andreas): We could just limit the number of required components to 32 or 64 and
    // then use a single u32/u64 as a bitmap.
    required_component_and_filter_bitmap_per_entity: IntMap<EntityPathHash, BitVec>,

    /// Which entities the visualizer can be applied to.
    applicable_entities: VisualizerApplicableEntities,

    /// List of all entities in this store that at some point in time had any of the indicator components.
    indicator_matching_entities: IntSet<EntityPathHash>,
}

impl VisualizerEntitySubscriber {
    pub fn new<T: IdentifiedViewSystem + ViewPartSystem>(visualizer: &T) -> Self {
        Self {
            visualizer: T::identifier(),
            indicator_components: visualizer.indicator_components(),
            required_components_indices: visualizer
                .required_components()
                .into_iter()
                .enumerate()
                .map(|(i, name)| (name, i))
                .collect(),
            per_store_mapping: Default::default(),
            applicability_filter: visualizer
                .applicability_filter()
                .unwrap_or_else(|| Box::new(DefaultVisualizerApplicabilityFilter)),
        }
    }

    /// List of entities that are applicable to the visualizer.
    #[inline]
    pub fn applicable_entities(&self, store: &StoreId) -> Option<&VisualizerApplicableEntities> {
        self.per_store_mapping
            .get(store)
            .map(|mapping| &mapping.applicable_entities)
    }

    /// List of entities that at some point in time had any of the indicator components advertised by this visualizer.
    ///
    /// Useful for quickly evaluating basic "should this visualizer apply by default"-heuristic.
    /// Does *not* imply that any of the given entities is also in the applicable-set!
    pub fn indicator_matching_entities(&self, store: &StoreId) -> Option<&IntSet<EntityPathHash>> {
        self.per_store_mapping
            .get(store)
            .map(|mapping| &mapping.indicator_matching_entities)
    }
}

impl StoreSubscriber for VisualizerEntitySubscriber {
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

    fn on_events(&mut self, events: &[re_arrow_store::StoreEvent]) {
        re_tracing::profile_function!(self.visualizer);

        // TODO(andreas): Need to react to store removals as well. As of writing doesn't exist yet.

        for event in events {
            if event.diff.kind != re_arrow_store::StoreDiffKind::Addition {
                // Applicability is only additive, don't care about removals.
                continue;
            }

            let store_mapping = self
                .per_store_mapping
                .entry(event.store_id.clone())
                .or_default();

            let entity_path = &event.diff.entity_path;
            let entity_path_hash = entity_path.hash();

            // Update indicator component tracking:
            if self
                .indicator_components
                .iter()
                .any(|component_name| event.diff.cells.keys().contains(component_name))
            {
                store_mapping
                    .indicator_matching_entities
                    .insert(entity_path_hash);
            }

            // Update required component tracking:
            let required_components_bitmap = store_mapping
                .required_component_and_filter_bitmap_per_entity
                .entry(entity_path_hash)
                .or_insert_with(|| {
                    BitVec::from_elem(self.required_components_indices.len() + 1, false)
                });

            if required_components_bitmap.all() {
                // We already know that this entity is applicable to the visualizer.
                continue;
            }

            for component_name in event.diff.cells.keys() {
                if let Some(index) = self.required_components_indices.get(component_name) {
                    required_components_bitmap.set(*index, true);
                }
            }

            let bit_index_for_filter = self.required_components_indices.len();
            let custom_filter = required_components_bitmap[bit_index_for_filter];
            if !custom_filter {
                required_components_bitmap.set(
                    bit_index_for_filter,
                    self.applicability_filter.update_applicability(event),
                );
            }

            if required_components_bitmap.all() {
                re_log::debug!(
                    "Entity {:?} in store {:?} is now applicable to visualizer {:?}",
                    entity_path,
                    event.store_id,
                    self.visualizer
                );

                store_mapping
                    .applicable_entities
                    .insert(entity_path.clone());
            }
        }
    }
}
