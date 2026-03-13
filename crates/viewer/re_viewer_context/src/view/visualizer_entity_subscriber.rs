use std::collections::hash_map::Entry;
use std::sync::Arc;

use nohash_hasher::IntSet;
use re_chunk::{ArchetypeName, ComponentIdentifier, ComponentType};
use re_chunk_store::ChunkStoreEvent;
use re_log::debug_panic;
use re_log_types::{EntityPath, StoreId};

use nohash_hasher::IntMap;

use crate::typed_entity_collections::{
    BufferAndFormatMatch, DatatypeMatch, SingleRequiredComponentMatch, VisualizableReason,
};
use crate::{
    IndicatedEntities, ViewSystemIdentifier, VisualizabilityConstraints, VisualizableEntities,
};

/// Configuration data needed to build a [`VisualizerEntitySubscriber`].
///
/// This is the immutable "template" stored in the [`crate::ViewClassRegistry`],
/// extracted from a visualizer's query info at registration time.
#[derive(Clone)] // Cheap to clone; uses ref-counted data internally.
pub struct VisualizerEntityConfig {
    /// Visualizer type this config is associated with.
    pub visualizer: ViewSystemIdentifier,

    /// See [`crate::VisualizerQueryInfo::relevant_archetype`]
    pub relevant_archetype: Option<ArchetypeName>,

    /// The mode for checking component requirements.
    ///
    /// See [`crate::VisualizerQueryInfo::constraints`]
    pub constraints: Arc<VisualizabilityConstraints>,

    /// Lists all known builtin enums components.
    ///
    /// Used by [`VisualizabilityConstraints::SingleRequiredComponent`] to skip physical-only matches
    /// for enum types (which should only match via native semantics).
    // TODO(andreas): It would be great if we could just always access the latest reflection data, but this is really hard to pipe through to a store subscriber.
    pub known_builtin_enum_components: Arc<IntSet<ComponentType>>,
}

impl re_byte_size::SizeBytes for VisualizerEntityConfig {
    fn heap_size_bytes(&self) -> u64 {
        0 // We use Arc:s, so this is more or less amortized
    }
}

impl VisualizerEntityConfig {
    /// Create a new [`VisualizerEntitySubscriber`] from this config with empty per-store data.
    pub fn create_subscriber(&self) -> VisualizerEntitySubscriber {
        VisualizerEntitySubscriber {
            config: self.clone(),
            mapping: Default::default(),
        }
    }
}

/// A per-store subscriber that tracks which entities can be
/// processed by a single given visualizer type.
///
/// The list of entities is additive:
/// If an entity was at any point in time passes the "visualizable" filter for the visualizer, it will be
/// kept in the list of entities.
///
/// "visualizable" is determined by the set of required components
///
/// There's only a single entity subscriber per visualizer *type* per store.
pub struct VisualizerEntitySubscriber {
    config: VisualizerEntityConfig,
    mapping: VisualizerEntityMapping,
}

impl re_byte_size::SizeBytes for VisualizerEntitySubscriber {
    fn heap_size_bytes(&self) -> u64 {
        re_tracing::profile_function!();
        let Self { config, mapping } = self;
        config.heap_size_bytes() + mapping.heap_size_bytes()
    }
}

/// Per-entity state for a [`VisualizabilityConstraints::BufferAndFormat`] constraint.
///
/// Buffer and format components may arrive in separate chunk store events, so we keep accumulating them here.
#[derive(Default)]
struct BufferAndFormatEntityState {
    all_buffer_matches: IntMap<ComponentIdentifier, DatatypeMatch>,
    all_formats_matches: IntSet<ComponentIdentifier>,
}

impl re_byte_size::SizeBytes for BufferAndFormatEntityState {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            all_buffer_matches,
            all_formats_matches,
        } = self;
        all_buffer_matches.heap_size_bytes() + all_formats_matches.heap_size_bytes()
    }
}

#[derive(Default)]
struct VisualizerEntityMapping {
    /// Which entities the visualizer can be applied to.
    visualizable_entities: VisualizableEntities,

    /// List of all entities in this store that at some point in time had any of the relevant archetypes.
    ///
    /// Special case:
    /// If the visualizer has no relevant archetypes, this list will contain all entities in the store.
    indicated_entities: IndicatedEntities,

    /// Per-entity state for [`VisualizabilityConstraints::BufferAndFormat`] constraints.
    ///
    /// Only populated when the requirement is [`VisualizabilityConstraints::BufferAndFormat`].
    buffer_and_format_state: IntMap<EntityPath, BufferAndFormatEntityState>,
}

impl re_byte_size::SizeBytes for VisualizerEntityMapping {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            visualizable_entities,
            indicated_entities,
            buffer_and_format_state,
        } = self;
        visualizable_entities.heap_size_bytes()
            + indicated_entities.heap_size_bytes()
            + buffer_and_format_state.heap_size_bytes()
    }
}

impl VisualizerEntityMapping {
    /// Adds a visualizability reason for the given entity and combines it with an existing one if any.
    ///
    /// Changing the type of reason is a usage error and will cause a debug panic and is ignored on release builds.
    fn add_visualizability_reason(
        &mut self,
        entity_path: &EntityPath,
        visualizer: &ViewSystemIdentifier,
        new_reason: VisualizableReason,
    ) {
        match self.visualizable_entities.0.entry(entity_path.clone()) {
            Entry::Occupied(mut occupied_entry) => {
                let debug_panic_for_incompatible_reason = || {
                    debug_panic!(
                        "entity {entity_path:?} already marked visualizable for visualizer {visualizer:?} with an incompatible reason",
                    );
                };

                match occupied_entry.get_mut() {
                    VisualizableReason::Always => {
                        if matches!(new_reason, VisualizableReason::Always) {
                            // No change, already visualizable for all reasons.
                        } else {
                            debug_panic_for_incompatible_reason();
                        }
                    }

                    VisualizableReason::ExactMatchAny => {
                        if matches!(new_reason, VisualizableReason::ExactMatchAny) {
                            // No change, already visualizable for any builtin component.
                        } else {
                            debug_panic_for_incompatible_reason();
                        }
                    }

                    VisualizableReason::SingleRequiredComponentMatch(matches) => {
                        if let VisualizableReason::SingleRequiredComponentMatch(new_match) =
                            new_reason
                        {
                            re_log::debug_assert_eq!(
                                new_match.target_component,
                                matches.target_component
                            );
                            matches.matches.extend(new_match.matches);
                        } else {
                            debug_panic_for_incompatible_reason();
                        }
                    }

                    VisualizableReason::BufferAndFormatMatch(matches) => {
                        if let VisualizableReason::BufferAndFormatMatch(new_match) = new_reason {
                            re_log::debug_assert_eq!(
                                new_match.buffer_target,
                                matches.buffer_target
                            );
                            re_log::debug_assert_eq!(
                                new_match.format_target,
                                matches.format_target
                            );
                            matches.buffer_matches.extend(new_match.buffer_matches);
                            matches.format_matches.extend(new_match.format_matches);
                        } else {
                            debug_panic_for_incompatible_reason();
                        }
                    }
                }
            }

            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(new_reason);
            }
        }
    }
}

impl VisualizerEntitySubscriber {
    /// List of entities that are visualizable by the visualizer.
    #[inline]
    pub fn visualizable_entities(&self) -> &VisualizableEntities {
        &self.mapping.visualizable_entities
    }

    /// List of entities that at some point in time had a component of an archetypes matching the visualizer's query.
    ///
    /// Useful for quickly evaluating basic "should this visualizer apply by default"-heuristic.
    /// Does *not* imply that any of the given entities is also in the visualizable-set!
    ///
    /// If the visualizer has no archetypes, this list will contain all entities in the store.
    pub fn indicated_entities(&self) -> &IndicatedEntities {
        &self.mapping.indicated_entities
    }
}

/// Process a single entity's components and update the visualizer entity mapping.
///
/// This is the shared core logic between physical chunk additions and virtual manifest additions.
fn process_entity_components(
    config: &VisualizerEntityConfig,
    store_mapping: &mut VisualizerEntityMapping,
    store_id: &StoreId,
    re_chunk_store::ChunkMeta {
        entity_path,
        components,
    }: re_chunk_store::ChunkMeta,
) {
    let VisualizerEntityConfig {
        relevant_archetype,
        constraints,
        visualizer,
        known_builtin_enum_components,
    } = config;

    // Update indicated_entities.
    if relevant_archetype.is_none_or(|archetype| {
        components
            .iter()
            .any(|c| c.descriptor.archetype == Some(archetype))
    }) {
        store_mapping
            .indicated_entities
            .0
            .insert(entity_path.clone());
    }

    // Check component requirements.
    match constraints.as_ref() {
        VisualizabilityConstraints::None => {
            re_log::trace!(
                "Entity {entity_path:?} in store {store_id:?} may now be visualizable by {visualizer:?} (no requirements)",
            );

            store_mapping
                .visualizable_entities
                .0
                .insert(entity_path.clone(), VisualizableReason::Always);
        }

        VisualizabilityConstraints::AnyBuiltinComponent(relevant_components) => {
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

        VisualizabilityConstraints::SingleRequiredComponent(constraint) => {
            let mut has_any_datatype = false;

            for c in components {
                if !constraint.allow_static_data() && c.is_static_only {
                    continue;
                }

                let Some(arrow_datatype) = &c.inner_arrow_datatype else {
                    continue;
                };

                if let Some(match_info) = constraint.check_datatype_match(
                    known_builtin_enum_components,
                    arrow_datatype,
                    c.descriptor.component_type,
                    c.descriptor.component,
                ) && c.has_data
                {
                    has_any_datatype = true;

                    store_mapping.add_visualizability_reason(
                        &entity_path,
                        visualizer,
                        VisualizableReason::SingleRequiredComponentMatch(
                            SingleRequiredComponentMatch {
                                target_component: constraint.target_component(),
                                matches: std::iter::once((c.descriptor.component, match_info))
                                    .collect(),
                            },
                        ),
                    );
                }
            }

            if has_any_datatype {
                re_log::trace!(
                    "Entity {entity_path:?} in store {store_id:?} may now be visualizable by {visualizer:?} (has any required datatype)",
                );
            }
        }

        VisualizabilityConstraints::BufferAndFormat(constraint) => {
            for c in components {
                if !c.has_data {
                    continue;
                }

                let Some(arrow_datatype) = &c.inner_arrow_datatype else {
                    continue;
                };

                let buffer_match = constraint.check_buffer_match(arrow_datatype, &c.descriptor);
                let is_format_match = constraint.check_format_match(arrow_datatype, &c.descriptor);
                if buffer_match.is_none() && !is_format_match {
                    continue;
                }

                let state = store_mapping
                    .buffer_and_format_state
                    .entry(entity_path.clone())
                    .or_default();

                if let Some(buffer_match) = buffer_match {
                    state
                        .all_buffer_matches
                        .insert(c.descriptor.component, buffer_match);
                }
                if is_format_match {
                    state.all_formats_matches.insert(c.descriptor.component);
                }

                if !state.all_buffer_matches.is_empty() && !state.all_formats_matches.is_empty() {
                    let buffer_matches = state.all_buffer_matches.clone();
                    let format_components = state.all_formats_matches.clone();
                    store_mapping.add_visualizability_reason(
                        &entity_path,
                        visualizer,
                        VisualizableReason::BufferAndFormatMatch(BufferAndFormatMatch {
                            buffer_target: constraint.buffer_target(),
                            format_target: constraint.format_target(),
                            buffer_matches,
                            format_matches: format_components,
                        }),
                    );
                }
            }
        }
    }
}

impl VisualizerEntitySubscriber {
    /// Bootstrap from an existing [`re_entity_db::EntityDb`], processing all existing data
    /// so that the subscriber is up-to-date without having received incremental events.
    pub fn bootstrap(&mut self, entity_db: &re_entity_db::EntityDb) {
        re_tracing::profile_function!(self.config.visualizer);

        let store_id = entity_db.store_id().clone();
        let engine = entity_db.storage_engine();
        let store = engine.store();

        // Process manifest (virtual additions).
        if let Some(manifest) = entity_db.rrd_manifest_index().manifest() {
            for meta in re_chunk_store::ChunkMeta::from_manifest(manifest) {
                process_entity_components(&self.config, &mut self.mapping, &store_id, meta);
            }
        }

        // Process existing physical chunks.
        for chunk in store.iter_physical_chunks() {
            let meta = re_chunk_store::ChunkMeta::from_chunk(chunk);
            process_entity_components(&self.config, &mut self.mapping, &store_id, meta);
        }
    }

    /// Process store events to update the per-entity visualizability data.
    pub fn on_events(&mut self, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!(self.config.visualizer);

        // TODO(andreas): Need to react to store removals as well. As of writing doesn't exist yet.
        //                These removals also need to keep in mind that things from the rrd manifest
        //                shouldn't be removed.

        for event in events {
            match &event.diff {
                re_chunk_store::ChunkStoreDiff::Addition(add) => {
                    // This is a purely additive datastructure, and it doesn't keep track of actual chunks,
                    // just the bits of data that are of actual interest.
                    // Therefore, the meta of the delta chunk is all we need, always.

                    process_entity_components(
                        &self.config,
                        &mut self.mapping,
                        &event.store_id,
                        add.chunk_meta(),
                    );
                }
                re_chunk_store::ChunkStoreDiff::VirtualAddition(virtual_add) => {
                    for meta in virtual_add.chunk_metas() {
                        process_entity_components(
                            &self.config,
                            &mut self.mapping,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BufferAndFormatConstraint;
    use arrow::array::ArrayRef;
    use re_chunk::{Chunk, RowId};
    use re_chunk_store::{
        ChunkDirectLineageReport, ChunkStoreDiff, ChunkStoreDiffAddition, ChunkStoreEvent,
        ChunkStoreGeneration,
    };
    use re_log_types::TimePoint;
    use re_sdk_types::ComponentDescriptor;

    const BUFFER_CTYPE: &str = "test.components.Buffer";
    const FORMAT_CTYPE: &str = "test.components.Format";

    fn test_constraint() -> BufferAndFormatConstraint {
        BufferAndFormatConstraint::new_with_type(
            "TestArch:buffer".into(),
            BUFFER_CTYPE.into(),
            "TestArch:format".into(),
            FORMAT_CTYPE.into(),
            arrow::datatypes::DataType::UInt32,
        )
    }

    fn test_store_id() -> StoreId {
        StoreId::random(re_log_types::StoreKind::Recording, "test_app")
    }

    /// Create a subscriber with a [`VisualizabilityConstraints::BufferAndFormat`] constraint.
    fn test_subscriber_with_buffer_and_format_constraint() -> VisualizerEntitySubscriber {
        VisualizerEntitySubscriber {
            config: VisualizerEntityConfig {
                visualizer: "TestVisualizer".into(),
                relevant_archetype: None,
                constraints: Arc::new(VisualizabilityConstraints::BufferAndFormat(
                    test_constraint(),
                )),
                known_builtin_enum_components: Arc::new(IntSet::default()),
            },
            mapping: Default::default(),
        }
    }

    /// Build a `ComponentDescriptor` with the given component identifier and optional semantic type.
    fn descriptor(component: &str, component_type: Option<&str>) -> ComponentDescriptor {
        ComponentDescriptor {
            archetype: None,
            component: component.into(),
            component_type: component_type.map(Into::into),
        }
    }

    /// Build a minimal chunk with the given entity path and component columns.
    ///
    /// Each entry is `(descriptor, arrow_datatype)` — a single-element array of the
    /// given type is created as data so that `has_data` is `true`.
    fn make_chunk(
        entity: &EntityPath,
        columns: &[(ComponentDescriptor, arrow::datatypes::DataType)],
    ) -> Arc<Chunk> {
        let row = columns.iter().map(|(desc, dt)| {
            let array: ArrayRef = arrow::array::new_null_array(dt, 1);
            (desc.clone(), array)
        });
        Arc::new(
            Chunk::builder(entity.clone())
                .with_row(RowId::new(), TimePoint::default(), row)
                .build()
                .expect("failed to build test chunk"),
        )
    }

    /// Wrap a chunk into a single `ChunkStoreEvent` (addition).
    fn addition_event(store_id: &StoreId, chunk: Arc<Chunk>) -> ChunkStoreEvent {
        ChunkStoreEvent {
            store_id: store_id.clone(),
            store_generation: ChunkStoreGeneration::default(),
            event_id: 0,
            diff: ChunkStoreDiff::Addition(ChunkStoreDiffAddition {
                chunk_before_processing: Arc::clone(&chunk),
                chunk_after_processing: chunk,
                direct_lineage: ChunkDirectLineageReport::Volatile,
            }),
        }
    }

    /// Assert that the subscriber has marked the entity as visualizable with a `BufferAndFormatMatch`.
    ///
    /// Returns the match struct for further inspection.
    fn expect_buffer_and_format_visualizable<'a>(
        subscriber: &'a VisualizerEntitySubscriber,
        entity: &EntityPath,
    ) -> &'a BufferAndFormatMatch {
        let entities = subscriber.visualizable_entities();
        let reason = entities.get(entity).expect("entity should be visualizable");
        match reason {
            VisualizableReason::BufferAndFormatMatch(m) => m,
            other => panic!("expected BufferAndFormatMatch, got {other:?}"),
        }
    }

    fn assert_not_visualizable(subscriber: &VisualizerEntitySubscriber, entity: &EntityPath) {
        let is_visualizable = subscriber.visualizable_entities().contains_key(entity);
        assert!(
            !is_visualizable,
            "entity {entity} should NOT be visualizable yet"
        );
    }

    // ---- Tests ----

    #[test]
    fn both_buffer_and_format_in_one_event() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();
        let mut sub = test_subscriber_with_buffer_and_format_constraint();

        let chunk = make_chunk(
            &entity,
            &[
                (
                    descriptor("buf", Some(BUFFER_CTYPE)),
                    BufferAndFormatConstraint::buffer_arrow_datatype(),
                ),
                (
                    descriptor("fmt", Some(FORMAT_CTYPE)),
                    arrow::datatypes::DataType::UInt32,
                ),
            ],
        );

        sub.on_events(&[addition_event(&store_id, chunk)]);

        let m = expect_buffer_and_format_visualizable(&sub, &entity);
        assert_eq!(m.buffer_matches.len(), 1);
        assert!(matches!(
            m.buffer_matches.get(&"buf".into()),
            Some(DatatypeMatch::NativeSemantics { .. })
        ));
        assert!(m.format_matches.contains(&ComponentIdentifier::from("fmt")));
    }

    #[test]
    fn buffer_and_format_not_at_once() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();

        let buffer_chunk = make_chunk(
            &entity,
            &[(
                descriptor("buf", Some(BUFFER_CTYPE)),
                BufferAndFormatConstraint::buffer_arrow_datatype(),
            )],
        );
        let format_chunk = make_chunk(
            &entity,
            &[(
                descriptor("fmt", Some(FORMAT_CTYPE)),
                arrow::datatypes::DataType::UInt32,
            )],
        );

        for (first_chunk, second_chunk) in [
            (buffer_chunk.clone(), format_chunk.clone()),
            (format_chunk.clone(), buffer_chunk.clone()),
        ] {
            let mut sub = test_subscriber_with_buffer_and_format_constraint();

            sub.on_events(&[addition_event(&store_id, first_chunk)]);
            assert_not_visualizable(&sub, &entity);

            sub.on_events(&[addition_event(&store_id, second_chunk)]);
            expect_buffer_and_format_visualizable(&sub, &entity);
        }
    }

    #[test]
    fn buffer_physical_only_match() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();
        let mut sub = test_subscriber_with_buffer_and_format_constraint();

        // Buffer has the right arrow type but wrong semantic type → PhysicalDatatypeOnly.
        let chunk = make_chunk(
            &entity,
            &[
                (
                    descriptor("buf", Some("other.components.Blob")),
                    BufferAndFormatConstraint::buffer_arrow_datatype(),
                ),
                (
                    descriptor("fmt", Some(FORMAT_CTYPE)),
                    arrow::datatypes::DataType::UInt32,
                ),
            ],
        );
        sub.on_events(&[addition_event(&store_id, chunk)]);

        let m = expect_buffer_and_format_visualizable(&sub, &entity);
        assert!(matches!(
            m.buffer_matches.get(&"buf".into()),
            Some(DatatypeMatch::PhysicalDatatypeOnly { .. })
        ));
    }

    #[test]
    fn format_without_semantic_match_is_rejected() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();
        let mut sub = test_subscriber_with_buffer_and_format_constraint();

        // Buffer matches, but format has wrong semantic type.
        let chunk = make_chunk(
            &entity,
            &[
                (
                    descriptor("buf", Some(BUFFER_CTYPE)),
                    BufferAndFormatConstraint::buffer_arrow_datatype(),
                ),
                (
                    descriptor("fmt", Some("wrong.components.Format")),
                    arrow::datatypes::DataType::UInt32,
                ),
            ],
        );
        sub.on_events(&[addition_event(&store_id, chunk)]);
        assert_not_visualizable(&sub, &entity);
    }

    #[test]
    fn wrong_arrow_datatype_rejected() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();
        let mut sub = test_subscriber_with_buffer_and_format_constraint();

        // Neither buffer nor format arrow types match.
        let chunk = make_chunk(
            &entity,
            &[
                (
                    descriptor("buf", Some(BUFFER_CTYPE)),
                    arrow::datatypes::DataType::Float64,
                ),
                (
                    descriptor("fmt", Some(FORMAT_CTYPE)),
                    arrow::datatypes::DataType::Float64,
                ),
            ],
        );
        sub.on_events(&[addition_event(&store_id, chunk)]);
        assert_not_visualizable(&sub, &entity);
    }

    #[test]
    fn multiple_buffer_matches_across_events() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();
        let mut sub = test_subscriber_with_buffer_and_format_constraint();

        // Event 1: first buffer + format.
        let chunk1 = make_chunk(
            &entity,
            &[
                (
                    descriptor("buf1", Some(BUFFER_CTYPE)),
                    BufferAndFormatConstraint::buffer_arrow_datatype(),
                ),
                (
                    descriptor("fmt", Some(FORMAT_CTYPE)),
                    arrow::datatypes::DataType::UInt32,
                ),
            ],
        );
        sub.on_events(&[addition_event(&store_id, chunk1)]);
        expect_buffer_and_format_visualizable(&sub, &entity);

        // Event 2: second buffer arrives.
        let chunk2 = make_chunk(
            &entity,
            &[(
                descriptor("buf2", Some("other.components.Blob")),
                BufferAndFormatConstraint::buffer_arrow_datatype(),
            )],
        );
        sub.on_events(&[addition_event(&store_id, chunk2)]);

        // Both buffer matches should be visible.
        let m = expect_buffer_and_format_visualizable(&sub, &entity);
        assert_eq!(m.buffer_matches.len(), 2);
        assert!(matches!(
            m.buffer_matches.get(&"buf1".into()),
            Some(DatatypeMatch::NativeSemantics { .. })
        ));
        assert!(matches!(
            m.buffer_matches.get(&"buf2".into()),
            Some(DatatypeMatch::PhysicalDatatypeOnly { .. })
        ));
    }

    #[test]
    fn multiple_buffers_with_multiple_formats() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();
        let mut sub = test_subscriber_with_buffer_and_format_constraint();

        // Event 1: first buffer + first format.
        let chunk1 = make_chunk(
            &entity,
            &[
                (
                    descriptor("buf1", Some(BUFFER_CTYPE)),
                    BufferAndFormatConstraint::buffer_arrow_datatype(),
                ),
                (
                    descriptor("fmt1", Some(FORMAT_CTYPE)),
                    arrow::datatypes::DataType::UInt32,
                ),
            ],
        );
        sub.on_events(&[addition_event(&store_id, chunk1)]);

        let m = expect_buffer_and_format_visualizable(&sub, &entity);
        assert_eq!(m.buffer_matches.len(), 1);
        assert_eq!(m.format_matches.len(), 1);

        // Event 2: second buffer + second format.
        let chunk2 = make_chunk(
            &entity,
            &[
                (
                    descriptor("buf2", Some("other.components.Blob")),
                    BufferAndFormatConstraint::buffer_arrow_datatype(),
                ),
                (
                    descriptor("fmt2", Some(FORMAT_CTYPE)),
                    arrow::datatypes::DataType::UInt32,
                ),
            ],
        );
        sub.on_events(&[addition_event(&store_id, chunk2)]);

        // Both buffers and both formats should be tracked in a single entry.
        let m = expect_buffer_and_format_visualizable(&sub, &entity);
        assert_eq!(m.buffer_matches.len(), 2);
        assert!(matches!(
            m.buffer_matches.get(&"buf1".into()),
            Some(DatatypeMatch::NativeSemantics { .. })
        ));
        assert!(matches!(
            m.buffer_matches.get(&"buf2".into()),
            Some(DatatypeMatch::PhysicalDatatypeOnly { .. })
        ));
        assert!(
            m.format_matches
                .contains(&ComponentIdentifier::from("fmt1"))
        );
        assert!(
            m.format_matches
                .contains(&ComponentIdentifier::from("fmt2"))
        );
    }

    #[test]
    fn nested_struct_with_two_blob_fields() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();
        let mut sub = test_subscriber_with_buffer_and_format_constraint();

        // A struct component containing two fields with the buffer arrow type.
        let struct_dt = arrow::datatypes::DataType::Struct(
            vec![
                arrow::datatypes::Field::new(
                    "blob_a",
                    BufferAndFormatConstraint::buffer_arrow_datatype(),
                    true,
                ),
                arrow::datatypes::Field::new(
                    "blob_b",
                    BufferAndFormatConstraint::buffer_arrow_datatype(),
                    true,
                ),
            ]
            .into(),
        );

        let chunk = make_chunk(
            &entity,
            &[
                (descriptor("data", None), struct_dt),
                (
                    descriptor("fmt", Some(FORMAT_CTYPE)),
                    arrow::datatypes::DataType::UInt32,
                ),
            ],
        );
        sub.on_events(&[addition_event(&store_id, chunk)]);

        let m = expect_buffer_and_format_visualizable(&sub, &entity);
        let data_match = m
            .buffer_matches
            .get(&ComponentIdentifier::from("data"))
            .expect("should have buffer match for 'data'");
        match data_match {
            DatatypeMatch::PhysicalDatatypeOnly { selectors, .. } => {
                assert_eq!(
                    selectors.len(),
                    2,
                    "should have selectors for both blob_a and blob_b, got {selectors:?}"
                );
            }
            other @ DatatypeMatch::NativeSemantics { .. } => {
                panic!("expected PhysicalDatatypeOnly with selectors, got {other:?}")
            }
        }
    }
}
