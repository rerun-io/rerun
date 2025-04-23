#![allow(clippy::unwrap_used)]

use re_log_types::{EntityPath, Timeline};
use re_types::archetypes;
use re_viewer_context::{test_context::TestContext, ViewClass as _};

use ndarray::{Array, ShapeBuilder as _};

#[allow(dead_code)] // TODO(emilk): expand tests
enum ImageSize {
    Small,
    Medium,
    Large,
}

#[allow(dead_code)] // TODO(emilk): expand tests
enum ImageType {
    Color,
    Depth,
    Segmentation,
}

enum EntityKind {
    Text,
    BoundingBoxes,
    Image(ImageType, ImageSize),
}

fn build_test_scene(entities: &[(&'static str, EntityKind)]) -> TestContext {
    let mut test_context = TestContext::default();
    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<re_view_spatial::SpatialView2D>();

    let timeline_step = Timeline::new_sequence("step");
    let time = [(timeline_step, 1)];

    for (entity_path, entity_kind) in entities {
        let entity_path = EntityPath::from(*entity_path);
        let row_id = re_types::RowId::new();

        test_context.log_entity(entity_path, |builder| match entity_kind {
            EntityKind::Text => builder.with_archetype(
                row_id,
                time,
                &archetypes::TextDocument::new("test document"),
            ),
            EntityKind::BoundingBoxes => builder.with_archetype(
                row_id,
                time,
                &archetypes::Boxes2D::from_centers_and_half_sizes(
                    [(5.0, 5.0), (10.0, 10.0), (15.0, 15.0)],
                    [(10.0, 10.0), (10.0, 10.0), (10.0, 10.0)],
                ),
            ),
            EntityKind::Image(image_type, image_size) => {
                let (w, h) = match image_size {
                    ImageSize::Small => (48, 32),
                    ImageSize::Medium => (320, 240),
                    ImageSize::Large => (640, 480),
                };

                match image_type {
                    ImageType::Color => builder.with_archetype(
                        row_id,
                        time,
                        &archetypes::Image::from_color_model_and_tensor(
                            re_types::datatypes::ColorModel::RGB,
                            Array::<u8, _>::zeros((h, w, 3).f()),
                        )
                        .unwrap(),
                    ),
                    ImageType::Depth => builder.with_archetype(
                        row_id,
                        time,
                        &archetypes::DepthImage::try_from(Array::<u8, _>::zeros((h, w).f()))
                            .unwrap(),
                    ),
                    ImageType::Segmentation => builder.with_archetype(
                        row_id,
                        time,
                        &archetypes::SegmentationImage::try_from(Array::<u8, _>::zeros((h, w).f()))
                            .unwrap(),
                    ),
                }
            }
        });
    }

    test_context
}

fn run_herustics_snapshot_test(name: &str, test_context: &TestContext) {
    let view_class = test_context
        .view_class_registry
        .class(re_view_spatial::SpatialView2D::identifier())
        .unwrap();

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        let recommended_views =
            view_class.spawn_heuristics(ctx, &re_log_types::ResolvedEntityPathFilter::properties());

        insta::assert_debug_snapshot!(name, recommended_views);
    });
}

#[test]
fn test_spatial_view_2d_spawn_heuristics_like_detect_and_track_objects() {
    use {ImageSize::*, ImageType::*};

    // Creates A 2D scene that mimics the `detect_and_track_objects` example.
    let test_context = build_test_scene(&[
        ("segmentation/rgb_scaled", EntityKind::Image(Color, Medium)),
        ("segmentation", EntityKind::Image(Segmentation, Medium)),
        ("segmentation/detection/things", EntityKind::BoundingBoxes),
        ("video", EntityKind::Image(Color, Large)),
        ("video/tracked/0", EntityKind::BoundingBoxes),
        ("video/tracked/1", EntityKind::BoundingBoxes),
        ("video/tracked/2", EntityKind::BoundingBoxes),
        // Since we haven't registered the text view, it won't show up in automatically generated views at all.
        // This is just here to mimic an entity the 2D spatial view can't handle at all.
        ("description", EntityKind::Text),
    ]);

    run_herustics_snapshot_test(
        "detect_and_track_objects_like_scene_2d_view_heuristic",
        &test_context,
    );
}
