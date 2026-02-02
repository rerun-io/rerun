//! Basic tests for the graph view, mostly focused on edge cases (pun intended).

use egui::Vec2;
use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::archetypes;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_view_graph::GraphView;
use re_viewer_context::ViewClass as _;
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn coincident_nodes() {
    let mut test_context = TestContext::new();
    let name = "coincident_nodes";

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<re_view_graph::GraphView>();

    let timepoint = TimePoint::from([(
        test_context
            .active_timeline()
            .expect("should have an active timeline"),
        1,
    )]);
    test_context.log_entity(name, |builder| {
        builder
            .with_archetype(
                RowId::new(),
                timepoint.clone(),
                &archetypes::GraphNodes::new(["A", "B"])
                    .with_positions([[42.0, 42.0], [42.0, 42.0]]),
            )
            .with_archetype(
                RowId::new(),
                timepoint,
                &archetypes::GraphEdges::new([("A", "B")]).with_directed_edges(),
            )
    });

    run_graph_view_and_save_snapshot(&mut test_context, name, Vec2::new(100.0, 100.0));
}

#[test]
pub fn self_and_multi_edges() {
    let mut test_context = TestContext::new();
    let name = "self_and_multi_edges";

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<GraphView>();

    let timepoint = TimePoint::from([(
        test_context
            .active_timeline()
            .expect("should have an active timeline"),
        1,
    )]);
    test_context.log_entity(name, |builder| {
        builder
            .with_archetype(
                RowId::new(),
                timepoint.clone(),
                &archetypes::GraphNodes::new(["A", "B"])
                    .with_positions([[0.0, 0.0], [200.0, 200.0]]),
            )
            .with_archetype(
                RowId::new(),
                timepoint,
                &archetypes::GraphEdges::new([
                    // self-edges
                    ("A", "A"),
                    ("B", "B"),
                    // duplicated edges
                    ("A", "B"),
                    ("A", "B"),
                    ("B", "A"),
                    // duplicated self-edges
                    ("A", "A"),
                    // TODO(grtlr): investigate instabilities in the graph layout to be able
                    // to test dynamically placed nodes.
                    // implicit edges
                    // ("B", "C"),
                    // ("C", "C"),
                ])
                .with_directed_edges(),
            )
    });

    run_graph_view_and_save_snapshot(&mut test_context, name, Vec2::new(400.0, 400.0));
}

#[test]
pub fn multi_graphs() {
    let mut test_context = TestContext::new();
    let name = "multi_graphs";

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<GraphView>();

    let timepoint = TimePoint::from([(
        test_context
            .active_timeline()
            .expect("Should have an active timeline"),
        1,
    )]);
    test_context.log_entity("graph1", |builder| {
        builder
            .with_archetype(
                RowId::new(),
                timepoint.clone(),
                &archetypes::GraphNodes::new(["A", "B"]).with_positions([[0.0, 0.0], [0.0, 0.0]]),
            )
            .with_archetype(
                RowId::new(),
                timepoint.clone(),
                &archetypes::GraphEdges::new([("A", "B")]),
            )
    });
    test_context.log_entity("graph2", |builder| {
        builder
            .with_archetype(
                RowId::new(),
                timepoint.clone(),
                &archetypes::GraphNodes::new(["A", "B"])
                    .with_positions([[80.0, 80.0], [80.0, 80.0]]),
            )
            .with_archetype(
                RowId::new(),
                timepoint,
                &archetypes::GraphEdges::new([("A", "B")]).with_directed_edges(),
            )
    });

    run_graph_view_and_save_snapshot(&mut test_context, name, Vec2::new(400.0, 400.0));
}

fn run_graph_view_and_save_snapshot(test_context: &mut TestContext, name: &str, size: Vec2) {
    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_graph::GraphView::identifier(),
        ))
    });

    let mut harness = test_context
        // Don't use `setup_kittest_for_rendering_ui` since those graph lines cause a lot of renderer discrepancies, similar to 3D rendering.
        .setup_kittest_for_rendering_3d(size)
        .with_max_steps(256) // Give it some time to settle the graph.
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();
    harness.snapshot(name);
}
