use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::archetypes::Points2D;
use re_sdk_types::components::Radius;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_view_spatial::SpatialView2D;
use re_viewer_context::ViewClass as _;
use re_viewport_blueprint::ViewBlueprint;

const SNAPSHOT_SIZE: egui::Vec2 = egui::vec2(400.0, 300.0);

fn snapshot_default_framing(name: &str, points: &Points2D, snapshot_results: &mut SnapshotResults) {
    let mut test_context = TestContext::new_with_view_class::<SpatialView2D>();

    test_context.log_entity("points", |builder| {
        builder.with_archetype(RowId::new(), TimePoint::STATIC, points)
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            SpatialView2D::identifier(),
        ))
    });

    snapshot_results.add(test_context.run_view_ui_and_save_renderer_snapshot(
        view_id,
        name,
        SNAPSHOT_SIZE,
        None,
    ));
}

#[test]
fn test_colinear_points_scene_radius() {
    let mut snapshot_results = SnapshotResults::new();

    let points = Points2D::new([[0.0, 0.0], [100.0, 0.0]])
        .with_radii([Radius::new_scene_units(10.0), Radius::new_scene_units(10.0)]);
    snapshot_default_framing(
        "points2d_default_framing_colinear_points",
        &points,
        &mut snapshot_results,
    );
}

#[test]
fn test_single_point_scene_radius() {
    let mut snapshot_results = SnapshotResults::new();

    let points = Points2D::new([[0.0, 0.0]]).with_radii([Radius::new_scene_units(200.0)]);
    snapshot_default_framing(
        "points2d_default_framing_single_point",
        &points,
        &mut snapshot_results,
    );
}
