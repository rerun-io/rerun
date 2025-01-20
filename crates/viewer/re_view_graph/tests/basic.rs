//! Basic tests for the graph view, mostly focused on edge cases (pun intended).

use std::sync::Arc;

use egui::Vec2;

use re_chunk_store::{Chunk, RowId};
use re_entity_db::EntityPath;
use re_types::{components, Component as _};
use re_view_graph::{GraphView, GraphViewState};
use re_viewer_context::{test_context::TestContext, RecommendedView, ViewClass};
use re_viewport_blueprint::test_context_ext::TestContextExt as _;
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn coincident_nodes() {
    let mut test_context = TestContext::default();
    let name = "coincident_nodes";

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<re_view_graph::GraphView>();

    let entity_path = EntityPath::from(name);

    let nodes = [
        components::GraphNode("A".into()),
        components::GraphNode("B".into()),
    ];

    let edges = [components::GraphEdge(("A", "B").into())];

    let directed = components::GraphType::Directed;

    let positions = [
        components::Position2D([42.0, 42.0].into()),
        components::Position2D([42.0, 42.0].into()),
    ];

    let mut builder = Chunk::builder(entity_path.clone());
    builder = builder.with_sparse_component_batches(
        RowId::new(),
        [(test_context.active_timeline(), 1)],
        [
            (components::GraphNode::descriptor(), Some(&nodes as _)),
            (components::Position2D::descriptor(), Some(&positions as _)),
            (components::GraphEdge::descriptor(), Some(&edges as _)),
            (components::GraphType::descriptor(), Some(&[directed] as _)),
        ],
    );

    test_context
        .recording_store
        .add_chunk(&Arc::new(builder.build().unwrap()))
        .unwrap();

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

    let entity_path = EntityPath::from(name);

    let nodes = [
        components::GraphNode("A".into()),
        components::GraphNode("B".into()),
    ];

    let edges = [
        // self-edges
        components::GraphEdge(("A", "A").into()),
        components::GraphEdge(("B", "B").into()),
        // duplicated edges
        components::GraphEdge(("A", "B").into()),
        components::GraphEdge(("A", "B").into()),
        components::GraphEdge(("B", "A").into()),
        // duplicated self-edges
        components::GraphEdge(("A", "A").into()),
        // TODO(grtlr): investigate instabilities in the graph layout to be able
        // to test dynamically placed nodes.
        // implicit edges
        // components::GraphEdge(("B", "C").into()),
        // components::GraphEdge(("C", "C").into()),
    ];

    let directed = components::GraphType::Directed;

    let positions = [
        components::Position2D([0.0, 0.0].into()),
        components::Position2D([200.0, 200.0].into()),
    ];

    let mut builder = Chunk::builder(entity_path.clone());
    builder = builder.with_sparse_component_batches(
        RowId::new(),
        [(test_context.active_timeline(), 1)],
        [
            (components::GraphNode::descriptor(), Some(&nodes as _)),
            (components::Position2D::descriptor(), Some(&positions as _)),
            (components::GraphEdge::descriptor(), Some(&edges as _)),
            (components::GraphType::descriptor(), Some(&[directed] as _)),
        ],
    );

    test_context
        .recording_store
        .add_chunk(&Arc::new(builder.build().unwrap()))
        .unwrap();

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

    let mut view_state = GraphViewState::default();

    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .with_max_steps(256) // Give it some time to settle the graph.
        .build_ui(|ui| {
            test_context.run(&ui.ctx().clone(), |ctx| {
                let view_class = ctx
                    .view_class_registry
                    .get_class_or_log_error(GraphView::identifier());

                let view_blueprint = ViewBlueprint::try_from_db(
                    view_id,
                    ctx.store_context.blueprint,
                    ctx.blueprint_query,
                )
                .expect("we just created that view");

                let (view_query, system_execution_output) = re_viewport::execute_systems_for_view(
                    ctx,
                    &view_blueprint,
                    ctx.current_query().at(), // TODO(andreas): why is this even needed to be passed in?
                    &view_state,
                );

                view_class
                    .ui(
                        ctx,
                        ui,
                        &mut view_state,
                        &view_query,
                        system_execution_output,
                    )
                    .expect("failed to run graph view ui");
            });

            test_context.handle_system_commands();
        });

    harness.run();
    harness.snapshot(name);
}
