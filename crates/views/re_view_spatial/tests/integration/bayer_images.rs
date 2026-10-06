use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::encodings::PixelFormat;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::ViewClass as _;
use re_viewport_blueprint::ViewBlueprint;

const IMAGE_SIZE: [u32; 2] = [64, 32];

/// Color gradient that the raw Bayer images are made from.
fn gradient_rgb([x, y]: [u32; 2]) -> [u8; 3] {
    [(x * 4) as u8, (y * 8) as u8, (255 - x * 4) as u8]
}

/// Samples [`gradient_rgb`] with the color filter pattern of the given format.
fn bayer_bytes(pixel_format: PixelFormat) -> Vec<u8> {
    let pattern = pixel_format
        .bayer_pattern()
        .expect("test only uses Bayer formats");
    let [w, h] = IMAGE_SIZE;

    let mut bytes = Vec::with_capacity(pixel_format.num_bytes(IMAGE_SIZE));
    for y in 0..h {
        for x in 0..w {
            bytes.push(gradient_rgb([x, y])[pattern.channel_at([x, y])]);
        }
    }
    bytes
}

fn run_bayer_test(
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
                bayer_bytes(pixel_format),
            ),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView2D::identifier());
        blueprint.add_view_at_root(view)
    });

    snapshot_results.add(test_context.run_view_ui_and_save_renderer_snapshot(
        view_id,
        snapshot_name,
        egui::vec2(160.0, 80.0),
        None,
    ));
}

/// Raw Bayer images of the same color gradient, in every supported pattern,
/// are demosaiced into the same RGB image.
#[test]
fn test_bayer_images() {
    let mut snapshot_results = SnapshotResults::new();

    for (pixel_format, snapshot_name) in [
        (PixelFormat::BayerRGGB8, "bayer_rggb8"),
        (PixelFormat::BayerBGGR8, "bayer_bggr8"),
        (PixelFormat::BayerGBRG8, "bayer_gbrg8"),
        (PixelFormat::BayerGRBG8, "bayer_grbg8"),
    ] {
        run_bayer_test(pixel_format, snapshot_name, &mut snapshot_results);
    }
}
