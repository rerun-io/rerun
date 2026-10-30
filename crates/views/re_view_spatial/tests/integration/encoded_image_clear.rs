#![expect(clippy::unwrap_used)] // It's a test!

use re_chunk_store::RowId;
use re_sdk_types::archetypes::EncodedImage;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{TimeControlCommand, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

/// A 64x48 PNG filled with a single color.
fn solid_png(color: [u8; 3]) -> Vec<u8> {
    let image = image::RgbImage::from_pixel(64, 48, image::Rgb(color));
    let mut buf = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    buf
}

/// Clearing the fields of an encoded image hides it. Nothing is shown at the time of the clear,
/// and the image logged after it is shown again.
#[test]
fn test_encoded_image_clear() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();
    let timeline = test_context
        .active_timeline()
        .expect("should have an active timeline");

    test_context.log_entity("image", |builder| {
        builder
            .with_archetype(
                RowId::new(),
                [(timeline, 1_i64)],
                &EncodedImage::from_file_contents(solid_png([255, 0, 0])),
            )
            .with_archetype(
                RowId::new(),
                [(timeline, 2_i64)],
                &EncodedImage::clear_fields(),
            )
            .with_archetype(
                RowId::new(),
                [(timeline, 3_i64)],
                &EncodedImage::from_file_contents(solid_png([0, 0, 255])),
            )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView2D::identifier(),
        ))
    });

    // Decoding runs on a separate thread, so the harness steps in real time.
    let step_dt_seconds = 1.0 / 4.0;
    let max_total_time_seconds = 60.0;
    let viewport_size = egui::vec2(300.0, 200.0);

    let mut harness = test_context
        .setup_kittest_for_rendering_3d(viewport_size)
        .with_step_dt(step_dt_seconds)
        .with_max_steps((max_total_time_seconds / step_dt_seconds) as u64)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);

            std::thread::sleep(std::time::Duration::from_millis(20));
        });

    for (label, time) in [("red_image", 1), ("cleared", 2), ("blue_image", 3)] {
        test_context.send_time_commands(
            test_context.active_store_id(),
            [
                TimeControlCommand::SetActiveTimeline(*timeline.name()),
                TimeControlCommand::SetTime(time.into()),
            ],
        );

        harness.try_run_realtime().unwrap();
        harness.snapshot_options(
            format!("encoded_image_clear_{label}"),
            &re_ui::testing::default_snapshot_options_for_3d(viewport_size),
        );
    }
}
