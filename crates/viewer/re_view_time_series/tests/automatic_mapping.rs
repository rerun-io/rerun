//! Test automatic component mapping and casting with different scenarios.
//!
//! This test verifies that the time series view correctly picks components
//! for visualization and installs mappings when needed.

use std::sync::Arc;

use re_log_types::external::arrow::array::{Float64Array, Int16Array, Int32Array};
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
            builder.with_archetype_auto_row([(timeline, i)], &archetypes::Scalars::single(i as f64))
        });
    }

    // Scenario 2: Entity with builtin Scalar and custom Float64 component
    // Expected: Should pick builtin Scalar component over custom
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
    // Expected: Should pick the custom component since it matches the datatype
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
    // Expected: Should pick the first one alphabetically
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
    // Expected: Should pick temporal component, not static
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
    // Expected: Should not visualize (can't plot static data in time series)
    test_context.log_entity("entity_rerun_type_static_only", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &DynamicArchetype::new("custom")
                .with_component::<components::LinearSpeed>("static_only", [999.0]),
        )
    });

    // Scenario 7: Entity with multiple fully custom components (no known Rerun types)
    // Expected: Should pick Float64 over Int types, preferring higher precision
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
    // Expected: Should prefer the fully custom component over LinearSpeed (both PhysicalDatatypeOnly)
    // Note: Fully custom component is named "zebra_custom" (alphabetically after "linear_speed")
    //       to ensure preference is based on component type metadata, not alphabetical ordering
    for i in 0..10 {
        test_context.log_entity("entity_fully_custom_vs_rerun_type", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("custom")
                    // Known Rerun component type (LinearSpeed) - has semantic meaning
                    .with_component::<components::LinearSpeed>("linear_speed", [i as f64 * 20.0])
                    // Fully custom Float64 - no ComponentType metadata (alphabetically after linear_speed)
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

/// Helper to get the single visualizer instruction for an entity.
/// Ensures there's exactly one visualizer and returns it.
fn single_visualizer_for<'a>(
    data_result_tree: &'a re_viewer_context::DataResultTree,
    entity_path: &str,
) -> &'a re_viewer_context::VisualizerInstruction {
    let result = data_result_tree
        .lookup_result_by_path(EntityPath::from(entity_path).hash())
        .unwrap_or_else(|| panic!("{entity_path} should be in query results"));

    assert_eq!(
        result.visualizer_instructions.len(),
        1,
        "{entity_path} should have exactly one visualizer",
    );

    &result.visualizer_instructions[0]
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

fn check_visualizer_instructions(test_context: &TestContext, view_id: ViewId) {
    let query_result = test_context
        .query_results
        .get(&view_id)
        .expect("View should have query results");
    let data_result_tree = &query_result.tree;
    let scalar_component = archetypes::Scalars::descriptor_scalars().component;

    // Scenario 1: Entity with only builtin Scalar component
    {
        let instruction = single_visualizer_for(data_result_tree, "entity_builtin_only");
        let mapping = scalar_mapping_for(instruction);

        assert!(
            mapping.is_identity_mapping(scalar_component),
            "Expected SourceComponent mapping for builtin Scalar"
        );
    }

    // Scenario 2: Entity with builtin Scalar and custom component
    {
        let instruction =
            single_visualizer_for(data_result_tree, "entity_builtin_and_custom_same_type");
        let mapping = scalar_mapping_for(instruction);

        assert!(
            mapping.is_identity_mapping(scalar_component),
            "Expected SourceComponent mapping for builtin Scalar"
        );
    }

    // Scenario 3: Entity with only custom Float64 component (temporal)
    {
        let instruction = single_visualizer_for(data_result_tree, "entity_custom_only_temporal");
        let mapping = scalar_mapping_for(instruction);

        match mapping {
            re_viewer_context::VisualizerComponentSource::SourceComponent {
                source_component,
                selector,
            } => {
                assert!(
                    source_component.as_str().contains("custom"),
                    "Should map to custom component: {}",
                    source_component.as_str()
                );
                assert!(
                    selector.is_empty(),
                    "Expected empty selector for direct component mapping"
                );
            }
            _ => panic!("Expected SourceComponent mapping for custom component"),
        }
    }

    // Scenario 4: Entity with multiple known Rerun component types (LinearSpeed)
    {
        let instruction =
            single_visualizer_for(data_result_tree, "entity_multiple_rerun_types_temporal");
        let mapping = scalar_mapping_for(instruction);

        match mapping {
            re_viewer_context::VisualizerComponentSource::SourceComponent {
                source_component,
                selector,
            } => {
                assert_eq!(
                    source_component.as_str(),
                    "custom:alpha_component",
                    "Should pick alphabetically first custom component (alpha_component): {}",
                    source_component.as_str()
                );
                assert!(
                    selector.is_empty(),
                    "Expected empty selector for direct component mapping"
                );
            }
            _ => panic!("Expected SourceComponent mapping for alphabetically first component"),
        }
    }

    // Scenario 5: Entity with static and temporal known Rerun component type (LinearSpeed)
    {
        let instruction =
            single_visualizer_for(data_result_tree, "entity_rerun_type_static_and_temporal");
        let mapping = scalar_mapping_for(instruction);

        match mapping {
            re_viewer_context::VisualizerComponentSource::SourceComponent {
                source_component,
                ..
            } => {
                assert!(
                    source_component.as_str().contains("temporal"),
                    "Should pick temporal LinearSpeed component, not static: {}",
                    source_component.as_str()
                );
            }
            _ => panic!("Expected SourceComponent mapping for temporal component"),
        }
    }

    // Scenario 6: Entity with only static known Rerun component type (LinearSpeed)
    {
        // We don't emit data result elements if there's no visualizer instructions in the first place,
        // so the lookup should come back empty.
        let result = data_result_tree
            .lookup_result_by_path(EntityPath::from("entity_rerun_type_static_only").hash());

        assert!(
            result.is_none(),
            "entity_rerun_type_static_only should not have any data result out of the box since it is marked as non-visualizable, but got: {result:?}",
        );
    }

    // Scenario 7: Entity with multiple fully custom components (Float64 and Int types)
    {
        let instruction =
            single_visualizer_for(data_result_tree, "entity_fully_custom_mixed_types");
        let mapping = scalar_mapping_for(instruction);

        match mapping {
            re_viewer_context::VisualizerComponentSource::SourceComponent {
                source_component,
                selector,
            } => {
                assert_eq!(
                    source_component.as_str(),
                    "custom:beta_component",
                    "Should pick Float64 component (beta_component) over Int types: {}",
                    source_component.as_str()
                );
                assert!(
                    selector.is_empty(),
                    "Expected empty selector for direct component mapping"
                );
            }
            _ => panic!("Expected SourceComponent mapping for entity_fully_custom_mixed_types"),
        }
    }

    // Scenario 8: Entity with fully custom Float64 vs known Rerun type (LinearSpeed)
    {
        let instruction =
            single_visualizer_for(data_result_tree, "entity_fully_custom_vs_rerun_type");
        let mapping = scalar_mapping_for(instruction);

        match mapping {
            re_viewer_context::VisualizerComponentSource::SourceComponent {
                source_component,
                selector,
            } => {
                assert_eq!(
                    source_component.as_str(),
                    "custom:zebra_custom",
                    "Should pick fully custom Float64 (zebra_custom, no ComponentType) over known Rerun type (linear_speed, LinearSpeed ComponentType), even though it's alphabetically after it: {}",
                    source_component.as_str()
                );
                assert!(
                    selector.is_empty(),
                    "Expected empty selector for direct component mapping"
                );
            }
            _ => panic!("Expected SourceComponent mapping for fully custom component"),
        }
    }

    // Scenario 9: Entity with fully custom Float64 vs Scalars (NativeSemantics match)
    {
        let instruction = single_visualizer_for(data_result_tree, "entity_fully_custom_vs_scalars");
        let mapping = scalar_mapping_for(instruction);

        match mapping {
            re_viewer_context::VisualizerComponentSource::SourceComponent {
                source_component,
                selector,
            } => {
                assert_eq!(
                    source_component.as_str(),
                    "custom:scalars",
                    "Should pick Scalars component (NativeSemantics match) over fully custom Float64 (PhysicalDatatypeOnly), even though fully custom is alphabetically first: {}",
                    source_component.as_str()
                );
                assert!(
                    selector.is_empty(),
                    "Expected empty selector for direct component mapping"
                );
            }
            _ => panic!("Expected SourceComponent mapping for Scalars component"),
        }
    }
}
