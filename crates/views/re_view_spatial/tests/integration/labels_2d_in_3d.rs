//! Snapshot coverage for 2D geometry labels in 2D and 3D views.
//!
//! A translated and rotated pinhole camera places the geometry in world space.
//! Labels must stay attached to that geometry in 3D and retain their placement in the camera's 2D view.
//! The scene covers points, boxes, ellipses, lines, arrows, and a shared label for multiple boxes.

use re_log_types::{EntityPathFilter, TimePoint};
use re_sdk_types::archetypes;
use re_sdk_types::blueprint::archetypes::{EyeControls3D, SpatialInformation};
use re_sdk_types::components::{Position3D, Vector3D};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

fn setup_scene(test_context: &mut TestContext) {
    test_context.log_entity("camera", |builder| {
        builder
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::Transform3D::from_rotation(
                    glam::Quat::from_rotation_z(0.4)
                        * glam::Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
                )
                .with_translation([1.0, 2.0, 3.0]),
            )
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::Pinhole::from_focal_length_and_resolution(
                    [400.0, 300.0],
                    [600.0, 400.0],
                )
                .with_image_plane_distance(2.0),
            )
    });
    test_context.log_entity("camera/points", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &archetypes::Points2D::new([[100.0, 80.0], [300.0, 80.0]])
                .with_labels(["point A", "point B"])
                .with_show_labels(true),
        )
    });
    test_context.log_entity("camera/boxes", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &archetypes::Boxes2D::from_centers_and_half_sizes([[100.0, 200.0]], [[50.0, 30.0]])
                .with_labels(["box"])
                .with_show_labels(true),
        )
    });
    test_context.log_entity("camera/ellipses", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &archetypes::Ellipses2D::from_half_sizes([[50.0, 30.0]])
                .with_centers([[300.0, 200.0]])
                .with_labels(["ellipse"])
                .with_show_labels(true),
        )
    });
    test_context.log_entity("camera/lines", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &archetypes::LineStrips2D::new([[[50.0, 320.0], [150.0, 320.0]]])
                .with_labels(["line"])
                .with_show_labels(true),
        )
    });
    test_context.log_entity("camera/arrows", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &archetypes::Arrows2D::from_vectors([[100.0, 0.0]])
                .with_origins([[250.0, 320.0]])
                .with_labels(["arrow"])
                .with_show_labels(true),
        )
    });
    test_context.log_entity("camera/shared_box_label", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &archetypes::Boxes2D::from_centers_and_half_sizes(
                [[480.0, 150.0], [480.0, 250.0]],
                [[30.0, 30.0]; 2],
            )
            .with_labels(["shared"])
            .with_show_labels(true),
        )
    });
}

#[test]
fn labels_2d_in_3d() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();
    setup_scene(&mut test_context);
    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_id = blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView3D::identifier(),
        ));
        ViewProperty::from_archetype_for_view::<SpatialInformation>(ctx, view_id)
            .save_blueprint_component(
                ctx,
                &SpatialInformation::descriptor_target_frame(),
                &re_tf::TransformFrameId::new("tf#/"),
            );
        let eye = ViewProperty::from_archetype_for_view::<EyeControls3D>(ctx, view_id);
        eye.save_blueprint_component(
            ctx,
            &EyeControls3D::descriptor_position(),
            &Position3D::new(-0.7, -1.3, 4.5),
        );
        eye.save_blueprint_component(
            ctx,
            &EyeControls3D::descriptor_look_target(),
            &Position3D::new(0.2, 3.8, 3.0),
        );
        eye.save_blueprint_component(
            ctx,
            &EyeControls3D::descriptor_eye_up(),
            &Vector3D::from([0.0, 0.0, 1.0]),
        );
        view_id
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_3d([800.0, 600.0])
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));
    harness.run();
    harness.snapshot("labels_2d_in_3d");
}

#[test]
fn labels_2d_in_2d() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();
    setup_scene(&mut test_context);
    let view_id = test_context.setup_viewport_blueprint(|_, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new(
            re_view_spatial::SpatialView2D::identifier(),
            RecommendedView {
                origin: "camera".into(),
                query_filter: EntityPathFilter::all(),
            },
        ))
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_3d([800.0, 600.0])
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));
    harness.run();
    harness.snapshot("labels_2d_in_2d");
}
