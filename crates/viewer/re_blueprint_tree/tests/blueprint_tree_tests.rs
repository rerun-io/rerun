use egui::Vec2;
use std::sync::Arc;

use re_blueprint_tree::BlueprintTree;
use re_chunk_store::external::re_chunk::ChunkBuilder;
use re_chunk_store::{Chunk, RowId};
use re_entity_db::external::re_chunk_store::LatestAtQuery;
use re_log_types::{build_frame_nr, EntityPath};
use re_types::archetypes::Points3D;
use re_viewer_context::{
    blueprint_timeline, test_context::TestContext, RecommendedView, ViewClass, ViewId,
};
use re_viewport_blueprint::{ViewBlueprint, ViewportBlueprint};

#[test]
fn blueprint_panel_should_match_snapshot() {
    let mut test_context = TestContext::default();

    test_context.register_view_class::<re_view_spatial::SpatialView3D>();

    test_context.log_entity("/entity/0".into(), add_point_to_chunk_builder);
    test_context.log_entity("/entity/1".into(), add_point_to_chunk_builder);
    test_context.log_entity("/entity/2".into(), add_point_to_chunk_builder);

    egui::__run_test_ctx(|egui_ctx| {
        test_context.run(egui_ctx, |ctx| {
            let viewport_blueprint = ViewportBlueprint::try_from_db(
                &test_context.blueprint_store,
                &test_context.blueprint_query,
            );
            viewport_blueprint.add_views(
                std::iter::once(ViewBlueprint::new(
                    re_view_spatial::SpatialView3D::identifier(),
                    RecommendedView::root(),
                )),
                None,
                None,
            );

            viewport_blueprint.save_to_blueprint_store(ctx, &ctx.view_class_registry);
        })
    });

    let blueprint_tree = BlueprintTree::default();
    run_blueprint_panel_and_save_snapshot(test_context, blueprint_tree, "blueprint_panel")
}

fn add_point_to_chunk_builder(builder: ChunkBuilder) -> ChunkBuilder {
    builder.with_archetype(
        RowId::new(),
        [build_frame_nr(0)],
        &Points3D::new([[0.0, 0.0, 0.0]]),
    )
}

fn run_blueprint_panel_and_save_snapshot(
    mut test_context: TestContext,
    mut blueprint_tree: BlueprintTree,
    snapshot_name: &str,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(Vec2::new(400.0, 800.0))
        .build_ui(|ui| {
            test_context.run(&ui.ctx().clone(), |viewer_ctx| {
                let blueprint = ViewportBlueprint::try_from_db(
                    viewer_ctx.store_context.blueprint,
                    &LatestAtQuery::latest(blueprint_timeline()),
                );

                blueprint_tree.show(viewer_ctx, &blueprint, ui);
            });

            test_context.handle_system_commands();
        });

    harness.run();
    harness.snapshot(snapshot_name);
}
