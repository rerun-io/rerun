#![expect(clippy::disallowed_methods)] // It's a test, it's fine to hardcode a color!

use glam::Vec3;
use re_log_types::TimePoint;
use re_renderer::Color32;
use re_sdk_types::blueprint::archetypes::EyeControls3D;
use re_sdk_types::components::{FillMode, Position3D};
use re_sdk_types::{AsComponents, RowId, archetypes};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::OsThreshold;
use re_test_viewport::TestContextExt as _;
use re_view_spatial::SpatialView3D;
use re_viewer_context::{BlueprintContext as _, RecommendedView, ViewClass as _};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

fn test_transparent_geometry<A: AsComponents>(
    name: &str,
    archetype_builder: impl Fn(f32, Color32) -> A,
) {
    let mut test_context = TestContext::new_with_view_class::<SpatialView3D>();

    // Log a bunch of transparent meshes.
    for (i, color) in [
        Color32::from_rgba_unmultiplied(255, 128, 128, 20),
        Color32::from_rgba_unmultiplied(128, 255, 128, 20),
        Color32::from_rgba_unmultiplied(128, 128, 255, 20),
    ]
    .into_iter()
    .enumerate()
    {
        let y = i as f32 * 2.0 - 2.0;
        test_context.log_entity(format!("geom_{i}"), |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &archetype_builder(y, color),
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

        view_id
    });

    let size = egui::vec2(300.0, 300.0);

    let default_options = re_ui::testing::default_snapshot_options_for_3d(size);
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(size)
        .with_options(
            re_ui::testing::default_snapshot_options_for_3d(size)
                // Transparency rendering on MacOS diverges significantly from the other platforms.
                // (not just on CI but also locally)
                .threshold(OsThreshold::new(default_options.threshold).macos(2.5))
                .failed_pixel_count_threshold(
                    OsThreshold::new(default_options.failed_pixel_count_threshold).macos(150),
                ),
        )
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    for (i, orientation_y) in [-1.0, 1.0].into_iter().enumerate() {
        // Flip the camera orientation to ensure sorting works as expected.

        test_context.with_blueprint_ctx(|ctx, _| {
            let eye_property = ViewProperty::from_archetype::<EyeControls3D>(
                ctx.current_blueprint(),
                ctx.blueprint_query(),
                view_id,
            );

            let len = 3.5;
            let dir = Vec3::new(0.25, orientation_y, 0.25).normalize();
            let position = len * dir;

            eye_property.save_blueprint_component(
                &ctx,
                &EyeControls3D::descriptor_position(),
                &Position3D::from(position),
            );
        });
        // Write blueprint by handling system commands.
        test_context.handle_system_commands(&harness.ctx);

        harness.run_steps(1);
        harness.snapshot(format!("transparent_{name}_{i}"));
    }
}

#[test]
pub fn test_transparent_mesh() {
    test_transparent_geometry("mesh", |y, color| {
        // Use thetrahedrons rather than something flat since they have a front & back, making them a lot more interesting.
        archetypes::Mesh3D::new([
            [0.0, y + 1.0, 0.0],
            [-1.0, y - 1.0, -1.0],
            [1.0, y - 1.0, -1.0],
            [0.0, y - 1.0, 1.0],
        ])
        .with_triangle_indices([[0, 1, 2], [0, 1, 3], [0, 2, 3], [1, 2, 3]])
        .with_albedo_factor(color)
    });
}

#[test]
pub fn test_transparent_boxes3d() {
    test_transparent_geometry("boxes3d", |y, color| {
        archetypes::Boxes3D::from_half_sizes([[0.5, 0.5, 0.5]])
            .with_centers([[0.0, y, 0.0]])
            .with_fill_mode(FillMode::Solid)
            .with_colors([color])
    });
}

#[test]
pub fn test_transparent_ellipsoids3d() {
    test_transparent_geometry("ellipsoids3d", |y, color| {
        archetypes::Ellipsoids3D::from_half_sizes([[0.5, 0.5, 0.5]])
            .with_centers([[0.0, y, 0.0]])
            .with_fill_mode(FillMode::Solid)
            .with_colors([color])
    });
}

#[test]
pub fn test_transparent_cylinders3d() {
    test_transparent_geometry("cylinders3d", |y, color| {
        archetypes::Cylinders3D::from_lengths_and_radii([1.0], [0.5])
            .with_centers([[0.0, y, 0.0]])
            .with_fill_mode(FillMode::Solid)
            .with_colors([color])
    });
}

#[test]
pub fn test_transparent_capsules3d() {
    test_transparent_geometry("capsules3d", |y, color| {
        archetypes::Capsules3D::from_lengths_and_radii([1.0], [0.5])
            .with_translations([[0.0, y, 0.0]])
            .with_fill_mode(FillMode::Solid)
            .with_colors([color])
    });
}
