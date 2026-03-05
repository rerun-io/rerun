use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::components::MagnificationFilter;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::ViewClass as _;
use re_viewport_blueprint::ViewBlueprint;

fn run_magnification_filter_test(
    filter: MagnificationFilter,
    snapshot_results: &mut SnapshotResults,
) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();

    // Create a small 4x4 checkerboard image that will be magnified,
    // making the filter differences clearly visible.
    let size: [u32; 2] = [4, 4];
    let mut pixels = Vec::with_capacity((size[0] * size[1] * 3) as usize);
    for y in 0..size[1] {
        for x in 0..size[0] {
            if (x + y) % 2 == 0 {
                pixels.extend_from_slice(&[255, 0, 0]); // Red
            } else {
                pixels.extend_from_slice(&[0, 0, 255]); // Blue
            }
        }
    }

    test_context.log_entity("image", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Image::from_elements(&pixels, size, re_sdk_types::datatypes::ColorModel::RGB)
                .with_magnification_filter(filter),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView2D::identifier(),
        ))
    });

    let snapshot_name = format!("magnification_filter_{}", filter.to_string().to_lowercase());

    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        &snapshot_name,
        egui::vec2(200.0, 200.0),
        None,
    ));
}

fn run_depth_image_magnification_filter_test(
    filter: MagnificationFilter,
    snapshot_results: &mut SnapshotResults,
) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();

    // Create a small 4x4 checkerboard depth image that will be magnified.
    let size: [u32; 2] = [4, 4];
    let mut pixels = Vec::with_capacity((size[0] * size[1]) as usize);
    for y in 0..size[1] {
        for x in 0..size[0] {
            if (x + y) % 2 == 0 {
                pixels.push(0u16);
            } else {
                pixels.push(u16::MAX);
            }
        }
    }

    test_context.log_entity("depth", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::DepthImage::from_gray16(
                bytemuck::cast_slice::<u16, u8>(&pixels),
                size,
            )
            .with_magnification_filter(filter),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView2D::identifier(),
        ))
    });

    let snapshot_name = format!(
        "magnification_filter_depth_image_{}",
        filter.to_string().to_lowercase()
    );

    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        &snapshot_name,
        egui::vec2(200.0, 200.0),
        None,
    ));
}

#[test]
fn test_magnification_filters() {
    let mut snapshot_results = SnapshotResults::new();

    run_magnification_filter_test(MagnificationFilter::Nearest, &mut snapshot_results);
    run_magnification_filter_test(MagnificationFilter::Linear, &mut snapshot_results);
    run_magnification_filter_test(MagnificationFilter::Bicubic, &mut snapshot_results);
}

#[test]
fn test_depth_image_magnification_filters() {
    let mut snapshot_results = SnapshotResults::new();

    run_depth_image_magnification_filter_test(MagnificationFilter::Nearest, &mut snapshot_results);
    run_depth_image_magnification_filter_test(MagnificationFilter::Linear, &mut snapshot_results);
    run_depth_image_magnification_filter_test(MagnificationFilter::Bicubic, &mut snapshot_results);
}
