use ndarray::{Array, ShapeBuilder as _, s};
use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::datatypes::Rgba32;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

/// Regression test for transparent annotation classes in segmentation images.
///
/// Transparent classes (alpha=0) should show through to layers below.
/// Previously, this only worked if the segmentation image's overall opacity was != 1.0.
#[test]
pub fn test_segmentation_image_transparency() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();

    let (width, height) = (12, 8);

    // Log annotation context: class 0 is fully transparent, 1 is red, 2 is green.
    test_context.log_entity("/", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::AnnotationContext::new([
                (0u16, "nothing", Rgba32::from_unmultiplied_rgba(0, 0, 0, 0)), // fully transparent
                (1, "red", Rgba32::from_unmultiplied_rgba(255, 0, 0, 255)),
                (2, "green", Rgba32::from_unmultiplied_rgba(0, 255, 0, 255)),
            ]),
        )
    });

    // Log a blue background image.
    test_context.log_entity("background", |builder| {
        let mut image = Array::<u8, _>::zeros((height, width, 3).f());
        image.slice_mut(s![.., .., 2]).fill(255); // blue
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Image::from_color_model_and_tensor(
                re_sdk_types::datatypes::ColorModel::RGB,
                image,
            )
            .unwrap(),
        )
    });

    // Log a segmentation image on top:
    // top-left quadrant = class 1 (red), bottom-right quadrant = class 2 (green), rest = class 0 (transparent).
    test_context.log_entity("segmentation", |builder| {
        let mut segmentation = Array::<u8, _>::zeros((height, width).f());
        segmentation.slice_mut(s![0..4, 0..6]).fill(1);
        segmentation.slice_mut(s![4..8, 6..12]).fill(2);
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::SegmentationImage::try_from(segmentation)
                .unwrap()
                .with_opacity(1.0), // Make sure opacity is 1.0, otherwise heuristics will set this to something lower.
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView2D::identifier(),
        ))
    });

    // The transparent class 0 regions should show the blue background through.
    run_view_ui_and_save_snapshot(
        &test_context,
        view_id,
        "segmentation_image_transparency",
        egui::vec2(150.0, 100.0) * 2.0,
    );
}

fn run_view_ui_and_save_snapshot(
    test_context: &TestContext,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();
    harness.snapshot(name);
}
