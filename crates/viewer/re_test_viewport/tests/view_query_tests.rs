//! Tests for [`re_viewer_context::ViewQuery`] functionality.

use std::collections::HashSet;

use re_chunk::TimePoint;
use re_entity_db::EntityPath;
use re_log_types::example_components::{MyPoint, MyPoints};
use re_sdk_types::Visualizer;
use re_test_context::TestContext;
use re_test_context::VisualizerBlueprintContext as _;
use re_test_viewport::{TestContextExt as _, TestView};
use re_viewer_context::{ViewClass as _, ViewSystemIdentifier};
use re_viewport_blueprint::{ViewBlueprint, ViewportBlueprint};

/// Test that [`re_viewer_context::ViewQuery::iter_visualizer_instruction_for`] returns the expected results.
#[test]
fn test_iter_visualizer_instruction() {
    let mut test_context = TestContext::new_with_view_class::<TestView>();

    // Log some test data to two entities
    test_context.log_entity("entity1", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &MyPoints::new(vec![MyPoint::new(0.0, 0.0)]),
        )
    });
    test_context.log_entity("entity2", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &MyPoints::new(vec![MyPoint::new(1.0, 1.0)]),
        )
    });

    // Setup blueprint with a view that has multiple visualizer instructions
    // of the same type for the same entity
    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(TestView::identifier());

        ctx.save_visualizers(
            &EntityPath::from("entity1"),
            view.id,
            [Visualizer::new("Test"), Visualizer::new("Test")],
        );
        ctx.save_visualizers(
            &EntityPath::from("entity2"),
            view.id,
            [Visualizer::new("Test")],
        );

        blueprint.add_view_at_root(view)
    });

    // Get the ViewQuery by constructing it from the query results that were built by setup_viewport_blueprint
    test_context.run_in_egui_central_panel(|ctx, _ui| {
        let viewport_blueprint =
            ViewportBlueprint::from_db(ctx.store_context.blueprint, &test_context.blueprint_query);
        let view_blueprint = viewport_blueprint.view(&view_id).unwrap();
        let view_query = re_viewport::new_view_query(ctx, view_blueprint);

        // These are the results we want to test.
        let results: Vec<_> = view_query
            .iter_visualizer_instruction_for(ViewSystemIdentifier::from("Test"))
            .collect();

        // We should have 3 instructions total:
        // - 2 from entity1 (both Test visualizers)
        // - 1 from entity2 (one Test visualizer)
        assert_eq!(
            results.len(),
            3,
            "Expected 3 Test visualizer instructions: 2 from entity1, 1 from entity2"
        );
        assert_eq!(results[0].0.entity_path, EntityPath::from("entity1"));
        assert_eq!(results[1].0.entity_path, EntityPath::from("entity1"));
        assert_eq!(results[2].0.entity_path, EntityPath::from("entity2"));

        // Verify that each instruction has a unique ID (no duplicates)
        let instruction_ids: Vec<_> = results
            .iter()
            .map(|(_, instruction)| instruction.id)
            .collect();
        let unique_ids: HashSet<_> = instruction_ids.iter().collect();
        assert_eq!(
            instruction_ids.len(),
            unique_ids.len(),
            "All instruction IDs should be unique (no duplicates)"
        );

        // Test querying for a non-existent visualizer type
        assert!(
            view_query
                .iter_visualizer_instruction_for(ViewSystemIdentifier::from("NonExistent"))
                .next()
                .is_none(),
            "Expected 0 results for non-existent visualizer"
        );
    });
}
