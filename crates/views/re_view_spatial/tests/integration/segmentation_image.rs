use ndarray::{Array, ShapeBuilder as _, s};
use re_chunk_store::RowId;
use re_log_types::{TimeInt, TimePoint, Timeline};
use re_sdk_types::encodings::Rgba32;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::ViewClass as _;
use re_viewport_blueprint::ViewBlueprint;

/// Regression test for transparent annotation classes in segmentation images.
///
/// Transparent classes (alpha=0) should show through to layers below.
/// Previously, this only worked if the segmentation image's overall opacity was != 1.0.
#[test]
pub fn test_segmentation_image_transparency() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();

    let (width, height) = (12, 8);
    let timeline = Timeline::new_sequence("frame");
    test_context.set_active_timeline(*timeline.name());
    let frame = |sequence: i64| {
        TimePoint::default().with(
            timeline,
            TimeInt::from_sequence(sequence.try_into().expect("unexpected min value")),
        )
    };

    // Class 0 stays transparent while the visible colors change with the annotation context.
    test_context.log_entity("/", |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(1),
            &re_sdk_types::archetypes::AnnotationContext::new([
                (0u16, "nothing", Rgba32::from_unmultiplied_rgba(0, 0, 0, 0)),
                (1, "red", Rgba32::from_unmultiplied_rgba(255, 0, 0, 255)),
                (2, "green", Rgba32::from_unmultiplied_rgba(0, 255, 0, 255)),
            ]),
        )
    });
    test_context.log_entity("/", |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(2),
            &re_sdk_types::archetypes::AnnotationContext::new([
                (0u16, "nothing", Rgba32::from_unmultiplied_rgba(0, 0, 0, 0)),
                (1, "cyan", Rgba32::from_unmultiplied_rgba(0, 255, 255, 255)),
                (
                    2,
                    "magenta",
                    Rgba32::from_unmultiplied_rgba(255, 0, 255, 255),
                ),
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
                re_sdk_types::encodings::ColorModel::RGB,
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
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(150.0, 100.0) * 2.0)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    test_context.set_time(1);
    harness.run();
    harness.snapshot("segmentation_image_transparency");

    test_context.set_time(2);
    harness.run();
    harness.snapshot("segmentation_image_transparency_updated_annotation_context");
}

/// A segmentation image without any associated annotation context will still use `class_id` generated colors.
#[test]
fn test_segmentation_image_without_annotations() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();
    test_context.log_entity("segmentation", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::SegmentationImage::try_from(ndarray::arr2(&[
                [0u8, 1],
                [2, 3],
            ]))
            .unwrap(),
        )
    });
    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView2D::identifier(),
        ))
    });
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(200.0, 200.0))
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    harness.run();
    harness.snapshot("segmentation_image_without_annotations");
}

// Regression test for https://github.com/rerun-io/rerun/issues/12939
#[test]
fn test_segmentation_image_class_ids_beyond_128() {
    use re_sdk_types::archetypes::{AnnotationContext, SegmentationImage};

    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();
    // IDs around half the padded colormap range distinguish division by N from division by N - 1.
    let classes = [
        (0u16, "black", [0, 0, 0]),
        (1, "red", [255, 0, 0]),
        (2, "yellow", [255, 255, 0]),
        (64, "light_blue", [0, 128, 255]),
        (126, "light_green", [128, 255, 0]),
        (127, "magenta", [255, 0, 255]),
        (128, "blue", [0, 0, 255]),
        (129, "gray", [128, 128, 128]),
        (130, "orange", [255, 128, 0]),
        (131, "purple", [128, 0, 128]),
        (132, "green", [0, 255, 0]),
    ];
    test_context.log_entity("segmentation_mask", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &AnnotationContext::new(classes.iter().map(|&(id, name, [r, g, b])| {
                (id, name, Rgba32::from_unmultiplied_rgba(r, g, b, 255))
            })),
        )
    });

    let mask = ndarray::Array2::from_shape_fn((1, classes.len()), |(_, x)| classes[x].0 as u8);
    test_context.log_entity("segmentation_mask", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &SegmentationImage::try_from(mask).unwrap().with_opacity(1.0),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView2D::identifier(),
        ))
    });
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(88.0, 24.0))
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    harness.run();
    harness.snapshot("segmentation_image_class_ids_beyond_128");
}
