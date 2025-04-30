//! Basic tests for the graph view, mostly focused on edge cases (pun intended).

use egui::Vec2;

use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_types::archetypes;
use re_view_graph::GraphView;
use re_viewer_context::external::egui_kittest::SnapshotOptions;
use re_viewer_context::{test_context::TestContext, RecommendedView, ViewClass as _};
use re_viewport_blueprint::{test_context_ext::TestContextExt as _, ViewBlueprint};

#[test]
pub fn coincident_nodes() {
    let mut test_context = TestContext::default();
    let name = "coincident_nodes";

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<re_view_graph::GraphView>();

    let timepoint = TimePoint::from([(test_context.active_timeline(), 1)]);
    test_context.log_entity(name.into(), |builder| {
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
    let mut test_context = TestContext::default();
    let name = "self_and_multi_edges";

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context
        .view_class_registry
        .add_class::<GraphView>()
        .unwrap();

    let timepoint = TimePoint::from([(test_context.active_timeline(), 1)]);
    test_context.log_entity(name.into(), |builder| {
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
    let mut test_context = TestContext::default();
    let name = "multi_graphs";

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context
        .view_class_registry
        .add_class::<GraphView>()
        .unwrap();

    let timepoint = TimePoint::from([(test_context.active_timeline(), 1)]);
    test_context.log_entity("graph1".into(), |builder| {
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
    test_context.log_entity("graph2".into(), |builder| {
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
    let view_id = test_context.setup_viewport_blueprint(|_, blueprint| {
        let view_blueprint = ViewBlueprint::new(
            re_view_graph::GraphView::identifier(),
            RecommendedView::root(),
        );

        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);
        view_id
    });

    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .with_max_steps(256) // Give it some time to settle the graph.
        .build_ui(|ui| {
            test_context.run(&ui.ctx().clone(), |ctx| {
                let view_class = ctx
                    .view_class_registry()
                    .get_class_or_log_error(GraphView::identifier());

                let view_blueprint = ViewBlueprint::try_from_db(
                    view_id,
                    ctx.store_context.blueprint,
                    ctx.blueprint_query,
                )
                .expect("we just created that view");

                let mut view_states = test_context.view_states.lock();
                let view_state = view_states.get_mut_or_create(view_id, view_class);

                let (view_query, system_execution_output) =
                    re_viewport::execute_systems_for_view(ctx, &view_blueprint, view_state);

                view_class
                    .ui(ctx, ui, view_state, &view_query, system_execution_output)
                    .expect("failed to run graph view ui");
            });

            test_context.handle_system_commands();
        });

    harness.run();
    harness.snapshot_options(name, &SnapshotOptions::default().threshold(1.3));
}
