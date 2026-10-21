use std::collections::hash_map::Entry;
use std::sync::Arc;

use nohash_hasher::IntSet;
#[cfg(test)]
use parking_lot::RwLock;

use re_chunk::ComponentIdentifier;
use re_chunk_store::ChunkStoreEvent;
use re_log::debug_panic;
use re_log_types::{EntityPath, StoreId};

use nohash_hasher::IntMap;

use crate::typed_entity_collections::{
    BufferAndFormatMatch, ComponentMatch, SingleRequiredComponentMatch, VisualizableReason,
};
use crate::{
    IndicatedEntities, RecommendedMappings, SharedReflection, ViewSystemIdentifier,
    VisualizabilityConstraints, VisualizableEntities, VisualizerComponentMappings,
    VisualizerComponentSource, VisualizerQueryInfo,
};

/// Configuration data needed to build a [`VisualizerEntitySubscriber`].
#[derive(Clone, re_byte_size::SizeBytes)] // Cheap to clone; uses ref-counted data internally.
pub struct VisualizerEntityConfig {
    /// Visualizer type this config is associated with.
    pub visualizer: ViewSystemIdentifier,

    #[size_bytes(ignore)]
    pub query_info: Arc<VisualizerQueryInfo>,

    /// Shared with the viewer and all visualizers, including those registered before reflection edits.
    /// Reflection must be finalized before processing store data. Existing matches are not invalidated on edits!
    #[size_bytes(ignore)]
    pub reflection: SharedReflection,
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
#[derive(re_byte_size::SizeBytes)]
#[size_bytes(profile)]
pub struct VisualizerEntitySubscriber {
    config: VisualizerEntityConfig,
    mapping: VisualizerEntityMapping,
}

/// Per-entity state for a [`VisualizabilityConstraints::BufferAndFormat`] constraint.
///
/// Buffer and format components may arrive in separate chunk store events, so we keep accumulating them here.
#[derive(Default, re_byte_size::SizeBytes)]
struct BufferAndFormatEntityState {
    all_buffer_matches: IntMap<ComponentIdentifier, ComponentMatch>,
    all_formats_matches: IntSet<ComponentIdentifier>,
}

#[derive(Default, re_byte_size::SizeBytes)]
struct VisualizerEntityMapping {
    /// Which entities the visualizer can be applied to.
    visualizable_entities: VisualizableEntities,

    /// Entities for which this visualizer should apply by default; see [`IndicatedEntities`].
    indicated_entities: IndicatedEntities,

    /// Per-entity state for [`VisualizabilityConstraints::BufferAndFormat`] constraints.
    ///
    /// Only populated when the requirement is [`VisualizabilityConstraints::BufferAndFormat`].
    buffer_and_format_state: IntMap<EntityPath, BufferAndFormatEntityState>,
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

    /// Entities for which this visualizer should apply by default.
    ///
    /// See [`IndicatedEntities`] for the indication rules.
    /// Indication does not imply that an entity is also in [`Self::visualizable_entities`].
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
    }: &re_chunk_store::ChunkMeta,
) {
    let VisualizerEntityConfig {
        query_info,
        visualizer,
        reflection,
    } = config;

    // Update indicated_entities.
    if query_info.relevant_archetype.is_none_or(|archetype| {
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
    match &query_info.constraints {
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
                .any(|c| relevant_components.contains(&c.descriptor.component));

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
            let reflection = reflection.read_recursive();
            let mut has_any_datatype = false;

            for c in components {
                if !constraint.allow_static_data() && c.is_static {
                    continue;
                }

                let Some(arrow_data_type) = &c.inner_arrow_datatype else {
                    continue;
                };

                let candidates = reflection
                    .component_identifiers
                    .get(&c.descriptor.component)
                    .map_or(&[][..], |r| r.heuristic_mappings());
                let target = constraint.target_component();
                if candidates
                    .iter()
                    .any(|mapping| mapping.descriptor.component == target)
                {
                    let mut mappings = VisualizerComponentMappings::default();
                    for mapping in candidates {
                        let component = mapping.descriptor.component;
                        if component == target
                            || query_info
                                .queried_components()
                                .any(|queried| queried == component)
                        {
                            mappings.insert(
                                component,
                                VisualizerComponentSource::SourceComponent {
                                    source_component: c.descriptor.component,
                                    selector: mapping.selector.clone(),
                                },
                            );
                        }
                    }
                    let mappings = RecommendedMappings::from_mappings(mappings);
                    store_mapping
                        .indicated_entities
                        .0
                        .insert(entity_path.clone());
                    store_mapping.add_visualizability_reason(
                        entity_path,
                        visualizer,
                        VisualizableReason::SingleRequiredComponentMatch(
                            SingleRequiredComponentMatch {
                                target_component: target,
                                matches: std::iter::once((
                                    c.descriptor.component,
                                    ComponentMatch::HeuristicMapping {
                                        component_type: c.descriptor.component_type,
                                        mappings,
                                    },
                                ))
                                .collect(),
                            },
                        ),
                    );

                    // Registered heuristic mappings take precedence over datatype matching.
                    continue;
                }

                if let Some(match_info) = constraint.check_datatype_match(
                    &reflection.components,
                    arrow_data_type,
                    c.descriptor.component_type,
                    c.descriptor.component,
                ) {
                    has_any_datatype = true;

                    store_mapping.add_visualizability_reason(
                        entity_path,
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
                let Some(arrow_data_type) = &c.inner_arrow_datatype else {
                    continue;
                };

                let buffer_match = constraint.check_buffer_match(arrow_data_type, &c.descriptor);
                let is_format_match = constraint.check_format_match(arrow_data_type, &c.descriptor);
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
                        entity_path,
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
                process_entity_components(&self.config, &mut self.mapping, &store_id, &meta);
            }
        }

        // Process existing physical chunks.
        for chunk in store.iter_physical_chunks() {
            let meta = re_chunk_store::ChunkMeta::from_chunk(chunk);
            process_entity_components(&self.config, &mut self.mapping, &store_id, &meta);
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
                re_chunk_store::ChunkStoreDiff::SchemaAddition(schema_add) => {
                    for meta in &schema_add.new_columns {
                        process_entity_components(
                            &self.config,
                            &mut self.mapping,
                            &event.store_id,
                            meta,
                        );
                    }
                }
                re_chunk_store::ChunkStoreDiff::Addition(..)
                | re_chunk_store::ChunkStoreDiff::Deletion(_)
                | re_chunk_store::ChunkStoreDiff::VirtualAddition(..) => {
                    // Our heuristics are additive and only change when new data is added to the store.
                    // For now we don't look at the component data to inform the recommendations for
                    // visualizers and view spawning, so looking at additions to the schema is enough.
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BufferAndFormatConstraint;
    use re_chunk_store::{
        ChunkStoreDiff, ChunkStoreDiffSchemaAddition, ChunkStoreEvent, ChunkStoreGeneration,
    };
    use re_log_types::{example_components::MyPoint, example_components::MyPoints};
    use re_sdk_types::{ComponentDescriptor, ComponentType};
    use re_types_core::ArrowDataType as _;
    use std::assert_matches;

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

    #[test]
    fn reflected_heuristic_mapping_matches_required_input() {
        use arrow::datatypes::DataType;
        use re_sdk_types::{archetypes::TextDocument, components::Text};
        use re_types_core::reflection::HeuristicMapping;

        let source = descriptor("custom:message", Some("custom.Message"));
        let target = TextDocument::descriptor_text();
        let mut reflection = re_sdk_types::reflection::reflection().clone();
        reflection
            .component_identifiers
            .entry(source.component)
            .or_insert_with(|| source.clone().into())
            .add_heuristic_mapping(HeuristicMapping {
                descriptor: target.clone(),
                selector: ".data".to_owned(),
            })
            .expect("a new target should be accepted");
        let reflection = Arc::new(RwLock::new(reflection));
        let entity = EntityPath::from("raw");
        let event = schema_addition_event(
            &test_store_id(),
            &entity,
            &[(source.clone(), DataType::UInt32)],
        );
        for (visualizer, required, expected) in [
            ("TextDocument", target.clone(), true),
            (
                "TextLog",
                re_sdk_types::archetypes::TextLog::descriptor_text(),
                false,
            ),
        ] {
            let mut subscriber = VisualizerEntityConfig {
                visualizer: visualizer.into(),
                query_info: Arc::new(VisualizerQueryInfo {
                    relevant_archetype: target.archetype,
                    ..VisualizerQueryInfo::single_required_component::<Text>(
                        &required,
                        std::slice::from_ref(&required),
                    )
                }),
                reflection: reflection.clone(),
            }
            .create_subscriber();
            subscriber.on_events(std::slice::from_ref(&event));
            subscriber.on_events(std::slice::from_ref(&event));
            assert_eq!(
                subscriber.visualizable_entities().contains_key(&entity),
                expected,
                "{visualizer}"
            );
            assert_eq!(
                subscriber.indicated_entities().contains(&entity),
                expected,
                "{visualizer}"
            );
            if expected {
                let entities = subscriber.visualizable_entities();
                let VisualizableReason::SingleRequiredComponentMatch(m) = &entities[&entity] else {
                    panic!("Expected a single required component match");
                };
                assert_eq!(m.matches.len(), 1);
                assert_matches!(
                    m.matches.get(&source.component),
                    Some(ComponentMatch::HeuristicMapping { .. })
                );
                assert_eq!(
                    m.matches[&source.component].component_type(),
                    source.component_type.as_ref()
                );
                let mappings: Vec<_> = entities[&entity].heuristic_mappings().collect();
                assert_eq!(mappings.len(), 1);
                assert_eq!(
                    mappings[0].mappings()[&target.component],
                    crate::VisualizerComponentSource::SourceComponent {
                        source_component: source.component,
                        selector: ".data".to_owned(),
                    }
                );
            }
        }
    }

    #[test]
    fn flat_mappings_assemble_only_queried_companions() {
        use re_sdk_types::archetypes::{CoordinateFrame, Measurements};
        use re_types_core::reflection::HeuristicMapping;

        let source = ComponentDescriptor::partial("custom:measurements");
        let values = Measurements::descriptor_values();
        let variances = Measurements::descriptor_variances();
        let units = Measurements::descriptor_units();
        let frame = CoordinateFrame::descriptor_frame();
        let mut reflection = re_sdk_types::reflection::reflection().clone();
        let mut entry: re_types_core::reflection::ComponentIdentifierReflection =
            source.clone().into();
        for (descriptor, selector) in [
            (values.clone(), ".value"),
            (variances.clone(), ".variance"),
            (units.clone(), ".unit"),
            (frame.clone(), ".frame"),
        ] {
            entry
                .add_heuristic_mapping(HeuristicMapping {
                    descriptor,
                    selector: selector.to_owned(),
                })
                .expect("each target should be unique");
        }
        reflection
            .component_identifiers
            .insert(source.component, entry);
        let config = VisualizerEntityConfig {
            visualizer: "Measurements".into(),
            query_info: Arc::new(VisualizerQueryInfo::single_required_component::<
                re_sdk_types::components::Scalar,
            >(
                &values,
                &[values.clone(), variances.clone(), units.clone()],
            )),
            reflection: Arc::new(RwLock::new(reflection)),
        };
        let entity = EntityPath::from("raw");
        let mut subscriber = config.create_subscriber();
        subscriber.on_events(&[schema_addition_event(
            &test_store_id(),
            &entity,
            &[(source.clone(), arrow::datatypes::DataType::UInt32)],
        )]);
        let entities = subscriber.visualizable_entities();
        let recommendations: Vec<_> = entities[&entity].heuristic_mappings().collect();
        assert_eq!(recommendations.len(), 1);
        assert_eq!(
            *recommendations[0].mappings(),
            [
                (values, ".value"),
                (variances, ".variance"),
                (units, ".unit")
            ]
            .into_iter()
            .map(|(target, selector)| (
                target.component,
                VisualizerComponentSource::SourceComponent {
                    source_component: source.component,
                    selector: selector.to_owned(),
                },
            ))
            .collect()
        );
    }

    fn test_store_id() -> StoreId {
        StoreId::random(re_log_types::StoreKind::Recording, "test_app")
    }

    /// Create a subscriber with a [`VisualizabilityConstraints::BufferAndFormat`] constraint.
    fn test_subscriber_with_buffer_and_format_constraint() -> VisualizerEntitySubscriber {
        VisualizerEntitySubscriber {
            config: VisualizerEntityConfig {
                visualizer: "TestVisualizer".into(),
                query_info: Arc::new(VisualizerQueryInfo {
                    constraints: VisualizabilityConstraints::BufferAndFormat(test_constraint()),
                    ..VisualizerQueryInfo::empty()
                }),
                reflection: Default::default(),
            },
            mapping: Default::default(),
        }
    }

    fn test_single_required_component_subscriber() -> VisualizerEntitySubscriber {
        VisualizerEntitySubscriber {
            config: VisualizerEntityConfig {
                visualizer: "TestVisualizer".into(),
                query_info: Arc::new(VisualizerQueryInfo::single_required_component::<MyPoint>(
                    &MyPoints::descriptor_points(),
                    &[MyPoints::descriptor_points()],
                )),
                reflection: Default::default(),
            },
            mapping: Default::default(),
        }
    }

    /// Build a `ComponentDescriptor` with the given component identifier and optional semantic type.
    fn descriptor(
        component: impl Into<ComponentIdentifier>,
        component_type: Option<&str>,
    ) -> ComponentDescriptor {
        ComponentDescriptor {
            archetype: None,
            component: component.into(),
            component_type: component_type.and_then(|s| ComponentType::try_new(s).ok()),
        }
    }

    /// Build a `ChunkStoreEvent` for a schema addition with only the specified columns.
    fn schema_addition_event(
        store_id: &StoreId,
        entity: &EntityPath,
        columns: &[(ComponentDescriptor, arrow::datatypes::DataType)],
    ) -> ChunkStoreEvent {
        let meta = re_chunk_store::ChunkMeta {
            entity_path: entity.clone(),
            components: columns
                .iter()
                .map(|(desc, dt)| re_chunk_store::ChunkComponentMeta {
                    descriptor: desc.clone(),
                    inner_arrow_datatype: Some(dt.clone()),
                    has_data: true,
                    is_static: false,
                })
                .collect(),
        };
        ChunkStoreEvent {
            store_id: store_id.clone(),
            store_generation: ChunkStoreGeneration::default(),
            event_id: 0,
            diff: ChunkStoreDiff::SchemaAddition(ChunkStoreDiffSchemaAddition {
                new_columns: vec![meta],
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

    fn assert_visualizable(subscriber: &VisualizerEntitySubscriber, entity: &EntityPath) {
        let is_visualizable = subscriber.visualizable_entities().contains_key(entity);
        assert!(is_visualizable, "entity {entity} should be visualizable");
    }

    // ---- Tests ----

    // Regression test for https://github.com/rerun-io/rerun/issues/12736
    #[test]
    fn empty_required_component_is_still_visualizable() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();
        let mut sub = test_single_required_component_subscriber();

        sub.on_events(&[ChunkStoreEvent {
            store_id: store_id.clone(),
            store_generation: ChunkStoreGeneration::default(),
            event_id: 0,
            diff: ChunkStoreDiff::SchemaAddition(ChunkStoreDiffSchemaAddition {
                new_columns: vec![re_chunk_store::ChunkMeta {
                    entity_path: entity.clone(),
                    components: vec![re_chunk_store::ChunkComponentMeta {
                        descriptor: MyPoints::descriptor_points(),
                        inner_arrow_datatype: Some(MyPoint::arrow_data_type()),
                        has_data: false, // This would happen if someone logs an entity without any rows!
                        is_static: false,
                    }],
                }],
            }),
        }]);

        assert!(sub.indicated_entities().contains(&entity));
        assert_visualizable(&sub, &entity);
    }

    #[test]
    fn both_buffer_and_format_in_one_event() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();
        let mut sub = test_subscriber_with_buffer_and_format_constraint();

        let columns = [
            (
                descriptor("buf", Some(BUFFER_CTYPE)),
                BufferAndFormatConstraint::buffer_arrow_datatype(),
            ),
            (
                descriptor("fmt", Some(FORMAT_CTYPE)),
                arrow::datatypes::DataType::UInt32,
            ),
        ];

        sub.on_events(&[schema_addition_event(&store_id, &entity, &columns)]);

        let m = expect_buffer_and_format_visualizable(&sub, &entity);
        assert_eq!(m.buffer_matches.len(), 1);
        assert_matches!(
            m.buffer_matches.get(&"buf".into()),
            Some(ComponentMatch::NativeSemantics { .. })
        );
        assert!(m.format_matches.contains(&ComponentIdentifier::from("fmt")));
    }

    #[test]
    fn buffer_and_format_not_at_once() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();

        let buffer_columns = [(
            descriptor("buf", Some(BUFFER_CTYPE)),
            BufferAndFormatConstraint::buffer_arrow_datatype(),
        )];
        let format_columns = [(
            descriptor("fmt", Some(FORMAT_CTYPE)),
            arrow::datatypes::DataType::UInt32,
        )];

        for (first, second) in [
            (&buffer_columns[..], &format_columns[..]),
            (&format_columns[..], &buffer_columns[..]),
        ] {
            let mut sub = test_subscriber_with_buffer_and_format_constraint();

            sub.on_events(&[schema_addition_event(&store_id, &entity, first)]);
            assert_not_visualizable(&sub, &entity);

            sub.on_events(&[schema_addition_event(&store_id, &entity, second)]);
            expect_buffer_and_format_visualizable(&sub, &entity);
        }
    }

    #[test]
    fn buffer_physical_only_match() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();
        let mut sub = test_subscriber_with_buffer_and_format_constraint();

        // Buffer has the right arrow type but wrong semantic type → PhysicalDatatypeOnly.
        let columns = [
            (
                descriptor("buf", Some("other.components.Blob")),
                BufferAndFormatConstraint::buffer_arrow_datatype(),
            ),
            (
                descriptor("fmt", Some(FORMAT_CTYPE)),
                arrow::datatypes::DataType::UInt32,
            ),
        ];
        sub.on_events(&[schema_addition_event(&store_id, &entity, &columns)]);

        let m = expect_buffer_and_format_visualizable(&sub, &entity);
        assert_matches!(
            m.buffer_matches.get(&"buf".into()),
            Some(ComponentMatch::PhysicalDatatypeOnly { .. })
        );
    }

    #[test]
    fn format_without_semantic_match_is_rejected() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();
        let mut sub = test_subscriber_with_buffer_and_format_constraint();

        // Buffer matches, but format has wrong semantic type.
        let columns = [
            (
                descriptor("buf", Some(BUFFER_CTYPE)),
                BufferAndFormatConstraint::buffer_arrow_datatype(),
            ),
            (
                descriptor("fmt", Some("wrong.components.Format")),
                arrow::datatypes::DataType::UInt32,
            ),
        ];
        sub.on_events(&[schema_addition_event(&store_id, &entity, &columns)]);
        assert_not_visualizable(&sub, &entity);
    }

    #[test]
    fn wrong_arrow_datatype_rejected() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();
        let mut sub = test_subscriber_with_buffer_and_format_constraint();

        // Neither buffer nor format arrow types match.
        let columns = [
            (
                descriptor("buf", Some(BUFFER_CTYPE)),
                arrow::datatypes::DataType::Float64,
            ),
            (
                descriptor("fmt", Some(FORMAT_CTYPE)),
                arrow::datatypes::DataType::Float64,
            ),
        ];
        sub.on_events(&[schema_addition_event(&store_id, &entity, &columns)]);
        assert_not_visualizable(&sub, &entity);
    }

    #[test]
    fn multiple_buffer_matches_across_events() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();
        let mut sub = test_subscriber_with_buffer_and_format_constraint();

        // Event 1: first buffer + format.
        let columns1 = [
            (
                descriptor("buf1", Some(BUFFER_CTYPE)),
                BufferAndFormatConstraint::buffer_arrow_datatype(),
            ),
            (
                descriptor("fmt", Some(FORMAT_CTYPE)),
                arrow::datatypes::DataType::UInt32,
            ),
        ];
        sub.on_events(&[schema_addition_event(&store_id, &entity, &columns1)]);
        expect_buffer_and_format_visualizable(&sub, &entity);

        // Event 2: second buffer arrives.
        let columns2 = [(
            descriptor("buf2", Some("other.components.Blob")),
            BufferAndFormatConstraint::buffer_arrow_datatype(),
        )];
        sub.on_events(&[schema_addition_event(&store_id, &entity, &columns2)]);

        // Both buffer matches should be visible.
        let m = expect_buffer_and_format_visualizable(&sub, &entity);
        assert_eq!(m.buffer_matches.len(), 2);
        assert_matches!(
            m.buffer_matches.get(&"buf1".into()),
            Some(ComponentMatch::NativeSemantics { .. })
        );
        assert_matches!(
            m.buffer_matches.get(&"buf2".into()),
            Some(ComponentMatch::PhysicalDatatypeOnly { .. })
        );
    }

    #[test]
    fn multiple_buffers_with_multiple_formats() {
        let store_id = test_store_id();
        let entity: EntityPath = "/test/entity".into();
        let mut sub = test_subscriber_with_buffer_and_format_constraint();

        // Event 1: first buffer + first format.
        let columns1 = [
            (
                descriptor("buf1", Some(BUFFER_CTYPE)),
                BufferAndFormatConstraint::buffer_arrow_datatype(),
            ),
            (
                descriptor("fmt1", Some(FORMAT_CTYPE)),
                arrow::datatypes::DataType::UInt32,
            ),
        ];
        sub.on_events(&[schema_addition_event(&store_id, &entity, &columns1)]);

        let m = expect_buffer_and_format_visualizable(&sub, &entity);
        assert_eq!(m.buffer_matches.len(), 1);
        assert_eq!(m.format_matches.len(), 1);

        // Event 2: second buffer + second format.
        let columns2 = [
            (
                descriptor("buf2", Some("other.components.Blob")),
                BufferAndFormatConstraint::buffer_arrow_datatype(),
            ),
            (
                descriptor("fmt2", Some(FORMAT_CTYPE)),
                arrow::datatypes::DataType::UInt32,
            ),
        ];
        sub.on_events(&[schema_addition_event(&store_id, &entity, &columns2)]);

        // Both buffers and both formats should be tracked in a single entry.
        let m = expect_buffer_and_format_visualizable(&sub, &entity);
        assert_eq!(m.buffer_matches.len(), 2);
        assert_matches!(
            m.buffer_matches.get(&"buf1".into()),
            Some(ComponentMatch::NativeSemantics { .. })
        );
        assert_matches!(
            m.buffer_matches.get(&"buf2".into()),
            Some(ComponentMatch::PhysicalDatatypeOnly { .. })
        );
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

        let columns = [
            (descriptor("data", None), struct_dt),
            (
                descriptor("fmt", Some(FORMAT_CTYPE)),
                arrow::datatypes::DataType::UInt32,
            ),
        ];
        sub.on_events(&[schema_addition_event(&store_id, &entity, &columns)]);

        let m = expect_buffer_and_format_visualizable(&sub, &entity);
        let data_match = m
            .buffer_matches
            .get(&ComponentIdentifier::from("data"))
            .expect("should have buffer match for 'data'");
        match data_match {
            ComponentMatch::PhysicalDatatypeOnly {
                field_accessors, ..
            } => {
                assert_eq!(
                    field_accessors.len(),
                    2,
                    "should have field accessors for both blob_a and blob_b, got {field_accessors:?}"
                );
            }
            other @ (ComponentMatch::NativeSemantics { .. }
            | ComponentMatch::HeuristicMapping { .. }) => {
                panic!("expected PhysicalDatatypeOnly with selectors, got {other:?}")
            }
        }
    }
}
