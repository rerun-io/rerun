//! Test that pressing the Escape key will back out of dialogs etc.

use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_viewer_context::ViewClass as _;
use re_viewer::external::{re_sdk_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_test_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::vec2(1024.0, 1024.0)),
        ..Default::default()
    });
    harness.init_recording();
    harness.set_selection_panel_opened(false);

    // Log some data
    harness.log_entity("boxes3d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                [(0.0, 0.0, 0.0)],
                [(1.0, 0.4, 0.2)],
            )
            .with_colors([0xFF9001FF]),
        )
    });

    harness.clear_current_blueprint();
    harness.setup_viewport_blueprint(move |_viewer_context, blueprint| {
        let mut view3d =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());
        view3d.display_name = Some("3D view".into());
        blueprint.add_view_at_root(view3d);
    });

    harness
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_deselect_on_escape() {
    let mut harness = make_test_harness();

    // Select view then press escape, view should be deselected
    harness.blueprint_tree().click_label("3D view");
    harness.key_press(egui::Key::Escape);
    harness.snapshot_app("deselect_on_escape_1");

    // Select view, click on dropdown, then press escape, view remains selected
    harness.blueprint_tree().click_label("3D view");
    harness.set_selection_panel_opened(true);
    harness.click_label("GradientDark");
    harness.snapshot_app("deselect_on_escape_2");
    harness.key_press(egui::Key::Escape);
    harness.snapshot_app("deselect_on_escape_3");

    // Select view, open "add view or container" modal, escape, view remains selected
    harness.blueprint_tree().click_label("3D view");
    harness.click_label("Open menu with more options");
    harness.click_label_contains("Add view or container");
    harness.snapshot_app("deselect_on_escape_4");
    harness.key_press(egui::Key::Escape);
    harness.snapshot_app("deselect_on_escape_5");

    // Select view, right click on blueprint tree, escape, view remains selected
    harness.blueprint_tree().click_label("3D view");
    harness.blueprint_tree().right_click_label("3D view");
    harness.snapshot_app("deselect_on_escape_6");
    harness.key_press(egui::Key::Escape);
    harness.snapshot_app("deselect_on_escape_7");
    // Whatever was selected should become deselected after pressing escape again
    harness.key_press(egui::Key::Escape);
    harness.set_selection_panel_opened(false);
    harness.snapshot_app("deselect_on_escape_8");
}
