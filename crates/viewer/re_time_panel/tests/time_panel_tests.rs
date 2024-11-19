use egui::{CentralPanel, Vec2};
use re_chunk_store::LatestAtQuery;
use re_time_panel::TimePanel;
use re_types::blueprint::components::PanelState;
use re_viewer_context::blueprint_timeline;
use re_viewer_context::test_context::TestContext;
use re_viewport_blueprint::ViewportBlueprint;

#[test]
pub fn time_panel_should_match_snapshot() {
    let mut test_context = TestContext::default();
    let mut panel = TimePanel::default();

    let mut harness = egui_kittest::Harness::builder()
        .with_size(Vec2::new(400.0, 250.0))
        .build(move |ctx| {
            test_context.run_simple(ctx, |viewer_ctx| {
                let (sender, _) = std::sync::mpsc::channel();
                let blueprint = ViewportBlueprint::try_from_db(
                    viewer_ctx.store_context.blueprint,
                    &LatestAtQuery::latest(blueprint_timeline()),
                    sender,
                );

                CentralPanel::default().show(ctx, |ui| {
                    panel.show_panel(
                        viewer_ctx,
                        &blueprint,
                        viewer_ctx.recording(),
                        viewer_ctx.rec_cfg,
                        ui,
                        PanelState::Expanded,
                    )
                });
            });

            test_context.handle_system_command();
        });

    harness.run();

    harness.wgpu_snapshot("time_panel");
}
