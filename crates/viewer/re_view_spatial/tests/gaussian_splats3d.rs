//! Renders a handful of gaussian splats, to cover the gaussian splat visualizer.
//!
//! The full `.ply` → `re_importer` → `GaussianSplats3D` path is covered end-to-end by
//! `crates/store/re_importer/tests/test_ply_importer.rs`.

use re_log_types::TimePoint;
use re_sdk_types::components::RotationAxisAngle;
use re_sdk_types::datatypes::Angle;
use re_sdk_types::{
    RowId,
    archetypes::{GaussianSplats3D, InstancePoses3D},
    datatypes::Quaternion,
};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::OsThreshold;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::ViewClass as _;
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_gaussian_splats3d() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    let gaussians = GaussianSplats3D::new([(0.0, 0.0, 0.0), (2.0, 0.0, 0.0), (4.0, 0.0, 0.0)])
        .with_scales([(1.0, 0.5, 0.25), (0.5, 1.0, 0.5), (0.25, 0.5, 1.0)])
        .with_quaternions([
            Quaternion::IDENTITY,
            Quaternion::from_xyzw([0.0, 0.0, 0.382_683, 0.923_880]), // 45 degrees around Z
            Quaternion::IDENTITY,
        ])
        .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF]);

    test_context.log_entity("world/gaussians", |builder| {
        builder.with_archetype(RowId::new(), TimePoint::default(), &gaussians)
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView3D::identifier(),
        ))
    });

    let size = egui::vec2(400.0, 400.0);

    // Alpha-blending overlapping gaussians accumulates in a GPU-dependent order, so the exact
    // per-pixel color differs slightly between the CI rasterizer and a real GPU. The committed
    // snapshot is the CI render; a small per-pixel tolerance covers local GPUs.
    let default_options = re_ui::testing::default_snapshot_options_for_3d(size);
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(size)
        .with_options(
            default_options.clone().threshold(
                OsThreshold::new(default_options.threshold)
                    .macos(10.0)
                    .windows(10.0),
            ),
        )
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));

    harness.snapshot("gaussian_splats3d");
}

/// Renders a real 3D Gaussian Splatting reconstruction (`cactus.ply`) through the splat renderer,
/// exercising the full loader → archetype → visualizer → renderer path on real data.
#[test]
pub fn test_gaussian_splats3d_cactus() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    let gaussians = GaussianSplats3D::from_ply_file_path(&re_test_context::asset_path(
        "gaussian_splats/cactus.ply",
    ))
    .expect("failed to load cactus.ply");

    test_context.log_entity("world/cactus", |builder| {
        builder.with_archetype(RowId::new(), TimePoint::default(), &gaussians)
    });

    // Several non-identity instance transforms: this exercises instancing (one draw per pose) as
    // well as the covariance-through-`world_from_obj` path in the vertex shader, for rotation and
    // for scale.
    test_context.log_entity("world/cactus", |builder| {
        let poses = InstancePoses3D::new()
            .with_translations([[-1.6, 0.0, 0.0], [0.0, 0.0, 0.0], [1.6, 0.0, 0.0]])
            .with_rotation_axis_angles([
                RotationAxisAngle::new([0.0, 1.0, 0.0], Angle::from_degrees(0.0)),
                RotationAxisAngle::new([0.0, 1.0, 0.0], Angle::from_degrees(45.0)),
                RotationAxisAngle::new([0.0, 0.0, 1.0], Angle::from_degrees(90.0)),
            ])
            .with_scales([1.0, 1.0, 0.5]);
        builder.with_archetype(RowId::new(), TimePoint::default(), &poses)
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView3D::identifier(),
        ))
    });

    let size = egui::vec2(400.0, 400.0);

    // A dense splat scene alpha-blends thousands of overlapping gaussians, so the accumulation
    // order (and thus the exact per-pixel color) is not stable: two runs on the same CI machine
    // differ by up to ~2.5 in kittest's squared-YIQ metric, and a real GPU differs from the CI
    // rasterizer by up to ~260 (out of a possible ~35000). The committed snapshot is the CI
    // render, so we keep the default budget of differing pixels and instead allow a larger
    // per-pixel difference — much larger on macOS and Windows, where we also run on real GPUs.
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(size)
        .with_options(
            re_ui::testing::default_snapshot_options_for_3d(size)
                .threshold(OsThreshold::new(10.0).macos(300.0).windows(300.0)),
        )
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));

    harness.snapshot("gaussian_splats3d_cactus");
}
