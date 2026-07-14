use re_log_types::TimePoint;
use re_sdk_types::RowId;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_dae_mesh_import() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    // Get the path to the DAE test file
    let workspace_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_dir = workspace_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap();
    let dae_path = workspace_dir.join("tests/assets/mesh/box.dae");

    // Log the DAE mesh as an Asset3D
    test_context.log_entity("world/mesh", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Asset3D::from_file_path(&dae_path).unwrap(),
        )
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

    harness.snapshot("dae_mesh_import");
}

/// Verify that a DAE geometry containing multiple `<triangles>` primitives is
/// fully loaded and rendered (not just the first group).
///
#[test]
pub fn test_dae_multi_triangle_groups() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    let workspace_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_dir = workspace_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap();
    let dae_path = workspace_dir.join("tests/assets/mesh/multi_triangle_groups.dae");

    test_context.log_entity("world/mesh", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Asset3D::from_file_path(&dae_path).unwrap(),
        )
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

    harness.snapshot("dae_multi_triangle_groups");
}
