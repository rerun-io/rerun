//! Test that 2D content can be added to a 3D space and vice versa.

use re_log_types::{EntityPath, EntityPathFilter, TimePoint};
use re_sdk_types::components::RotationAxisAngle;
use re_sdk_types::datatypes::Angle;
use re_sdk_types::{archetypes, blueprint::archetypes as blueprint_archetypes, components};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

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
    let camera_image = {
        let height = 2;
        let width = 3;
        let mut data = Array::<u8, _>::zeros((height, width, 3).f());

        // Create a colored checkerboard pattern
        for y in 0..height {
            for x in 0..width {
                let is_even_square = (x + y) % 2 == 0;
                let color = if is_even_square {
                    [255, 100, 100] // Light red
                } else {
                    [100, 100, 255] // Light blue
                };
                data[[y, x, 0]] = color[0];
                data[[y, x, 1]] = color[1];
                data[[y, x, 2]] = color[2];
            }
        }

        archetypes::Image::from_color_model_and_tensor(
            re_sdk_types::datatypes::ColorModel::RGB,
            data,
        )
        .expect("failed to create image")
    };

    let points2d =
        archetypes::Points2D::new([[0.0, 0.0], [3.0, 0.0], [0.0, 2.0], [3.0, 2.0], [1.5, 1.0]])
            .with_radii([0.2]);

    let boxes = archetypes::Boxes3D::from_centers_and_half_sizes(
        [(-1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (1.0, 1.0, 0.0)],
        [(0.2, 0.4, 0.2), (0.2, 0.2, 0.4), (0.4, 0.2, 0.2)],
    )
    .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF])
    .with_fill_mode(components::FillMode::Solid);

    let origin_transform = archetypes::Transform3D::from_rotation(RotationAxisAngle::new(
        glam::Vec3::Z,
        Angle::from_degrees(30.0),
    ));

    if use_explicit_frames {
        // ROS style frame ids, flat entity hierarchy with indirection at the origin.
        let root_frame = components::TransformFrameId::new("tf#/");
        let origin_frame = components::TransformFrameId::new("origin");

        // Add indirection: origin -> world
        test_context.log_entity("origin", |builder| {
            builder
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &origin_transform
                        .with_parent_frame(root_frame.clone())
                        .with_child_frame(origin_frame.clone()),
                )
                // Make sure that if we set the origin entity to this, we get the right transform.
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::CoordinateFrame::new(origin_frame.clone()),
                )
        });

        test_context.log_entity("boxes", |builder| {
            builder
                .with_archetype_auto_row(TimePoint::STATIC, &boxes)
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::CoordinateFrame::new(origin_frame.clone()),
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
                        .with_parent_frame(origin_frame.clone())
                        .with_child_frame("camera"),
                )
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &camera_intrinsics
                        .with_parent_frame("camera")
                        .with_child_frame("2D"),
                )
        });
    } else {
        // Classic Rerun hierarchy with indirection at the origin.
        test_context.log_entity("origin", |builder| {
            builder.with_archetype_auto_row(TimePoint::STATIC, &origin_transform)
        });

        test_context.log_entity("origin/boxes", |builder| {
            builder.with_archetype_auto_row(TimePoint::STATIC, &boxes)
        });

        test_context.log_entity("origin/camera", |builder| {
            builder
                .with_archetype_auto_row(TimePoint::STATIC, &camera_extrincis)
                .with_archetype_auto_row(TimePoint::STATIC, &camera_intrinsics)
                .with_archetype_auto_row(TimePoint::STATIC, &camera_image)
        });

        test_context.log_entity("origin/camera/points", |builder| {
            builder.with_archetype_auto_row(TimePoint::STATIC, &points2d)
        });
    }
}

fn test_2d_in_3d(use_named_frames: bool, origin: EntityPath) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    setup_scene(&mut test_context, use_named_frames);

    // Named vs non-named should produce the same images, but easier to deal with test failures if it's separate.
    let origin_name = if origin.is_root() {
        "root".to_owned()
    } else {
        origin
            .to_string()
            .replace('/', "_")
            .trim_matches('_')
            .to_owned()
    };
    let name = if use_named_frames {
        format!("2d_in_3d_with_explicit_frames_at_{origin_name}")
    } else {
        format!("2d_in_3d_at_{origin_name}")
    };

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView {
                origin,
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

    harness.run();

    harness.snapshot(name);
}

#[test]
fn test_2d_in_3d_at_root_with_explicit_frames() {
    test_2d_in_3d(true, EntityPath::root());
}

#[test]
fn test_2d_in_3d_at_root_without_explicit_frames() {
    test_2d_in_3d(false, EntityPath::root());
}

#[test]
fn test_2d_in_3d_at_subpath_with_explicit_frames() {
    test_2d_in_3d(true, EntityPath::from("origin"));
}

#[test]
fn test_2d_in_3d_at_subpath_without_explicit_frames() {
    test_2d_in_3d(false, EntityPath::from("origin"));
}

fn test_3d_in_2d(use_explicit_frames: bool) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();

    setup_scene(&mut test_context, use_explicit_frames);

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new(
            re_view_spatial::SpatialView2D::identifier(),
            RecommendedView {
                origin: "origin/camera".into(),
                query_filter: EntityPathFilter::all(),
            },
        );

        // TODO(RR-3076): We don't correctly pick up the target frame from a pinhole origin without coordinate frame.
        // But we also want to remove origin in the future. Either way it's a matter of better target frame heuristic.
        if use_explicit_frames {
            ViewProperty::from_archetype::<blueprint_archetypes::SpatialInformation>(
                ctx.blueprint_db(),
                ctx.blueprint_query,
                view.id,
            )
            .save_blueprint_component(
                ctx,
                &blueprint_archetypes::SpatialInformation::descriptor_target_frame(),
                &re_tf::TransformFrameId::new("2D"),
            );
        }

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
