#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use crate::{ui::GraphViewState, GraphView};
use egui::Vec2;
use re_chunk_store::{Chunk, RowId};
use re_log_types::{build_frame_nr, EntityPath};
use re_types::{components, Component};
use re_viewer_context::{test_context::TestContext, ViewClass, ViewClassExt as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn self_and_multi_edges() {
    let mut test_context = TestContext::default();

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context
        .view_class_registry
        .add_class::<GraphView>()
        .unwrap();

    setup_store(&mut test_context);
    run_graph_view_and_save_snapshot(&mut test_context);
}

fn setup_store(test_context: &mut TestContext) {
    let entity_path = EntityPath::from(format!("/self_and_multi_edges"));

    let nodes = [
        components::GraphNode("A".into()),
        components::GraphNode("B".into()),
    ];

    let positions = [
        components::Position2D([0.0, 0.0].into()),
        components::Position2D([200.0, 200.0].into()),
    ];

    let mut builder = Chunk::builder(entity_path.clone());
    builder = builder.with_sparse_component_batches(
        RowId::new(),
        [build_frame_nr(1)],
        [
            (components::GraphNode::descriptor(), Some(&nodes as _)),
            (components::Position2D::descriptor(), Some(&positions as _)),
        ],
    );

    test_context
        .recording_store
        .add_chunk(&Arc::new(builder.build().unwrap()))
        .unwrap();
}

pub fn setup_graph_view_blueprint(test_context: &mut TestContext) -> ViewId {
    // Views are always logged at `/{view_id}` in the blueprint store.
    let view_id = ViewId::random(); // TODO: is this fishy for testing?

    // Use the timeline that is queried for blueprints.
    let timepoint = [(test_context.blueprint_query.timeline(), 0)];

    let view_chunk = Chunk::builder(view_id.as_entity_path().clone())
        .with_archetype(
            RowId::new(),
            timepoint,
            &re_types::blueprint::archetypes::ViewBlueprint::new(GraphView::identifier().as_str()),
        )
        .build()
        .unwrap();
    test_context
        .blueprint_store
        .add_chunk(&Arc::new(view_chunk))
        .unwrap();

    // TODO: can we use the `ViewProperty` utilities for this?
    let view_contents_chunk =
        Chunk::builder(format!("{}/ViewContents", view_id.as_entity_path()).into())
            .with_archetype(
                RowId::new(),
                timepoint,
                &re_types::blueprint::archetypes::ViewContents::new(std::iter::once(
                    re_types::datatypes::Utf8::from("/**"),
                )),
            )
            .build()
            .unwrap();
    test_context
        .blueprint_store
        .add_chunk(&Arc::new(view_contents_chunk))
        .unwrap();

    view_id
}

fn run_graph_view_and_save_snapshot(test_context: &mut TestContext) {
    let view_id = setup_graph_view_blueprint(test_context);
    let view_blueprint = ViewBlueprint::try_from_db(
        view_id,
        &test_context.blueprint_store,
        &test_context.blueprint_query,
    )
    .expect("failed to get view blueprint");

    let mut view_state = GraphViewState::default();
    let class_identifier = GraphView::identifier();

    let view_class_registry = &mut test_context.view_class_registry;
    let applicable_entities_per_visualizer = view_class_registry
        .applicable_entities_for_visualizer_systems(&test_context.recording_store.store_id());

    // TODO: this is c&p from TestContext::run. Make it nicer plz ;)
    let store_context = re_viewer_context::StoreContext {
        app_id: "rerun_test".into(),
        blueprint: &test_context.blueprint_store,
        default_blueprint: None,
        recording: &test_context.recording_store,
        bundle: &Default::default(),
        caches: &Default::default(),
        hub: &Default::default(),
    };

    // Execute the queries for every `View`
    test_context.query_results = std::iter::once((view_id, {
        // TODO(andreas): This needs to be done in a store subscriber that exists per view (instance, not class!).
        // Note that right now we determine *all* visualizable entities, not just the queried ones.
        // In a store subscriber set this is fine, but on a per-frame basis it's wasteful.
        let visualizable_entities = view_class_registry
            .get_class_or_log_error(class_identifier)
            .determine_visualizable_entities(
                &applicable_entities_per_visualizer,
                &test_context.recording_store,
                &view_class_registry.new_visualizer_collection(class_identifier),
                &view_blueprint.space_origin,
            );

        // TODO:
        dbg!(&view_blueprint.contents);

        view_blueprint.contents.execute_query(
            &store_context,
            view_class_registry,
            &test_context.blueprint_query,
            view_id,
            &visualizable_entities,
        )
    }))
    .collect();

    dbg!(&test_context
        .query_results
        .iter()
        .map(|(_, qr)| &qr.tree)
        .collect::<Vec<_>>());

    //TODO(ab): this contains a lot of boilerplate which should be provided by helpers
    let mut harness = egui_kittest::Harness::builder()
        .with_size(Vec2::new(400.0, 400.0))
        .build_ui(|ui| {
            test_context.run(&ui.ctx().clone(), |viewer_ctx| {
                let view_class = test_context
                    .view_class_registry
                    .get_class_or_log_error(GraphView::identifier());

                let (view_query, system_execution_output) = re_viewport::execute_systems_for_view(
                    viewer_ctx,
                    &view_blueprint,
                    viewer_ctx.current_query().at(), // TODO: why is this even needed to be passed in?
                    &view_state,
                );

                view_class
                    .ui(
                        viewer_ctx,
                        ui,
                        dbg!(&mut view_state), // TODO: why is this still empty?
                        &view_query,
                        system_execution_output,
                    )
                    .expect("failed to run graph view ui");
            });

            test_context.handle_system_commands();
        });

    // todo: figure out how we do this for n iterations

    harness.run();

    //TODO(#8245): enable this everywhere when we have a software renderer setup
    #[cfg(target_os = "macos")]
    harness.wgpu_snapshot("self_and_multi_edges");
}
