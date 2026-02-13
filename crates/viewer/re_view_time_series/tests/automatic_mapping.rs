//! Test automatic component mapping and casting with different scenarios.
//!
//! This test verifies that the time series view correctly picks components
//! for visualization and installs mappings when needed.

use std::sync::Arc;

use re_log_types::external::arrow::array::{
    Array, Float64Array, Int16Array, Int32Array, StructArray, UInt32Array,
};
use re_log_types::external::arrow::datatypes::{DataType, Field};
use re_log_types::{EntityPath, TimePoint, Timeline};
use re_sdk_types::components;
use re_sdk_types::{DynamicArchetype, archetypes};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_automatic_component_mapping() {
    let mut test_context = TestContext::new();
    test_context.register_view_class::<TimeSeriesView>();

    setup_store(&mut test_context);
    let view_id = setup_blueprint(&mut test_context);

    // Now check the visualizer instructions for each entity
    check_visualizer_instructions(&test_context, view_id);
}

fn setup_store(test_context: &mut TestContext) {
    let timeline = Timeline::log_tick();

    // Scenario 1: Entity with only builtin Scalar component
    // Expected: Should pick Scalar component
    for i in 0..10 {
        test_context.log_entity("entity_builtin_only", |builder| {
            builder
                .with_archetype_auto_row([(timeline, i)], &archetypes::Scalars::single(i as f64))
                .with_archetype_auto_row(
                    [(timeline, i)],
                    &archetypes::SeriesLines::update_fields().with_widths([10.0, 20.0]),
                )
        });
    }

    // Scenario 2: Entity with builtin Scalar and custom Float64 component
    // Expected: Only Scalar is recommended (LinearSpeed is not recommended)
    for i in 0..10 {
        test_context.log_entity("entity_builtin_and_custom_same_type", |builder| {
            builder
                .with_archetype_auto_row(
                    [(timeline, i)],
                    &archetypes::Scalars::single(i as f64 * 2.0),
                )
                .with_archetype_auto_row(
                    [(timeline, i)],
                    &DynamicArchetype::new("custom")
                        .with_component::<components::LinearSpeed>("custom_f64", [i as f64 * 3.0]),
                )
        });
    }

    // Scenario 3: Entity with only custom Float64 component (temporal)
    // Expected: No visualizers (LinearSpeed is not recommended)
    for i in 0..10 {
        test_context.log_entity("entity_custom_only_temporal", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("custom")
                    .with_component::<components::LinearSpeed>("custom_f64_only", [i as f64 * 4.0]),
            )
        });
    }

    // Scenario 4: Entity with multiple known Rerun component types (LinearSpeed) on custom archetype
    // Expected: No visualizers (LinearSpeed is not recommended)
    for i in 0..10 {
        test_context.log_entity("entity_multiple_rerun_types_temporal", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("custom")
                    .with_component::<components::LinearSpeed>("zebra_component", [i as f64 * 5.0])
                    .with_component::<components::LinearSpeed>("alpha_component", [i as f64 * 6.0])
                    .with_component::<components::LinearSpeed>("beta_component", [i as f64 * 7.0]),
            )
        });
    }

    // Scenario 5: Entity with both static and temporal known Rerun component type (LinearSpeed)
    // Expected: No visualizers (LinearSpeed is not recommended)
    test_context.log_entity("entity_rerun_type_static_and_temporal", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &DynamicArchetype::new("custom")
                .with_component::<components::LinearSpeed>("static_linear_speed", [100.0]),
        )
    });
    for i in 0..10 {
        test_context.log_entity("entity_rerun_type_static_and_temporal", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("custom").with_component::<components::LinearSpeed>(
                    "temporal_linear_speed",
                    [i as f64 * 8.0],
                ),
            )
        });
    }

    // Scenario 6: Entity with only static known Rerun component type (LinearSpeed)
    // Expected: No visualizers (LinearSpeed is not recommended, and it's also static)
    test_context.log_entity("entity_rerun_type_static_only", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &DynamicArchetype::new("custom")
                .with_component::<components::LinearSpeed>("static_only", [999.0]),
        )
    });

    // Scenario 7: Entity with multiple fully custom components (no known Rerun types)
    // Expected: Should only recommend Float64 (Int types are never recommended)
    for i in 0..10 {
        test_context.log_entity("entity_fully_custom_mixed_types", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("custom")
                    .with_component_from_data(
                        "beta_component",
                        Arc::new(Float64Array::from(vec![i as f64 * 5.0])),
                    )
                    .with_component_from_data(
                        "alpha_component",
                        Arc::new(Int16Array::from(vec![i as i16 * 8])),
                    )
                    .with_component_from_data(
                        "zebra_component",
                        Arc::new(Int32Array::from(vec![i as i32 * 7])),
                    ),
            )
        });
    }

    // Scenario 8: Entity with fully custom Float64 vs known Rerun component type (LinearSpeed)
    // Expected: Only fully custom Float64 is recommended (LinearSpeed is not recommended)
    for i in 0..10 {
        test_context.log_entity("entity_fully_custom_vs_rerun_type", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("custom")
                    // LinearSpeed component - not recommended for time series
                    .with_component::<components::LinearSpeed>("linear_speed", [i as f64 * 20.0])
                    // Fully custom Float64 - recommended
                    .with_component_from_data(
                        "zebra_custom",
                        Arc::new(Float64Array::from(vec![i as f64 * 10.0])),
                    ),
            )
        });
    }

    // Scenario 9: Entity with fully custom Float64 vs Scalars component (NativeSemantics match)
    // Expected: Should prefer Scalars (NativeSemantics match) over fully custom Float64 (PhysicalDatatypeOnly)
    // Note: Fully custom component is named "aaa_custom" (alphabetically before "scalars")
    //       to ensure preference is based on semantic match (NativeSemantics > PhysicalDatatypeOnly),
    //       not alphabetical ordering
    for i in 0..10 {
        test_context.log_entity("entity_fully_custom_vs_scalars", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("custom")
                    // Fully custom Float64 - no ComponentType metadata (alphabetically first)
                    .with_component_from_data(
                        "aaa_custom",
                        Arc::new(Float64Array::from(vec![i as f64 * 10.0])),
                    )
                    // Known Rerun Scalars component (NativeSemantics - the exact type we're expecting!)
                    .with_component::<components::Scalar>("scalars", [i as f64 * 30.0]),
            )
        });
    }

    // Scenario 10: Entity with nested struct containing Float64 and Int32 fields
    // Expected: Should be visualizable via nested field access
    for i in 0..10 {
        use re_log_types::external::arrow;

        let struct_array = StructArray::from(vec![
            (
                Arc::new(Field::new("x", DataType::Float64, false)),
                Arc::new(Float64Array::from(vec![i as f64 * 11.0])) as Arc<dyn arrow::array::Array>,
            ),
            (
                Arc::new(Field::new("y", DataType::Int32, false)),
                Arc::new(Int32Array::from(vec![i as i32 * 12])) as Arc<dyn arrow::array::Array>,
            ),
        ]);

        test_context.log_entity("entity_nested_struct", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("custom")
                    .with_component_from_data("nested_data", Arc::new(struct_array)),
            )
        });
    }

    // Scenario 11: Complex nested structure with mixed types
    // Structure: { a: { b: Int32, c: Float64 }, x: Float32 }
    // Expected: Should pick .a.c (Float64) over .x (Float32) due to datatype priority
    for i in 0..10 {
        use re_log_types::external::arrow;

        let inner_struct = StructArray::from(vec![
            (
                Arc::new(Field::new("b", DataType::Int32, false)),
                Arc::new(Int32Array::from(vec![i as i32 * 13])) as Arc<dyn arrow::array::Array>,
            ),
            (
                Arc::new(Field::new("c", DataType::Float64, false)),
                Arc::new(Float64Array::from(vec![i as f64 * 14.0])) as Arc<dyn arrow::array::Array>,
            ),
        ]);

        let outer_struct = StructArray::from(vec![
            (
                Arc::new(Field::new(
                    "a",
                    DataType::Struct(inner_struct.fields().clone()),
                    false,
                )),
                Arc::new(inner_struct) as Arc<dyn arrow::array::Array>,
            ),
            (
                Arc::new(Field::new("x", DataType::Float32, false)),
                Arc::new(re_log_types::external::arrow::array::Float32Array::from(
                    vec![i as f32 * 15.0],
                )) as Arc<dyn arrow::array::Array>,
            ),
        ]);

        test_context.log_entity("entity_nested_datatype_priority", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("custom")
                    .with_component_from_data("complex_data", Arc::new(outer_struct)),
            )
        });
    }

    // Scenario 12: All Float64 fields but different path lengths
    // Structure: { z: Float64, a: { b: Float64 } }
    // Expected: Should pick .z (shorter) over .a.b (longer), even though "z" comes after "a.b" alphabetically
    for i in 0..10 {
        use re_log_types::external::arrow;

        let inner_struct = StructArray::from(vec![(
            Arc::new(Field::new("b", DataType::Float64, false)),
            Arc::new(Float64Array::from(vec![i as f64 * 16.0])) as Arc<dyn arrow::array::Array>,
        )]);

        let outer_struct = StructArray::from(vec![
            (
                Arc::new(Field::new(
                    "a",
                    DataType::Struct(inner_struct.fields().clone()),
                    false,
                )),
                Arc::new(inner_struct) as Arc<dyn arrow::array::Array>,
            ),
            (
                Arc::new(Field::new("z", DataType::Float64, false)),
                Arc::new(Float64Array::from(vec![i as f64 * 17.0])) as Arc<dyn arrow::array::Array>,
            ),
        ]);

        test_context.log_entity("entity_nested_path_length", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("custom")
                    .with_component_from_data("path_data", Arc::new(outer_struct)),
            )
        });
    }

    // Scenario 13: Entity with list of structs with mixed field types
    // Structure: { a: [{ b: Uint32, c: Float64 }] }
    // Expected: Should extract nested Float64 field using [] operator (.a[].c)
    for i in 0..10 {
        use re_log_types::external::arrow::array::ListArray;
        use re_log_types::external::arrow::buffer::OffsetBuffer;

        // Create struct array with b (Uint32) and c (Float64) fields
        let inner_struct = StructArray::from(vec![
            (
                Arc::new(Field::new("b", DataType::UInt32, false)),
                Arc::new(UInt32Array::from(vec![i as u32 * 18, i as u32 * 19])) as Arc<dyn Array>,
            ),
            (
                Arc::new(Field::new("c", DataType::Float64, false)),
                Arc::new(Float64Array::from(vec![i as f64 * 20.0, i as f64 * 21.0]))
                    as Arc<dyn Array>,
            ),
        ]);

        // Wrap in a list array (each row contains a list of structs)
        let list_field = Arc::new(Field::new_list_field(
            DataType::Struct(inner_struct.fields().clone()),
            false,
        ));
        let offsets = OffsetBuffer::new(vec![0, 2].into());
        let struct_list = ListArray::new(list_field, offsets, Arc::new(inner_struct), None);

        // Create outer struct containing the list
        let outer_struct = StructArray::from(vec![(
            Arc::new(Field::new("a", struct_list.data_type().clone(), false)),
            Arc::new(struct_list) as Arc<dyn Array>,
        )]);

        test_context.log_entity("entity_list_of_structs", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("custom")
                    .with_component_from_data("data", Arc::new(outer_struct)),
            )
        });
    }

    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetActiveTimeline(*timeline.name())],
    );
}

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        // Only set up the view itself, no visualizer configuration
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            TimeSeriesView::identifier(),
        ))
    })
}

/// Helper to get all visualizer instructions for an entity.
fn visualizers_for<'a>(
    data_result_tree: &'a re_viewer_context::DataResultTree,
    entity_path: &str,
) -> &'a [re_viewer_context::VisualizerInstruction] {
    let result = data_result_tree
        .lookup_result_by_path(EntityPath::from(entity_path).hash())
        .unwrap_or_else(|| panic!("{entity_path} should be in query results"));
    &result.visualizer_instructions
}

/// Helper to extract the scalar component mapping from a visualizer instruction.
fn scalar_mapping_for(
    instruction: &re_viewer_context::VisualizerInstruction,
) -> &re_viewer_context::VisualizerComponentSource {
    instruction
        .component_mappings
        .get(&archetypes::Scalars::descriptor_scalars().component)
        .expect("Should have mapping for Scalar component")
}

/// Helper to extract source component name and selector from a visualizer instruction.
/// Panics if the mapping is not a `SourceComponent`.
/// Returns (`component_name`, `selector_str`).
fn source_component_for(instruction: &re_viewer_context::VisualizerInstruction) -> (&str, &str) {
    match scalar_mapping_for(instruction) {
        re_viewer_context::VisualizerComponentSource::SourceComponent {
            source_component,
            selector,
        } => (source_component.as_str(), selector.as_str()),
        other => panic!("Expected SourceComponent mapping, got {other:?}"),
    }
}

fn check_visualizer_instructions(test_context: &TestContext, view_id: ViewId) {
    let query_result = test_context
        .query_results
        .get(&view_id)
        .expect("View should have query results");
    let data_result_tree = &query_result.tree;
    let scalar_component = archetypes::Scalars::descriptor_scalars().component;

    // Scenario 1: Entity with only builtin Scalar component
    // Expected: Should pick Scalar component (identity mapping)
    {
        let instructions = visualizers_for(data_result_tree, "entity_builtin_only");
        assert_eq!(instructions.len(), 1);
        let mapping = scalar_mapping_for(&instructions[0]);

        assert!(
            mapping.is_identity_mapping(scalar_component),
            "Expected identity mapping for builtin Scalar"
        );
    }

    // Scenario 2: Entity with builtin Scalar and custom component
    // Expected: Only Scalar gets an instruction (LinearSpeed is not recommended)
    {
        let instructions = visualizers_for(data_result_tree, "entity_builtin_and_custom_same_type");
        assert_eq!(instructions.len(), 1);

        let mapping = scalar_mapping_for(&instructions[0]);
        assert!(
            mapping.is_identity_mapping(scalar_component),
            "Expected identity mapping for builtin Scalar; LinearSpeed is not recommended"
        );
    }

    // Scenario 3: Entity with only custom Float64 component (temporal)
    // Expected: Should not visualize (LinearSpeed is not recommended)
    {
        let instructions = visualizers_for(data_result_tree, "entity_custom_only_temporal");
        assert!(
            instructions.is_empty(),
            "entity_custom_only_temporal should have no visualizer instructions since LinearSpeed is not recommended, but got: {instructions:?}",
        );
    }

    // Scenario 4: Entity with multiple known Rerun component types (LinearSpeed)
    // Expected: Should not visualize (LinearSpeed is not recommended)
    {
        let instructions =
            visualizers_for(data_result_tree, "entity_multiple_rerun_types_temporal");
        assert!(
            instructions.is_empty(),
            "entity_multiple_rerun_types_temporal should have no visualizer instructions since LinearSpeed is not recommended, but got: {instructions:?}",
        );
    }

    // Scenario 5: Entity with static and temporal known Rerun component type (LinearSpeed)
    // Expected: Should not visualize (LinearSpeed is not recommended)
    {
        let instructions =
            visualizers_for(data_result_tree, "entity_rerun_type_static_and_temporal");
        assert!(
            instructions.is_empty(),
            "entity_rerun_type_static_and_temporal should have no visualizer instructions since LinearSpeed is not recommended, but got: {instructions:?}",
        );
    }

    // Scenario 6: Entity with only static known Rerun component type (LinearSpeed)
    // Expected: Should not visualize (LinearSpeed is not recommended, and it's also static)
    {
        // We don't emit data result elements if there's no visualizer instructions in the first place,
        // so the lookup should come back empty.
        let result = data_result_tree
            .lookup_result_by_path(EntityPath::from("entity_rerun_type_static_only").hash());

        assert!(
            result.is_none(),
            "entity_rerun_type_static_only should not have any data result since LinearSpeed is not recommended, but got: {result:?}",
        );
    }

    // Scenario 7: Entity with multiple fully custom components (Float64 and Int types)
    // Expected: Should only recommend Float64 (Int types are never recommended)
    {
        let instructions = visualizers_for(data_result_tree, "entity_fully_custom_mixed_types");
        assert_eq!(instructions.len(), 1);

        let (component, selector) = source_component_for(&instructions[0]);
        assert_eq!(
            component, "custom:beta_component",
            "Should recommend Float64 component (beta_component); Int types are never recommended: {component}"
        );
        assert!(
            selector.is_empty(),
            "Expected empty selector for direct component mapping"
        );
    }

    // Scenario 8: Entity with fully custom Float64 vs known Rerun type (LinearSpeed)
    // Expected: Only fully custom Float64 gets an instruction (LinearSpeed is not recommended)
    {
        let instructions = visualizers_for(data_result_tree, "entity_fully_custom_vs_rerun_type");
        assert_eq!(instructions.len(), 1);

        let (component, selector) = source_component_for(&instructions[0]);
        assert_eq!(
            component, "custom:zebra_custom",
            "Should recommend fully custom Float64 (zebra_custom); LinearSpeed is not recommended: {component}"
        );
        assert!(
            selector.is_empty(),
            "Expected empty selector for direct component mapping"
        );
    }

    // Scenario 9: Entity with fully custom Float64 vs Scalars (NativeSemantics match)
    // Expected: Both get their own instruction (2 total), Scalars first (NativeSemantics > PhysicalDatatypeOnly)
    {
        let instructions = visualizers_for(data_result_tree, "entity_fully_custom_vs_scalars");
        assert_eq!(instructions.len(), 2);

        // Should pick Scalars component (NativeSemantics match) over fully custom Float64
        // (PhysicalDatatypeOnly), even though fully custom is alphabetically first.
        let (component, selector) = source_component_for(&instructions[0]);
        assert_eq!(
            component, "custom:scalars",
            "Should pick Scalars component (NativeSemantics match) first: {component}"
        );
        assert!(
            selector.is_empty(),
            "Expected empty selector for direct component mapping"
        );

        let (component, selector) = source_component_for(&instructions[1]);
        assert_eq!(component, "custom:aaa_custom");
        assert!(
            selector.is_empty(),
            "Expected empty selector for direct component mapping"
        );
    }

    // Scenario 10: Entity with nested struct containing Float64 and Int32 fields
    // Expected: Should only recommend Float64 field (.x) (Int32 is never recommended)
    {
        let instructions = visualizers_for(data_result_tree, "entity_nested_struct");
        assert_eq!(instructions.len(), 1);

        let (component, selector) = source_component_for(&instructions[0]);
        assert_eq!(
            component, "custom:nested_data",
            "Should map to nested struct component"
        );
        assert_eq!(
            selector, ".x",
            "Expected selector `.x` (Float64 field); Int32 field `.y` is never recommended"
        );
    }

    // Scenario 11: Complex nested structure with mixed types
    // Structure: { a: { b: Int32, c: Float64 }, x: Float32 }
    // Expected: Both float fields get their own instruction (.a.c Float64 first, .x Float32 second)
    {
        let instructions = visualizers_for(data_result_tree, "entity_nested_datatype_priority");
        assert_eq!(instructions.len(), 2);

        let (component, selector) = source_component_for(&instructions[0]);
        assert_eq!(
            component, "custom:complex_data",
            "Should map to nested struct component"
        );
        assert_eq!(
            selector, ".a.c",
            "Expected selector `.a.c` (Float64 is ordered before Float32); Int32 field `.b` is never recommended"
        );

        let (component, selector) = source_component_for(&instructions[1]);
        assert_eq!(
            component, "custom:complex_data",
            "Should map to nested struct component"
        );
        assert_eq!(selector, ".x");
    }

    // Scenario 12: All Float64 fields but different path lengths
    // Structure: { z: Float64, a: { b: Float64 } }
    // Expected: Both Float64 fields get their own instruction (.z first - shorter path)
    {
        let instructions = visualizers_for(data_result_tree, "entity_nested_path_length");
        assert_eq!(instructions.len(), 2);

        let (component, selector) = source_component_for(&instructions[0]);
        assert_eq!(
            component, "custom:path_data",
            "Should map to nested struct component"
        );
        assert_eq!(
            selector, ".z",
            "Expected selector `.z` (shorter path than `.a.b`)"
        );

        let (component, selector) = source_component_for(&instructions[1]);
        assert_eq!(
            component, "custom:path_data",
            "Should map to nested struct component"
        );
        assert_eq!(selector, ".a.b");
    }

    // Scenario 13: Entity with list of structs with mixed field types
    // Structure: { a: [{ b: Uint32, c: Float64 }] }
    // Expected: Should only recommend Float64 field (.a[].c) (Uint32 is never recommended)
    {
        let instructions = visualizers_for(data_result_tree, "entity_list_of_structs");
        assert_eq!(instructions.len(), 1);

        let (component, selector) = source_component_for(&instructions[0]);
        assert_eq!(
            component, "custom:data",
            "Should map to component containing list of structs"
        );
        assert_eq!(
            selector, ".a[].c",
            "Expected selector `.a[].c` to access Float64 field (c) within list of structs; Uint32 field (b) is never recommended"
        );
    }
}
