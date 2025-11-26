//! Test that 2D content can be added to a 3D space and vice versa.

use re_log_types::{EntityPathFilter, TimePoint};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_types::{archetypes, components};
use re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

fn setup_scene(test_context: &mut TestContext, use_explicit_frames: bool) {
    use ndarray::{Array, ShapeBuilder as _};

    let eye_position = glam::vec3(0.0, -1.0, 0.2);
    let camera_extrincis = archetypes::Transform3D::from_mat3x3(
        // Look at the middle box.
        glam::Mat3::look_at_rh(eye_position, glam::vec3(0.0, 1.0, 0.0), glam::Vec3::Z),
    )
    .with_translation(eye_position);
    let camera_intrinsics =
        archetypes::Pinhole::from_focal_length_and_resolution([2., 2.], [3., 2.])
            .with_image_plane_distance(1.0);
    let camera_image = archetypes::Image::from_color_model_and_tensor(
        re_types::datatypes::ColorModel::RGB,
        Array::<u8, _>::zeros((2, 3, 3).f()),
    )
    .expect("failed to create image");

    let points2d =
        archetypes::Points2D::new([[0.0, 0.0], [3.0, 0.0], [0.0, 2.0], [3.0, 2.0], [1.5, 1.0]])
            .with_radii([0.2]);

    let boxes = archetypes::Boxes3D::from_centers_and_half_sizes(
        [(-1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (1.0, 1.0, 0.0)],
        [(0.2, 0.4, 0.2), (0.2, 0.2, 0.4), (0.4, 0.2, 0.2)],
    )
    .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF])
    .with_fill_mode(components::FillMode::Solid);

    if use_explicit_frames {
        // ROS style frame ids, flat entity hierarchy.
        let root_frame = components::TransformFrameId::new("tf#/");

        test_context.log_entity("boxes", |builder| {
            builder
                .with_archetype_auto_row(TimePoint::STATIC, &boxes)
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::CoordinateFrame::new(root_frame.clone()),
                )
        });
        test_context.log_entity("points", |builder| {
            builder
                .with_archetype_auto_row(TimePoint::STATIC, &points2d)
                .with_archetype_auto_row(TimePoint::STATIC, &archetypes::CoordinateFrame::new("2D"))
        });
        test_context.log_entity("image", |builder| {
            builder
                .with_archetype_auto_row(TimePoint::STATIC, &camera_image)
                .with_archetype_auto_row(TimePoint::STATIC, &archetypes::CoordinateFrame::new("2D"))
        });
        test_context.log_entity("camera", |builder| {
            builder
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &camera_extrincis
                        .with_parent_frame(root_frame.clone())
                        .with_child_frame("camera"),
                )
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &camera_intrinsics
                        .with_parent_frame("camera")
                        .with_child_frame("2D"),
                )
                // TODO(RR-2997): The pinhole should show without this just fine. But space origin can only be an entity, so we rely on pinhole having a coordinate frame that we can pick up.
                .with_archetype_auto_row(TimePoint::STATIC, &archetypes::CoordinateFrame::new("2D"))
        });
    } else {
        // Classic Rerun hierarchy.
        test_context.log_entity("boxes", |builder| {
            builder.with_archetype_auto_row(TimePoint::STATIC, &boxes)
        });

        test_context.log_entity("camera", |builder| {
            builder
                .with_archetype_auto_row(TimePoint::STATIC, &camera_extrincis)
                .with_archetype_auto_row(TimePoint::STATIC, &camera_intrinsics)
                .with_archetype_auto_row(TimePoint::STATIC, &camera_image)
        });

        test_context.log_entity("camera/points", |builder| {
            builder.with_archetype_auto_row(TimePoint::STATIC, &points2d)
        });
    }
}

fn test_2d_in_3d(use_explicit_frames: bool) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    setup_scene(&mut test_context, use_explicit_frames);

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());
        blueprint.add_view_at_root(view)
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_3d([400.0, 300.0])
        .build_ui(|ui| {
            test_context.run_ui(ui, |ctx, ui| {
                test_context.ui_for_single_view(ui, ctx, view_id);
            });
        });

    harness.run();

    // Should produce the same images, but easier to deal with test failures if it's separate.
    let name = if use_explicit_frames {
        "2d_in_3d_with_explicit_frames"
    } else {
        "2d_in_3d"
    };

    harness.snapshot(name);
}

#[test]
fn test_2d_in_3d_with_explicit_frames() {
    test_2d_in_3d(true);
}

#[test]
fn test_2d_in_3d_without_explicit_frames() {
    test_2d_in_3d(false);
}

fn test_3d_in_2d(use_explicit_frames: bool) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();

    setup_scene(&mut test_context, use_explicit_frames);

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view = ViewBlueprint::new(
            re_view_spatial::SpatialView2D::identifier(),
            RecommendedView {
                origin: "camera".into(),
                query_filter: EntityPathFilter::all(),
            },
        );
        blueprint.add_view_at_root(view)
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_3d([400.0, 300.0])
        .build_ui(|ui| {
            test_context.run_ui(ui, |ctx, ui| {
                test_context.ui_for_single_view(ui, ctx, view_id);
            });
        });

    // Should produce the same images, but easier to deal with test failures if it's separate.
    let name = if use_explicit_frames {
        "3d_in_2d_with_explicit_frames"
    } else {
        "3d_in_2d"
    };

    harness.run();
    harness.snapshot(name);
}

#[test]
fn test_3d_in_2d_with_explicit_frames() {
    test_3d_in_2d(true);
}

#[test]
fn test_3d_in_2d_without_explicit_frames() {
    test_3d_in_2d(false);
}
