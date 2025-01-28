#![cfg(feature = "testing")]

use egui::Vec2;

use re_blueprint_tree::BlueprintTree;
use re_chunk_store::external::re_chunk::ChunkBuilder;
use re_chunk_store::RowId;
use re_log_types::{build_frame_nr, Timeline};
use re_types::archetypes::Points3D;
use re_viewer_context::{
    test_context::TestContext, CollapseScope, RecommendedView, ViewClass, ViewId,
};
use re_viewport_blueprint::{
    test_context_ext::TestContextExt as _, ViewBlueprint, ViewportBlueprint,
};

#[test]
fn basic_blueprint_panel_should_match_snapshot() {
    let mut test_context = TestContext::default();

    test_context.register_view_class::<re_view_spatial::SpatialView3D>();

    test_context.log_entity("/entity0".into(), add_point_to_chunk_builder);
    test_context.log_entity("/entity1".into(), add_point_to_chunk_builder);
    test_context.log_entity("/entity2".into(), add_point_to_chunk_builder);

    test_context.setup_viewport_blueprint(|_, blueprint| {
        blueprint.add_views(
            std::iter::once(ViewBlueprint::new(
                re_view_spatial::SpatialView3D::identifier(),
                RecommendedView::root(),
            )),
            None,
            None,
        );
    });

    let blueprint_tree = BlueprintTree::default();
    run_blueprint_panel_and_save_snapshot(test_context, blueprint_tree, "basic_blueprint_panel");
}

// ---

#[test]
fn collapse_expand_all_blueprint_panel_should_match_snapshot() {
    for (snapshot_name, should_expand) in &[
        ("expand_all_blueprint_panel", true),
        ("collapse_all_blueprint_panel", false),
    ] {
        let mut test_context = TestContext::default();

        test_context.register_view_class::<re_view_spatial::SpatialView3D>();

        test_context.log_entity("/path/to/entity0".into(), add_point_to_chunk_builder);
        test_context.log_entity("/path/to/entity1".into(), add_point_to_chunk_builder);
        test_context.log_entity("/another/way/to/entity2".into(), add_point_to_chunk_builder);

        let view_id = ViewId::hashed_from_str("some-view-id-hash");

        test_context.setup_viewport_blueprint(|_ctx, blueprint| {
            let view = ViewBlueprint::new_with_id(
                re_view_spatial::SpatialView3D::identifier(),
                RecommendedView::root(),
                view_id,
            );

            blueprint.add_views(std::iter::once(view), None, None);

            // TODO(ab): add containers in the hierarchy (requires work on the container API,
            // currently very cumbersome to use for testing purposes).
        });

        let mut blueprint_tree = BlueprintTree::default();

        // set the current timeline to the timeline where data was logged to
        test_context.set_active_timeline(Timeline::new_sequence("frame_nr"));

        let mut harness = test_context
            .setup_kittest_for_rendering()
            .with_size(Vec2::new(400.0, 800.0))
            .build_ui(|ui| {
                test_context.run(&ui.ctx().clone(), |viewer_ctx| {
                    re_context_menu::collapse_expand::collapse_expand_view(
                        viewer_ctx,
                        &view_id,
                        CollapseScope::BlueprintTree,
                        *should_expand,
                    );

                    let blueprint = ViewportBlueprint::try_from_db(
                        viewer_ctx.store_context.blueprint,
                        viewer_ctx.blueprint_query,
                    );

                    blueprint_tree.show(viewer_ctx, &blueprint, ui);
                });

                test_context.handle_system_commands();
            });

        harness.run();
        harness.snapshot(snapshot_name);
    }
}

// ---

#[test]
fn blueprint_panel_filter_active_inside_origin_should_match_snapshot() {
    let (test_context, blueprint_tree) = setup_filter_test(Some("left"));

    run_blueprint_panel_and_save_snapshot(
        test_context,
        blueprint_tree,
        "blueprint_panel_filter_active_inside_origin",
    );
}

#[test]
fn blueprint_panel_filter_active_outside_origin_should_match_snapshot() {
    let (test_context, blueprint_tree) = setup_filter_test(Some("out"));

    run_blueprint_panel_and_save_snapshot(
        test_context,
        blueprint_tree,
        "blueprint_panel_filter_active_outside_origin",
    );
}

#[test]
fn blueprint_panel_filter_active_above_origin_should_match_snapshot() {
    let (test_context, blueprint_tree) = setup_filter_test(Some("path"));

    run_blueprint_panel_and_save_snapshot(
        test_context,
        blueprint_tree,
        "blueprint_panel_filter_active_above_origin",
    );
}

fn setup_filter_test(query: Option<&str>) -> (TestContext, BlueprintTree) {
    let mut test_context = TestContext::default();
    test_context.register_view_class::<re_view_spatial::SpatialView3D>();

    test_context.log_entity("/path/to/left".into(), add_point_to_chunk_builder);
    test_context.log_entity("/path/to/right".into(), add_point_to_chunk_builder);
    test_context.log_entity("/path/is/outside".into(), add_point_to_chunk_builder);

    test_context.setup_viewport_blueprint(|_, blueprint| {
        blueprint.add_views(
            std::iter::once(ViewBlueprint::new(
                re_view_spatial::SpatialView3D::identifier(),
                RecommendedView {
                    origin: "/path/to".into(),
                    query_filter: "+ /**".try_into().expect("valid entity path filter"),
                },
            )),
            None,
            None,
        );
    });

    let mut blueprint_tree = BlueprintTree::default();

    // This trick here is to run the blueprint panel for a frame, such that it registers the current
    // application id. This way, the blueprint panel will not discard the filter state we set up
    // when it's run for the snapshot.
    test_context.run_in_egui_central_panel(|ctx, ui| {
        let blueprint =
            ViewportBlueprint::try_from_db(ctx.store_context.blueprint, ctx.blueprint_query);

        blueprint_tree.show(ctx, &blueprint, ui);
    });

    if let Some(query) = query {
        blueprint_tree.activate_filter(query);
    }

    (test_context, blueprint_tree)
}

// ---

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
    // set the current timeline to the timeline where data was logged to
    test_context.set_active_timeline(Timeline::new_sequence("frame_nr"));

    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(Vec2::new(400.0, 800.0))
        .build_ui(|ui| {
            test_context.run(&ui.ctx().clone(), |viewer_ctx| {
                let blueprint = ViewportBlueprint::try_from_db(
                    viewer_ctx.store_context.blueprint,
                    viewer_ctx.blueprint_query,
                );

                blueprint_tree.show(viewer_ctx, &blueprint, ui);
            });

            test_context.handle_system_commands();
        });

    harness.run();
    harness.snapshot(snapshot_name);
}
