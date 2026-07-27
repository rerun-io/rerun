//! Renders a handful of gaussian splats, to cover the placeholder visualizer.
//!
//! The full `.ply` → `re_importer` → `GaussianSplats3D` path is covered end-to-end by
//! `crates/store/re_importer/tests/test_ply_importer.rs`.

use re_log_types::TimePoint;
use re_sdk_types::{RowId, archetypes::GaussianSplats3D, datatypes::Quaternion};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{RecommendedView, ViewClass as _};
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
        let view_blueprint = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView::root(),
        );

        let view_id = view_blueprint.id;

        blueprint.add_views(std::iter::once(view_blueprint), None, None);

        view_id
    });

    let size = egui::vec2(400.0, 400.0);

    let mut harness = test_context
        .setup_kittest_for_rendering_3d(size)
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));

    harness.snapshot("gaussian_splats3d");
}
