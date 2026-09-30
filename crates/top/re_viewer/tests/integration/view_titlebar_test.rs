use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::ViewClass as _;
use re_viewport::ViewportUi;
use re_viewport_blueprint::ViewBlueprint;

/// Two 2D views side by side, where the left one turns its title bar off.
/// The left view takes up the full height of its tile, and the right view keeps its title bar.
#[test]
fn test_view_without_titlebar() {
    let mut test_context = TestContext::new();
    test_context.register_view_class::<re_view_spatial::SpatialView2D>();

    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let mut without_titlebar =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView2D::identifier());
        without_titlebar.titlebar = false;
        let with_titlebar =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView2D::identifier());

        blueprint.add_views([without_titlebar, with_titlebar].into_iter(), None, None);
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([600.0, 400.0])
        .build_ui(|ui| {
            test_context.run_ui(ui, |ctx, ui| {
                let viewport_blueprint = re_viewport_blueprint::ViewportBlueprint::from_db(
                    ctx.blueprint_db(),
                    &test_context.blueprint_query,
                );
                let viewport_ui = ViewportUi::new(viewport_blueprint);
                viewport_ui.viewport_ui(ui, ctx, &mut test_context.view_states.lock());
            });

            test_context.handle_system_commands(ui.ctx());
        });
    harness.run();
    harness.snapshot("view_without_titlebar");
}
