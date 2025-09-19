#![expect(clippy::unwrap_used)] // It's a test!

use std::cell::Cell;

use image::GenericImageView;
use itertools::Itertools as _;
use re_chunk_store::RowId;
use re_log_types::{NonMinI64, TimeInt, TimePoint};
use re_test_context::{
    TestContext,
    external::egui_kittest::{OsThreshold, SnapshotOptions},
};
use re_test_viewport::TestContextExt as _;
use re_types::{
    archetypes::{AssetVideo, VideoFrameReference, VideoStream},
    components::{self, MediaType, VideoTimestamp},
    datatypes,
};
use re_video::{VideoCodec, VideoDataDescription};
use re_viewer_context::{ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

fn workspace_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf()
}

#[test]
fn test_bgr_image() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();

    let test_image = image::load_from_memory(include_bytes!("assets/grinda.jpg")).unwrap();
    let size = test_image.dimensions();
    let rgb_u8 = test_image.to_rgb8().into_raw();
    let bgr_u8 = rgb_u8
        .chunks(3)
        .flat_map(|p| [p[2], p[1], p[0]])
        .collect_vec();
    let bgra_u8 = rgb_u8
        .chunks(3)
        .flat_map(|p| [p[2], p[1], p[0], 255])
        .collect_vec();

    test_context.log_entity("2d_layering/middle_blue", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::Image::from_elements(
                &bgr_u8,
                size.into(),
                re_types::datatypes::ColorModel::BGR,
            ),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView2D::identifier(),
        ))
    });

    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "bgr_image",
        egui::vec2(160.0, 120.0),
    );
}

fn run_view_ui_and_save_snapshot(
    test_context: &mut TestContext,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();
    harness.snapshot_options(
        name,
        &SnapshotOptions::new().failed_pixel_count_threshold(OsThreshold::new(0).macos(40)),
    );
}
