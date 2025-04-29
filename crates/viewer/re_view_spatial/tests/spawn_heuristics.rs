#![expect(clippy::tuple_array_conversions)]
#![expect(clippy::unwrap_used)]

use re_log_types::{EntityPath, Timeline};
use re_types::{archetypes, AsComponents};
use re_viewer_context::{test_context::TestContext, ViewClass as _};

use ndarray::{Array, ShapeBuilder as _};

enum ImageSize {
    Small,
    Medium,
    Large,
}

impl ImageSize {
    fn wh(&self) -> [usize; 2] {
        match self {
            Self::Small => [48, 32],
            Self::Medium => [320, 240],
            Self::Large => [640, 480],
        }
    }
}

enum ImageType {
    Color,
    Depth,
    Segmentation,
}

enum EntityKind {
    Text,
    BBox2D,
    BBox3D,
    ViewCoords,
    Pinhole(ImageSize),
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

        test_context.log_entity(entity_path, |builder| {
            let component = match entity_kind {
                EntityKind::Text => {
                    &archetypes::TextDocument::new("test document") as &dyn AsComponents
                }

                EntityKind::BBox2D => &archetypes::Boxes2D::from_centers_and_half_sizes(
                    [(5.0, 5.0), (10.0, 10.0), (15.0, 15.0)],
                    [(10.0, 10.0), (10.0, 10.0), (10.0, 10.0)],
                ),

                EntityKind::BBox3D => &archetypes::Boxes3D::from_centers_and_half_sizes(
                    [(5.0, 5.0, 5.0)],
                    [(10.0, 10.0, 1.0)],
                ),

                EntityKind::ViewCoords => &archetypes::ViewCoordinates::RIGHT_HAND_Y_DOWN(),

                EntityKind::Pinhole(image_size) => {
                    let [w, h] = image_size.wh();
                    let resolution = [w as f32, h as f32];
                    &archetypes::Pinhole::from_focal_length_and_resolution(resolution, resolution)
                }
                EntityKind::Image(image_type, image_size) => {
                    let [w, h] = image_size.wh();

                    match image_type {
                        ImageType::Color => &archetypes::Image::from_color_model_and_tensor(
                            re_types::datatypes::ColorModel::RGB,
                            Array::<u8, _>::zeros((h, w, 3).f()),
                        )
                        .unwrap() as &dyn AsComponents,
                        ImageType::Depth => {
                            &archetypes::DepthImage::try_from(Array::<u8, _>::zeros((h, w).f()))
                                .unwrap()
                        }
                        ImageType::Segmentation => &archetypes::SegmentationImage::try_from(
                            Array::<u8, _>::zeros((h, w).f()),
                        )
                        .unwrap(),
                    }
                }
            };
            builder.with_archetype(row_id, time, component)
        });
    }

    test_context
}

fn run_heuristics_snapshot_test(name: &str, test_context: &TestContext) {
    let view_class = test_context
        .view_class_registry
        .class(re_view_spatial::SpatialView2D::identifier())
        .unwrap();

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        let excluded_entities = re_log_types::ResolvedEntityPathFilter::properties();
        let include_entity = |ent: &EntityPath| !excluded_entities.matches(ent);
        let recommended_views = view_class.spawn_heuristics(ctx, &include_entity);

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
        ("segmentation/detection/things", EntityKind::BBox2D),
        ("video", EntityKind::Image(Color, Large)),
        ("video/tracked/0", EntityKind::BBox2D),
        ("video/tracked/1", EntityKind::BBox2D),
        ("video/tracked/2", EntityKind::BBox2D),
        // Since we haven't registered the text view, it won't show up in automatically generated views at all.
        // This is just here to mimic an entity the 2D spatial view can't handle at all.
        ("description", EntityKind::Text),
    ]);

    run_heuristics_snapshot_test(
        "detect_and_track_objects_like_scene_2d_view_heuristic",
        &test_context,
    );
}

#[test]
fn test_differing_image_sizes() {
    use {ImageSize::*, ImageType::*};

    let test_context = build_test_scene(&[
        ("image", EntityKind::Image(Color, Large)),
        (
            "image/segmentation",
            EntityKind::Image(Segmentation, Medium),
        ),
    ]);

    run_heuristics_snapshot_test(
        "should_be_two_separate_views_because_differing_sizes",
        &test_context,
    );
}

#[test]
fn test_not_stacking_color_images() {
    use {ImageSize::*, ImageType::*};

    let test_context = build_test_scene(&[
        ("image/a", EntityKind::Image(Color, Medium)),
        ("image/b", EntityKind::Image(Color, Medium)),
    ]);

    run_heuristics_snapshot_test(
        "should_be_two_separate_views_because_we_cant_stack_color_images",
        &test_context,
    );
}

#[test]
fn test_stacking_color_and_seg() {
    use {ImageSize::*, ImageType::*};

    let test_context = build_test_scene(&[
        ("image/color", EntityKind::Image(Color, Medium)),
        ("image/depth", EntityKind::Image(Depth, Medium)),
        ("image/seg", EntityKind::Image(Segmentation, Medium)),
    ]);

    run_heuristics_snapshot_test("should_be_a_single_view", &test_context);
}

#[test]
fn test_mixed_2d_and_3d() {
    use {ImageSize::*, ImageType::*};

    let test_context = build_test_scene(&[
        ("image1", EntityKind::Image(Color, Small)), // should be separate 2D views
        ("image2", EntityKind::Image(Color, Small)), // should be separate 2D views
        ("3D", EntityKind::ViewCoords), // should be a separate 3D view, but NOT a 2D view
        ("3D/box", EntityKind::BBox3D),
        ("3D/camera", EntityKind::Pinhole(Small)),
        ("3D/camera", EntityKind::Image(Color, Small)), // should be a separate 2D view
    ]);

    run_heuristics_snapshot_test("should_be_three_space_views", &test_context);
}

#[test]
fn test_mixed_images() {
    use {ImageSize::*, ImageType::*};

    let test_context = build_test_scene(&[
        ("image1", EntityKind::Image(Color, Small)),
        ("image2", EntityKind::Image(Color, Small)),
        ("image3", EntityKind::Image(Color, Small)),
        ("image3/nested", EntityKind::Image(Color, Small)), // Need to be a separate space view, because we don't overlap color images
        ("segmented/image4", EntityKind::Image(Color, Small)),
        ("segmented/seg", EntityKind::Image(Segmentation, Small)),
    ]);

    run_heuristics_snapshot_test("four_color_views_and_one_segmentation", &test_context);
}
