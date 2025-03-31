use re_log_types::Timeline;
use re_types::archetypes;
use re_viewer_context::{test_context::TestContext, ViewClass as _};

use ndarray::{Array, ShapeBuilder as _};

// Creates a 2D scene that mimics the `detect_and_track_objects` example.
#[allow(clippy::unwrap_used)]
fn build_2d_scene(test_context: &mut TestContext) {
    let timeline_step = Timeline::new_sequence("step");
    let time = [(timeline_step, 1)];

    const IMAGE_WIDTH: usize = 64;
    const IMAGE_HEIGHT: usize = 48;

    test_context.log_entity("segmentation/rgb_scaled".into(), |builder| {
        builder.with_archetype(
            re_types::RowId::new(),
            time,
            &archetypes::Image::from_color_model_and_tensor(
                re_types::datatypes::ColorModel::RGB,
                Array::<u8, _>::zeros((IMAGE_HEIGHT, IMAGE_WIDTH, 3).f()),
            )
            .unwrap(),
        )
    });
    test_context.log_entity("segmentation".into(), |builder| {
        builder.with_archetype(
            re_types::RowId::new(),
            time,
            &archetypes::SegmentationImage::try_from(Array::<u8, _>::zeros(
                (IMAGE_HEIGHT, IMAGE_WIDTH).f(),
            ))
            .unwrap(),
        )
    });
    test_context.log_entity("segmentation/detections/things".into(), |builder| {
        builder.with_archetype(
            re_types::RowId::new(),
            time,
            &archetypes::Boxes2D::from_centers_and_half_sizes(
                [(5.0, 5.0), (10.0, 10.0), (15.0, 15.0)],
                [(10.0, 10.0), (10.0, 10.0), (10.0, 10.0)],
            ),
        )
    });

    test_context.log_entity("video".into(), |builder| {
        builder
            .with_archetype(
                re_types::RowId::new(),
                time,
                &archetypes::AssetVideo::from_file_path("../../../tests/assets/empty.mp4").unwrap(),
            )
            .with_archetype(
                re_types::RowId::new(),
                time,
                &archetypes::VideoFrameReference::new(0),
            )
    });

    // Since we haven't registered the text view, it won't show up in automatically generated views at all.
    // This is just here to mimic an entity the 2D spatial view can't handle at all.
    test_context.log_entity("description".into(), |builder| {
        builder.with_archetype(
            re_types::RowId::new(),
            re_log_types::TimePoint::default(),
            &archetypes::TextDocument::new("test document"),
        )
    });
}

#[test]
fn test_spatial_view_2d_spawn_heuristics() {
    let mut test_context = TestContext::default();
    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<re_view_spatial::SpatialView2D>();

    build_2d_scene(&mut test_context);

    let view_class = test_context
        .view_class_registry
        .get_class_or_log_error(re_view_spatial::SpatialView2D::identifier());

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        let recommended_views =
            view_class.spawn_heuristics(ctx, &re_log_types::ResolvedEntityPathFilter::properties());

        insta::assert_debug_snapshot!(
            "detect_and_track_objects_like_scene_2d_view_heuristic",
            recommended_views
        );
    });
}
