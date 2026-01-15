#![expect(clippy::disallowed_methods)] // It's a test, it's fine to hardcode a color!

use re_entity_db::InstancePath;
use re_log_types::TimePoint;
use re_renderer::Color32;
use re_sdk_types::components::FillMode;
use re_sdk_types::{RowId, archetypes};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_view_spatial::SpatialView3D;
use re_viewer_context::{Item, RecommendedView, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

/// Tests selecting box instances from a batch (!) of boxes.
///
/// Note that we could also repeat this test for many other shapes but this would be largely redundant.
///
/// As of writing this is _partially_ true for `transparent_geometry` tests as well.
/// However, the transparency test also tests regular meshes and doesn't take batches as input, so the dataflow is quite different.
#[test]
fn test_select_box_instances() {
    let mut test_context = TestContext::new_with_view_class::<SpatialView3D>();

    test_context.log_entity("shapes", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &archetypes::Boxes3D::from_half_sizes([[0.5, 0.5, 0.5]])
                .with_centers([[0.0, -2.0, 0.0], [0.0, 0.0, 0.0], [0.0, 2.0, 0.0]])
                .with_fill_mode(FillMode::Solid)
                .with_colors([
                    Color32::from_rgba_unmultiplied(255, 128, 128, 255),
                    Color32::from_rgba_unmultiplied(128, 255, 128, 255),
                    Color32::from_rgba_unmultiplied(128, 128, 255, 255),
                ]),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view_blueprint =
            ViewBlueprint::new(SpatialView3D::identifier(), RecommendedView::root());
        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);

        view_id
    });

    let mut snapshot_results = SnapshotResults::new();
    for selected_instance_path in [
        InstancePath::instance("shapes", 0),
        InstancePath::instance("shapes", 1),
        InstancePath::entity_all("shapes"),
    ] {
        // This exaggerates the outlines, making it easier to see & get caught by the snapshot test.
        let ui_scale = 4.0;
        let mut harness = test_context
            .setup_kittest_for_rendering_3d(egui::vec2(300.0, 300.0) / ui_scale)
            .with_pixels_per_point(ui_scale);
        // Have to set options explicitly here, since `setup_kittest_for_rendering_3d` isn't aware of the ui scaling.§
        harness.with_options(re_ui::testing::default_snapshot_options_for_3d(egui::vec2(
            300.0, 300.0,
        )));
        let mut harness = harness.build_ui(|ui| {
            test_context.edit_selection(|selection_state| {
                selection_state.set_selection(Item::InstancePath(selected_instance_path.clone()));
            });
            test_context.run_with_single_view(ui, view_id);
        });

        let name = if selected_instance_path.instance.is_specific() {
            format!(
                "select_box_instances_{}",
                selected_instance_path.instance.get()
            )
        } else {
            "select_box_instances_all".to_owned()
        };
        harness.snapshot(name);
        snapshot_results.extend_harness(&mut harness);
    }
}
