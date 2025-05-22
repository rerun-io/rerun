#![cfg(feature = "testing")]

use egui::Vec2;
use egui_kittest::kittest::{Key, Queryable as _};
use re_blueprint_tree::BlueprintTree;
use re_chunk_store::RowId;
use re_chunk_store::external::re_chunk::ChunkBuilder;
use re_log_types::{Timeline, build_frame_nr};
use re_types::archetypes::Points3D;
use re_viewer_context::test_context::TestContext;
use re_viewer_context::{Contents, RecommendedView, ViewClass as _, VisitorControlFlow};
use re_viewport_blueprint::test_context_ext::TestContextExt as _;
use re_viewport_blueprint::{ViewBlueprint, ViewportBlueprint};

#[test]
fn test_range_selection_in_blueprint_tree() {
    let mut test_context = TestContext::default();

    test_context.register_view_class::<re_view_spatial::SpatialView3D>();

    for i in 0..=10 {
        test_context.log_entity(format!("/entity{i}").into(), add_point_to_chunk_builder);
    }

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

    let mut blueprint_tree = BlueprintTree::default();

    // set the current timeline to the timeline where data was logged to
    test_context.set_active_timeline(Timeline::new_sequence("frame_nr"));

    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(Vec2::new(400.0, 500.0))
        .build(|ctx| {
            // We must create a side panel here (instead of the default central panel, as
            // `list_item::LabelContent`'s sizing behave differently there.
            egui::SidePanel::left("blueprint_tree")
                .default_width(400.0)
                .show(ctx, |ui| {
                    test_context.run(&ui.ctx().clone(), |viewer_ctx| {
                        let blueprint = ViewportBlueprint::try_from_db(
                            viewer_ctx.store_context.blueprint,
                            viewer_ctx.blueprint_query,
                        );

                        // expand the view
                        let view_id = blueprint
                            .visit_contents(&mut |contents, _| match contents {
                                Contents::View(id) => VisitorControlFlow::Break(*id),
                                Contents::Container(_) => VisitorControlFlow::Continue,
                            })
                            .break_value()
                            .expect("A view we know exists was not found");

                        re_context_menu::collapse_expand::collapse_expand_view(
                            viewer_ctx,
                            &view_id,
                            blueprint_tree.collapse_scope(),
                            true,
                        );

                        blueprint_tree.show(viewer_ctx, &blueprint, ui);
                    });

                    test_context.handle_system_commands();
                });
        });

    harness.run();

    let node0 = harness.get_by_label("entity0");
    node0.click();

    harness.run();
    let node2 = harness.get_by_label("entity2");
    node2.key_down(Key::Shift);
    node2.click();
    node2.key_up(Key::Shift);
    harness.run();

    let node5 = harness.get_by_label("entity5");
    node5.key_down(Key::Command);
    node5.click();
    node5.key_up(Key::Command);
    harness.run();

    let node10 = harness.get_by_label("entity10");
    node10.key_down(Key::Command);
    node10.key_down(Key::Shift);
    node10.click();
    node10.key_up(Key::Command);
    node10.key_up(Key::Shift);

    harness.run();

    harness.snapshot("range_selection_in_blueprint_tree");
}

fn add_point_to_chunk_builder(builder: ChunkBuilder) -> ChunkBuilder {
    builder.with_archetype(
        RowId::new(),
        [build_frame_nr(0)],
        &Points3D::new([[0.0, 0.0, 0.0]]),
    )
}
