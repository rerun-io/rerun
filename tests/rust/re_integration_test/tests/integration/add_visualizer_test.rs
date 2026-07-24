use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_viewer_context::ViewClass as _;
use re_viewer::external::{re_sdk_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

/// Test adding an additional visualizer (`TransformAxes3D`) through the UI.
///
/// This test verifies that:
/// 1. An entity with `Transform3D` (but no `TransformAxes3D`) can be visualized with boxes
/// 2. The `TransformAxes3D` visualizer can be manually added via the UI
/// 3. After adding the visualizer, transform axes appear in the 3D view
#[tokio::test(flavor = "multi_thread")]
pub async fn test_add_visualizer_axes() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::Vec2::new(1024.0, 1024.0)),
        max_steps: Some(100),
        ..Default::default()
    });
    harness.init_recording();
    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(true);

    // Log Boxes3D with a Transform3D (but no TransformAxes3D)
    harness.log_entity("boxes3d", |builder| {
        builder
            .with_archetype(
                RowId::new(),
                TimePoint::STATIC,
                &re_sdk_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                    [(0.0, 0.0, 0.0), (2.0, 0.0, 0.0)],
                    [(0.5, 0.5, 0.5), (0.3, 0.3, 0.3)],
                )
                .with_colors([0xFF0000FF, 0x00FF00FF]),
            )
            .with_archetype(
                RowId::new(),
                TimePoint::STATIC,
                &re_sdk_types::archetypes::Transform3D::from_translation([2.0, 0.0, 0.0]),
            )
    });

    // Set up a 3D view
    harness.clear_current_blueprint();
    harness.setup_viewport_blueprint(|_viewer_context, blueprint| {
        let mut view_3d =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());
        view_3d.display_name = Some("3D view".to_owned());
        blueprint.add_view_at_root(view_3d);
    });

    harness.snapshot_app("add_visualizer_axes_1");

    harness.blueprint_tree().right_click_label("3D view");
    harness.click_label("Expand all");
    harness.snapshot_app("add_visualizer_axes_2");

    harness.blueprint_tree().click_label("boxes3d");
    harness.snapshot_app("add_visualizer_axes_3");

    harness.selection_panel().click_label("Add new visualizer…");
    harness.run();
    harness.snapshot_app("add_visualizer_axes_4");

    harness.click_label("TransformAxes3D");
    harness.run();
    harness.snapshot_app("add_visualizer_axes_5");
}
