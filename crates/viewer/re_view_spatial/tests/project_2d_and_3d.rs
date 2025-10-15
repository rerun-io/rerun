//! Test that 2D content can be added to a 3D space and vice versa.

use re_log_types::{EntityPathFilter, TimePoint};
use re_test_context::{TestContext, external::egui_kittest::SnapshotOptions};
use re_test_viewport::TestContextExt as _;
use re_types::{RowId, archetypes, components};
use re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

fn setup_scene(test_context: &mut TestContext) {
    use ndarray::{Array, ShapeBuilder as _};

    test_context.log_entity("boxes", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &archetypes::Boxes3D::from_centers_and_half_sizes(
                [(-1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (1.0, 1.0, 0.0)],
                [(0.2, 0.4, 0.2), (0.2, 0.2, 0.4), (0.4, 0.2, 0.2)],
            )
            .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF])
            .with_fill_mode(components::FillMode::Solid),
        )
    });

    let eye_position = glam::vec3(0.0, -1.0, 0.2);

    test_context.log_entity("camera", |builder| {
        builder
            .with_archetype(
                RowId::new(),
                TimePoint::default(),
                &archetypes::Transform3D::from_mat3x3(
                    // Look at the middle box.
                    glam::Mat3::look_at_rh(eye_position, glam::vec3(0.0, 1.0, 0.0), glam::Vec3::Z),
                )
                .with_translation(eye_position),
            )
            .with_archetype(
                RowId::new(),
                TimePoint::default(),
                &archetypes::Pinhole::from_focal_length_and_resolution([2., 2.], [3., 2.])
                    .with_image_plane_distance(1.0),
            )
            .with_archetype(
                RowId::new(),
                TimePoint::default(),
                &archetypes::Image::from_color_model_and_tensor(
                    re_types::datatypes::ColorModel::RGB,
                    Array::<u8, _>::zeros((2, 3, 3).f()),
                )
                .expect("failed to create image"),
            )
    });
    test_context.log_entity("camera/points", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &archetypes::Points2D::new([
                [0.0, 0.0],
                [3.0, 0.0],
                [0.0, 2.0],
                [3.0, 2.0],
                [1.5, 1.0],
            ])
            .with_radii([0.2]),
        )
    });
}

#[test]
pub fn test_2d_in_3d() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    setup_scene(&mut test_context);

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
    harness.snapshot_options(
        "2d_in_3d",
        &SnapshotOptions::new().failed_pixel_count_threshold(4),
    );
}

#[test]
pub fn test_3d_in_2d() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();

    setup_scene(&mut test_context);

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

    harness.run();
    harness.snapshot("3d_in_2d");
}
