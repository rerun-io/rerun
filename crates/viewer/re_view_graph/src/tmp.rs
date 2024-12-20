use egui::Vec2;
use re_chunk_store::{Chunk, LatestAtQuery, RowId};
use re_log_types::{build_frame_nr, EntityPath};
use re_types::{components, Component};
use crate::{GraphView, ui::GraphViewState};
use re_viewer_context::{
    blueprint_timeline, test_context::TestContext, SystemExecutionOutput, ViewClass, ViewId,
};
use re_viewport_blueprint::{ViewBlueprint, ViewportBlueprint};

#[test]
pub fn self_and_multi_edges() {
    let mut test_context = TestContext::default();
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

pub fn setup_blueprint(test_context: &mut TestContext) {
    let view_id = ViewId::random(); // TODO: is this fishy for testing?

    let mut builder = Chunk::builder(view_id.as_entity_path().clone());
    let time_point = test_context.blueprint_query.at();
    builder.with_archetype(RowId::new(), time_point, re_types::blueprint::archetypes::ViewBlueprint {
        class_identifier: GraphView::identifier().0.into(),
        display_name: None,
        space_origin: None,
        visible: None,
    });

    test_context.blueprint_store
    .add_chunk(&Arc::new(builder.build().unwrap()))
    .unwrap();
}

fn run_graph_view_and_save_snapshot(mut test_context: TestContext, _snapshot_name: &str) {
    setup_blueprint(&mut test_context);

    let view_class = GraphView::default();
    let view_blueprint = ViewBlueprint::from_b

    //TODO(ab): this contains a lot of boilerplate which should be provided by helpers
    let mut harness = egui_kittest::Harness::builder()
        .with_size(Vec2::new(400.0, 400.0))
        .build_ui(|ui| {
            test_context.run(&ui.ctx().clone(), |viewer_ctx| {
                // let blueprint = ViewportBlueprint::try_from_db(
                //     viewer_ctx.store_context.blueprint,
                //     &LatestAtQuery::latest(blueprint_timeline()),
                // );

                //let mut time_ctrl = viewer_ctx.rec_cfg.time_ctrl.read().clone();

                // todo: run systems

                // todo: draw ui
                // view_class.ui(
                //     viewer_ctx,
                //     ui,
                //     GraphViewState::default(),
                //     &ctx.query(),
                //     SystemExecutionOutput {
                //         view_systems: Default::default(),
                //         context_systems: Default::default(),
                //         draw_data: Default::default(),
                //     },
                // );

                //*viewer_ctx.rec_cfg.time_ctrl.write() = time_ctrl;
            });

            test_context.handle_system_commands();
        });

    // todo: figure out how we do this for n iterations

    harness.run();

    //TODO(#8245): enable this everywhere when we have a software renderer setup
    #[cfg(target_os = "macos")]
    harness.wgpu_snapshot(_snapshot_name);
}
