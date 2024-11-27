use egui::{CentralPanel, Vec2};
use re_chunk_store::{Chunk, LatestAtQuery, RowId};
use re_log_types::example_components::MyPoint;
use re_log_types::external::re_types_core::Component;
use re_log_types::{build_frame_nr, EntityPath};
use re_time_panel::TimePanel;
use re_viewer_context::blueprint_timeline;
use re_viewer_context::test_context::TestContext;
use re_viewport_blueprint::ViewportBlueprint;
use std::sync::Arc;

#[test]
pub fn time_panel_should_match_snapshot() {
    let mut test_context = TestContext::default();
    let mut panel = TimePanel::default();

    let points1 = MyPoint::from_iter(0..1);

    for i in 0..2 {
        let entity_path = EntityPath::from(format!("/entity/{i}"));
        let mut builder = Chunk::builder(entity_path.clone());
        for frame in [10, 11, 12, 15, 18, 100, 102, 104].map(|frame| frame + i) {
            builder = builder.with_sparse_component_batches(
                RowId::new(),
                [build_frame_nr(frame)],
                [(MyPoint::name(), Some(&points1 as _))],
            );
        }
        test_context
            .recording_store
            .add_chunk(&Arc::new(builder.build().unwrap()))
            .unwrap();
    }

    let mut harness = egui_kittest::Harness::builder()
        .with_size(Vec2::new(700.0, 300.0))
        .build(move |ctx| {
            test_context.run_simple(ctx, |viewer_ctx| {
                let (sender, _) = std::sync::mpsc::channel();
                let blueprint = ViewportBlueprint::try_from_db(
                    viewer_ctx.store_context.blueprint,
                    &LatestAtQuery::latest(blueprint_timeline()),
                    sender,
                );

                CentralPanel::default().show(ctx, |ui| {
                    let time_ctrl_before = viewer_ctx.rec_cfg.time_ctrl.read().clone();
                    let mut time_ctrl_after = time_ctrl_before.clone();

                    panel.show_expanded_with_header(
                        viewer_ctx,
                        &blueprint,
                        viewer_ctx.recording(),
                        &mut time_ctrl_after,
                        ui,
                    );
                });
            });

            test_context.handle_system_command();
        });

    harness.run();

    harness.wgpu_snapshot("time_panel");
}
