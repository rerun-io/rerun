#![expect(clippy::disallowed_methods)] // It's a test, it's fine to hardcode values!

use re_log_types::TimePoint;
use re_renderer::Color32;
use re_test_context::{TestContext, external::egui_kittest::SnapshotOptions};
use re_test_viewport::TestContextExt as _;
use re_types::{
    RowId, Rotation3D, archetypes,
    blueprint::archetypes::EyeControls3D,
    components::{FillMode, Position3D},
    datatypes::{Angle, RotationAxisAngle, Vec3D},
};
use re_view_spatial::SpatialView3D;
use re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

/// Test that boxes with rotation transforms correctly fall back to the slow path.
/// This verifies the fix for the issue where the fast path was ignoring rotations.
#[test]
pub fn test_boxes3d_with_rotation() {
    let mut test_context = TestContext::new_with_view_class::<SpatialView3D>();

    // Log boxes with different rotation angles to verify transform handling
    for (i, angle_deg) in [0.0, 45.0, 90.0].into_iter().enumerate() {
        let y = i as f32 * 2.5 - 2.5;

        // Create rotation around Z axis
        let rotation = RotationAxisAngle::new(
            Vec3D([0.0, 0.0, 1.0]),
            Angle::from_degrees(angle_deg),
        );

        test_context.log_entity(format!("boxes/rotated_{i}"), |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &archetypes::Boxes3D::from_half_sizes([[1.0, 0.3, 0.3]])
                    .with_centers([[0.0, y, 0.0]])
                    .with_rotation_axis_angles([rotation])
                    .with_fill_mode(FillMode::Solid)
                    .with_colors([Color32::from_rgba_unmultiplied(
                        255,
                        (128 + i * 40) as u8,
                        128,
                        255,
                    )]),
            )
        });
    }

    // Add some boxes without rotation for comparison (should use fast path if count > threshold)
    for i in 0..3 {
        let x = i as f32 * 2.5 - 2.5;
        test_context.log_entity(format!("boxes/no_rotation_{i}"), |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &archetypes::Boxes3D::from_half_sizes([[0.3, 0.3, 0.3]])
                    .with_centers([[x, -4.0, 0.0]])
                    .with_fill_mode(FillMode::Solid)
                    .with_colors([Color32::from_rgba_unmultiplied(128, 128, 255, 255)]),
            )
        });
    }

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_blueprint =
            ViewBlueprint::new(SpatialView3D::identifier(), RecommendedView::root());
        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);

        let eye_property = ViewProperty::from_archetype::<EyeControls3D>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            view_id,
        );

        eye_property.save_blueprint_component(
            ctx,
            &EyeControls3D::descriptor_look_target(),
            &Position3D::ZERO,
        );

        // Position camera to see all boxes
        eye_property.save_blueprint_component(
            ctx,
            &EyeControls3D::descriptor_position(),
            &Position3D::from([8.0, 0.0, 8.0]),
        );

        view_id
    });

    let size = egui::vec2(400.0, 400.0);

    let mut harness = test_context
        .setup_kittest_for_rendering_3d(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run_steps(5);
    harness.snapshot_options(
        "boxes3d_with_rotation",
        &SnapshotOptions::default()
            .threshold(2.0)
            .failed_pixel_count_threshold(5),
    );
}

/// Test boxes with non-uniform scaling transforms.
#[test]
pub fn test_boxes3d_with_scaling() {
    let mut test_context = TestContext::new_with_view_class::<SpatialView3D>();

    // Create boxes with rotations AND non-uniform scaling by logging transforms separately
    // This tests the complete transform path
    for (i, (angle_deg, scale)) in [(0.0, [1.0, 1.0, 1.0]), (45.0, [2.0, 1.0, 0.5]), (90.0, [0.5, 2.0, 1.0])]
        .into_iter()
        .enumerate()
    {
        let y = i as f32 * 3.0 - 3.0;
        let entity_path = format!("boxes/scaled_{i}");

        // Log transform with rotation and scaling
        let rotation = RotationAxisAngle::new(
            Vec3D([0.0, 0.0, 1.0]),
            Angle::from_degrees(angle_deg),
        );
        test_context.log_entity(entity_path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &archetypes::Transform3D::from_translation_rotation_scale(
                    Vec3D([0.0, y, 0.0]),
                    Rotation3D::AxisAngle(rotation.into()),
                    Vec3D(scale),
                ),
            )
        });

        // Log the box
        test_context.log_entity(entity_path, |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &archetypes::Boxes3D::from_half_sizes([[0.5, 0.5, 0.5]])
                    .with_fill_mode(FillMode::Solid)
                    .with_colors([Color32::from_rgba_unmultiplied(
                        (200 - i * 50) as u8,
                        200,
                        (128 + i * 40) as u8,
                        255,
                    )]),
            )
        });
    }

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_blueprint =
            ViewBlueprint::new(SpatialView3D::identifier(), RecommendedView::root());
        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);

        let eye_property = ViewProperty::from_archetype::<EyeControls3D>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            view_id,
        );

        eye_property.save_blueprint_component(
            ctx,
            &EyeControls3D::descriptor_look_target(),
            &Position3D::ZERO,
        );

        eye_property.save_blueprint_component(
            ctx,
            &EyeControls3D::descriptor_position(),
            &Position3D::from([10.0, 0.0, 10.0]),
        );

        view_id
    });

    let size = egui::vec2(400.0, 400.0);

    let mut harness = test_context
        .setup_kittest_for_rendering_3d(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run_steps(5);
    harness.snapshot_options(
        "boxes3d_with_scaling",
        &SnapshotOptions::default()
            .threshold(2.0)
            .failed_pixel_count_threshold(5),
    );
}
