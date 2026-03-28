//! Tests that coplanar pinhole images are rendered in the correct order based on their draw order property.

use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::archetypes::{Image, Pinhole, Transform3D};
use re_sdk_types::blueprint::archetypes::EyeControls3D;
use re_sdk_types::components::Position3D;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{BlueprintContext as _, ViewClass as _, ViewId};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

/// Helper struct to specify the properties of a pinhole image to be rendered in the test scene.
struct PinholeImageSpec<'a> {
    entity_path: &'a str,
    translation: [f32; 3],
    image_color: [u8; 3],
    opacity: f32,
    draw_order: f32,
}

#[test]
fn test_pinhole_draw_order_black_above_white() {
    run_pinhole_snapshot(
        "pinhole_draw_order_black_above_white",
        &[
            PinholeImageSpec {
                entity_path: "world/black_cam",
                translation: [0.0, 0.0, 0.0],
                image_color: [0, 0, 0],
                opacity: 1.0,
                draw_order: 1.0,
            },
            PinholeImageSpec {
                entity_path: "world/white_cam",
                translation: [0.15, 0.15, 0.0],
                image_color: [255, 255, 255],
                opacity: 1.0,
                draw_order: 0.0,
            },
        ],
    );
}

#[test]
fn test_pinhole_draw_order_white_above_black() {
    run_pinhole_snapshot(
        "pinhole_draw_order_white_above_black",
        &[
            PinholeImageSpec {
                entity_path: "world/black_cam",
                translation: [0.0, 0.0, 0.0],
                image_color: [0, 0, 0],
                opacity: 1.0,
                draw_order: 0.0,
            },
            PinholeImageSpec {
                entity_path: "world/white_cam",
                translation: [0.15, 0.15, 0.0],
                image_color: [255, 255, 255],
                opacity: 1.0,
                draw_order: 1.0,
            },
        ],
    );
}

#[test]
fn test_pinhole_draw_order_black_above_white_transparent() {
    run_pinhole_snapshot(
        "pinhole_draw_order_black_above_white_transparent",
        &[
            PinholeImageSpec {
                entity_path: "world/black_cam",
                translation: [0.0, 0.0, 0.0],
                image_color: [0, 0, 0],
                opacity: 0.6,
                draw_order: 1.0,
            },
            PinholeImageSpec {
                entity_path: "world/white_cam",
                translation: [0.15, 0.15, 0.0],
                image_color: [255, 255, 255],
                opacity: 0.6,
                draw_order: 0.0,
            },
        ],
    );
}

#[test]
fn test_pinhole_draw_order_white_above_black_transparent() {
    run_pinhole_snapshot(
        "pinhole_draw_order_white_above_black_transparent",
        &[
            PinholeImageSpec {
                entity_path: "world/black_cam",
                translation: [0.0, 0.0, 0.0],
                image_color: [0, 0, 0],
                opacity: 0.6,
                draw_order: 0.0,
            },
            PinholeImageSpec {
                entity_path: "world/white_cam",
                translation: [0.15, 0.15, 0.0],
                image_color: [255, 255, 255],
                opacity: 0.6,
                draw_order: 1.0,
            },
        ],
    );
}

#[test]
fn test_pinhole_draw_order_sandwiched_opaque_red() {
    run_pinhole_snapshot(
        "pinhole_draw_order_sandwiched_opaque_red",
        &[
            PinholeImageSpec {
                entity_path: "world/black_cam",
                translation: [0.0, 0.0, 0.0],
                image_color: [0, 0, 0],
                opacity: 0.6,
                draw_order: 0.0,
            },
            PinholeImageSpec {
                entity_path: "world/red_cam",
                translation: [0.15, 0.15, 0.0],
                image_color: [255, 0, 0],
                opacity: 1.0,
                draw_order: 0.5,
            },
            PinholeImageSpec {
                entity_path: "world/white_cam",
                translation: [0.3, 0.3, 0.0],
                image_color: [255, 255, 255],
                opacity: 0.6,
                draw_order: 1.0,
            },
        ],
    );
}

fn run_pinhole_snapshot(name: &str, image_planes: &[PinholeImageSpec<'_>]) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();
    setup_scene(&mut test_context, image_planes);

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());
        blueprint.add_view_at_root(view)
    });

    run_view_ui_and_save_snapshot(&test_context, view_id, name, egui::vec2(300.0, 300.0));
}

fn setup_scene(test_context: &mut TestContext, image_planes: &[PinholeImageSpec<'_>]) {
    let width = 64;
    let height = 64;
    let focal_length = [64.0, 64.0];
    let resolution = [width as f32, height as f32];

    let pinhole = Pinhole::from_focal_length_and_resolution(focal_length, resolution)
        .with_image_plane_distance(1.);

    for image_plane in image_planes {
        let image = solid_rgb_image(width, height, image_plane.image_color)
            .with_draw_order(image_plane.draw_order)
            .with_opacity(image_plane.opacity);

        test_context.log_entity(image_plane.entity_path, |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &Transform3D::from_translation(image_plane.translation),
                )
                .with_archetype(RowId::new(), TimePoint::default(), &pinhole)
                .with_archetype(RowId::new(), TimePoint::default(), &image)
        });
    }
}

fn solid_rgb_image(width: usize, height: usize, color: [u8; 3]) -> Image {
    use ndarray::{Array, ShapeBuilder as _};

    let mut image = Array::<u8, _>::zeros((height, width, 3).f());
    for (channel, value) in color.iter().enumerate() {
        image.index_axis_mut(ndarray::Axis(2), channel).fill(*value);
    }

    Image::from_color_model_and_tensor(re_sdk_types::datatypes::ColorModel::RGB, image)
        .expect("failed to create test image")
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

    test_context.with_blueprint_ctx(|ctx, _| {
        let property = ViewProperty::from_archetype::<EyeControls3D>(
            ctx.current_blueprint(),
            ctx.blueprint_query(),
            view_id,
        );

        property.save_blueprint_component(
            &ctx,
            &EyeControls3D::descriptor_position(),
            &Position3D::new(2.0, 2.0, 2.0),
        );
    });

    harness.run_steps(10);
    harness.snapshot(name);
}
