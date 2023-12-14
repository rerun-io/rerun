use ahash::HashMap;
use bit_vec::BitVec;
use nohash_hasher::IntMap;

use re_arrow_store::StoreSubscriber;
use re_log_types::{EntityPath, EntityPathHash, StoreId};
use re_types::ComponentName;

use crate::{IdentifiedViewSystem, ViewPartSystem, ViewSystemIdentifier};

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

    /// Assigns each required component an index.
    required_components_indices: IntMap<ComponentName, usize>,

    per_store_mapping: HashMap<StoreId, VisualizerEntityMapping>,
}

#[derive(Default)]
struct VisualizerEntityMapping {
    /// For each entity, which of the required components are present.
    /// In order of `required_components`.
    /// If all bits are set, the entity is applicable to the visualizer.
    // TODO(andreas): We could just limit the number of required components to 32 or 64 and
    // then use a single u32/u64 as a bitmap.
    required_component_bitmap_per_entity: IntMap<EntityPathHash, BitVec>,

    /// Which entities the visualizer can be applied to.
    ///
    /// Guaranteed to not have any duplicate entries.
    /// Order is not defined.
    applicable_entities: Vec<EntityPath>,
}

impl VisualizerEntitySubscriber {
    pub fn new<T: IdentifiedViewSystem + ViewPartSystem>(visualizer: &T) -> Self {
        Self {
            visualizer: T::identifier(),
            required_components_indices: visualizer
                .required_components()
                .into_iter()
                .enumerate()
                .map(|(i, name)| (name, i))
                .collect(),
            per_store_mapping: Default::default(),
        }
    }

    /// List of entities that are applicable to the visualizer.
    #[inline]
    pub fn entities(&self, store: &StoreId) -> Option<&[EntityPath]> {
        self.per_store_mapping
            .get(store)
            .map(|mapping| mapping.applicable_entities.as_slice())
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

            let required_components_bitmap = store_mapping
                .required_component_bitmap_per_entity
                .entry(event.diff.entity_path.hash())
                .or_insert_with(|| {
                    BitVec::from_elem(self.required_components_indices.len(), false)
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

            if required_components_bitmap.all() {
                store_mapping
                    .applicable_entities
                    .push(event.diff.entity_path.clone());
            }
        }
    }
}
