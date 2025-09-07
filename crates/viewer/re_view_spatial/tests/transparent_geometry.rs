#![expect(clippy::disallowed_methods)] // It's a test, it's fine to hardcode a color!

use re_log_types::TimePoint;
use re_renderer::Color32;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_types::{AsComponents, RowId, archetypes, components::FillMode};
use re_view_spatial::{SpatialView3D, SpatialViewState, ViewEye};
use re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

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

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view_blueprint =
            ViewBlueprint::new(SpatialView3D::identifier(), RecommendedView::root());
        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);

        view_id
    });

    let size = egui::vec2(300.0, 300.0);

    let camera_orientation = std::sync::Arc::new(std::sync::atomic::AtomicI32::new(1));

    let mut harness = {
        let camera_orientation = camera_orientation.clone();
        test_context
            .setup_kittest_for_rendering()
            .with_size(size)
            .build_ui(move |ui| {
                // TODO(#8265): Could simplify this a lot of we could set the camera in blueprint.
                {
                    let mut view_states = test_context.view_states.lock();
                    let view_class = test_context
                        .view_class_registry
                        .get_class_or_log_error(SpatialView3D::identifier());
                    let view_state = view_states.get_mut_or_create(view_id, view_class);

                    let view_state: &mut SpatialViewState = view_state
                        .as_any_mut()
                        .downcast_mut::<SpatialViewState>()
                        .expect("view state is not of correct type");

                    let orientation =
                        camera_orientation.load(std::sync::atomic::Ordering::Acquire) as f32;
                    view_state.state_3d.view_eye = Some(ViewEye::new_orbital(
                        glam::Vec3::ZERO,
                        3.5,
                        glam::Quat::from_affine3(
                            &glam::Affine3A::look_at_rh(
                                glam::vec3(0.25, orientation, 0.25),
                                glam::Vec3::ZERO,
                                glam::Vec3::Z,
                            )
                            .inverse(),
                        ),
                        glam::Vec3::Z,
                    ));
                    view_state.state_3d.last_eye_interaction = Some(std::time::Instant::now());
                }

                test_context.run_with_single_view(ui, view_id);
            })
    };

    for i in 0..2 {
        // Flip the camera orientation to ensure sorting works as expected.
        camera_orientation.store(1 - i * 2, std::sync::atomic::Ordering::Release);
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
