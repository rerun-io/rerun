//! Tests for the source component dropdown in the visualizer UI.
//!
//! This test verifies that:
//! 1. The source component dropdown is visible in the selection panel for entities with visualizers
//! 2. Users can change the source component mapping through the UI
//! 3. The changes are correctly persisted in the blueprint

use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_viewer_context::ViewClass as _;
use re_viewer::external::{re_sdk_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::Vec2::new(1200.0, 800.0)),
        max_steps: Some(10),
        ..Default::default()
    });
    harness.init_recording();
    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(true);

    // Log 3D points with colors and radii - similar to add_visualizer_test
    harness.log_entity("points3d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::Points3D::new([
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (0.0, 1.0, 0.0),
                (1.0, 1.0, 0.0),
            ])
            .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF])
            .with_radii([0.1, 0.15, 0.2, 0.25]),
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

    harness
}

/// Test that the source component dropdown is visible and can be interacted with.
#[tokio::test(flavor = "multi_thread")]
pub async fn test_source_component_dropdown() {
    let mut harness = make_harness();

    // Take initial snapshot
    harness.snapshot_app("source_component_1");

    // Expand the view in the blueprint tree
    harness.blueprint_tree().right_click_label("3D view");
    harness.click_label("Expand all");
    harness.snapshot_app("source_component_2");

    // Click on the points3d entity to select it
    // After selection, the Visualizers section should already be expanded showing components
    harness.blueprint_tree().click_label("points3d");
    harness.snapshot_app("source_component_3");

    harness
        .selection_panel()
        .toggle_nth_hierarchical_list("radii", 1);
    harness.snapshot_app("source_component_4");

    // Now we should see the "Source" dropdown showing "Points3D:radii"
    harness
        .selection_panel()
        .click_label("Points3D:radii_$source");
    harness.snapshot_app("source_component_5");

    // Select "View Default" from the dropdown options to avoid confusion with the "Override" RadioButton
    harness.click_label("View Default");
    harness.snapshot_app("source_component_6");
}
