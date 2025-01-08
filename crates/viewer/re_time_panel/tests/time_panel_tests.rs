use std::sync::Arc;

use egui::Vec2;

use re_chunk_store::{Chunk, LatestAtQuery, RowId};
use re_log_types::example_components::MyPoint;
use re_log_types::external::re_types_core::Component;
use re_log_types::{build_frame_nr, EntityPath};
use re_time_panel::TimePanel;
use re_viewer_context::blueprint_timeline;
use re_viewer_context::test_context::TestContext;
use re_viewport_blueprint::ViewportBlueprint;

#[test]
pub fn time_panel_two_sections_should_match_snapshot() {
    TimePanel::ensure_registered_subscribers();
    let mut test_context = TestContext::default();

    let points1 = MyPoint::from_iter(0..1);
    for i in 0..2 {
        let entity_path = EntityPath::from(format!("/entity/{i}"));
        let mut builder = Chunk::builder(entity_path.clone());
        for frame in [10, 11, 12, 15, 18, 100, 102, 104].map(|frame| frame + i) {
            builder = builder.with_sparse_component_batches(
                RowId::new(),
                [build_frame_nr(frame)],
                [(MyPoint::descriptor(), Some(&points1 as _))],
            );
        }
        test_context
            .recording_store
            .add_chunk(&Arc::new(builder.build().unwrap()))
            .unwrap();
    }

    run_time_panel_and_save_snapshot(test_context, "time_panel_two_sections");
}

#[test]
pub fn time_panel_dense_data_should_match_snapshot() {
    TimePanel::ensure_registered_subscribers();
    let mut test_context = TestContext::default();

    let points1 = MyPoint::from_iter(0..1);

    let mut rng_seed = 0b1010_1010_1010_1010_1010_1010_1010_1010u64;
    let mut rng = || {
        rng_seed ^= rng_seed >> 12;
        rng_seed ^= rng_seed << 25;
        rng_seed ^= rng_seed >> 27;
        rng_seed.wrapping_mul(0x2545_f491_4f6c_dd1d)
    };

    let entity_path = EntityPath::from("/entity");
    let mut builder = Chunk::builder(entity_path.clone());
    for frame in 0..1_000 {
        if rng() & 0b1 == 0 {
            continue;
        }

        builder = builder.with_sparse_component_batches(
            RowId::new(),
            [build_frame_nr(frame)],
            [(MyPoint::descriptor(), Some(&points1 as _))],
        );
    }
    test_context
        .recording_store
        .add_chunk(&Arc::new(builder.build().unwrap()))
        .unwrap();

    run_time_panel_and_save_snapshot(test_context, "time_panel_dense_data");
}

fn run_time_panel_and_save_snapshot(mut test_context: TestContext, _snapshot_name: &str) {
    let mut panel = TimePanel::default();

    //TODO(ab): this contains a lot of boilerplate which should be provided by helpers
    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(Vec2::new(700.0, 300.0))
        .build_ui(|ui| {
            test_context.run(&ui.ctx().clone(), |viewer_ctx| {
                let blueprint = ViewportBlueprint::try_from_db(
                    viewer_ctx.store_context.blueprint,
                    &LatestAtQuery::latest(blueprint_timeline()),
                );

                let mut time_ctrl = viewer_ctx.rec_cfg.time_ctrl.read().clone();

                panel.show_expanded_with_header(
                    viewer_ctx,
                    &blueprint,
                    viewer_ctx.recording(),
                    &mut time_ctrl,
                    ui,
                );

                *viewer_ctx.rec_cfg.time_ctrl.write() = time_ctrl;
            });

            test_context.handle_system_commands();
        });

    harness.run();
    harness.snapshot(_snapshot_name);
}
