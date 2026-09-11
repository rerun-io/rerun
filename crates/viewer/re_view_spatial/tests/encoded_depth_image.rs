#![expect(clippy::unwrap_used)] // It's a test!

use re_chunk_store::RowId;
use re_sdk_types::archetypes::EncodedDepthImage;
use re_sdk_types::components::MediaType;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{TimeControlCommand, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

/// A 64x48 gray F32 TIFF holding a horizontal depth ramp from 0 to 4 meters.
fn tiff_depth_ramp() -> Vec<u8> {
    let (width, height) = (64_u32, 48_u32);
    let pixels: Vec<f32> = (0..height)
        .flat_map(|_| (0..width).map(move |x| 4.0 * x as f32 / (width - 1) as f32))
        .collect();

    let mut cursor = std::io::Cursor::new(Vec::new());
    tiff::encoder::TiffEncoder::new(&mut cursor)
        .unwrap()
        .write_image::<tiff::encoder::colortype::Gray32Float>(width, height, &pixels)
        .unwrap();
    cursor.into_inner()
}

#[test]
fn test_encoded_depth_image_tiff() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();
    let timeline = test_context
        .active_timeline()
        .expect("should have an active timeline");

    test_context.log_entity("depth", |builder| {
        builder.with_archetype(
            RowId::new(),
            [(timeline, 0_i64)],
            &EncodedDepthImage::new(tiff_depth_ramp())
                .with_media_type(MediaType::tiff())
                .with_meter(1.0),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView2D::identifier(),
        ))
    });

    // Decoding runs on a separate thread, so give it real time instead of busy looping.
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

    test_context.send_time_commands(
        test_context.active_store_id(),
        [
            TimeControlCommand::SetActiveTimeline(*timeline.name()),
            TimeControlCommand::SetTime(0.into()),
        ],
    );

    harness.try_run_realtime().unwrap();
    harness.snapshot_options(
        "encoded_depth_image_tiff",
        // The ramp has hard color edges that hardware and software rasterizers place a
        // pixel apart, shifting up to ~920 pixels between llvmpipe (CI) and Metal.
        &re_ui::testing::default_snapshot_options_for_3d(viewport_size).max_failed_pixels(1200),
    );
}
