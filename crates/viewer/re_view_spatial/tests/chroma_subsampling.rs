use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::datatypes::PixelFormat;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::ViewClass as _;
use re_viewport_blueprint::ViewBlueprint;

// `logo_dark_mode.png` is 152x32.
const IMAGE_SIZE: [u32; 2] = [152, 32];

fn run_chroma_test(
    bytes: &[u8],
    pixel_format: PixelFormat,
    snapshot_name: &str,
    snapshot_results: &mut SnapshotResults,
) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();
    test_context.log_entity("image", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Image::from_pixel_format(
                IMAGE_SIZE,
                pixel_format,
                bytes.to_vec(),
            ),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView2D::identifier());
        blueprint.add_view_at_root(view)
    });

    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        snapshot_name,
        egui::vec2(380.0, 80.0),
        None,
    ));
}

#[test]
fn test_chroma_subsampling() {
    let mut snapshot_results = SnapshotResults::new();

    run_chroma_test(
        include_bytes!("../../../../tests/assets/image/logo_dark_mode_y_u_v24_limited.bin"),
        PixelFormat::Y_U_V24_LimitedRange,
        "chroma_y_u_v24_limited",
        &mut snapshot_results,
    );
    run_chroma_test(
        include_bytes!("../../../../tests/assets/image/logo_dark_mode_y_u_v24_full.bin"),
        PixelFormat::Y_U_V24_FullRange,
        "chroma_y_u_v24_full",
        &mut snapshot_results,
    );

    run_chroma_test(
        include_bytes!("../../../../tests/assets/image/logo_dark_mode_y_u_v16_limited.bin"),
        PixelFormat::Y_U_V16_LimitedRange,
        "chroma_y_u_v16_limited",
        &mut snapshot_results,
    );
    run_chroma_test(
        include_bytes!("../../../../tests/assets/image/logo_dark_mode_y_u_v16_full.bin"),
        PixelFormat::Y_U_V16_FullRange,
        "chroma_y_u_v16_full",
        &mut snapshot_results,
    );

    run_chroma_test(
        include_bytes!("../../../../tests/assets/image/logo_dark_mode_y_u_v12_limited.bin"),
        PixelFormat::Y_U_V12_LimitedRange,
        "chroma_y_u_v12_limited",
        &mut snapshot_results,
    );
    run_chroma_test(
        include_bytes!("../../../../tests/assets/image/logo_dark_mode_y_u_v12_full.bin"),
        PixelFormat::Y_U_V12_FullRange,
        "chroma_y_u_v12_full",
        &mut snapshot_results,
    );

    run_chroma_test(
        include_bytes!("../../../../tests/assets/image/logo_dark_mode_y8_limited.bin"),
        PixelFormat::Y8_LimitedRange,
        "chroma_y8_limited",
        &mut snapshot_results,
    );
    run_chroma_test(
        include_bytes!("../../../../tests/assets/image/logo_dark_mode_y8_full.bin"),
        PixelFormat::Y8_FullRange,
        "chroma_y8_full",
        &mut snapshot_results,
    );

    run_chroma_test(
        include_bytes!("../../../../tests/assets/image/logo_dark_mode_nv12.bin"),
        PixelFormat::NV12,
        "chroma_nv12",
        &mut snapshot_results,
    );
    run_chroma_test(
        include_bytes!("../../../../tests/assets/image/logo_dark_mode_yuy2.bin"),
        PixelFormat::YUY2,
        "chroma_yuy2",
        &mut snapshot_results,
    );
}
