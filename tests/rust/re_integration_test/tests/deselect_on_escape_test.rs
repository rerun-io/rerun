use egui::Modifiers;

use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_viewer_context::ViewClass as _;
use re_viewer::external::{re_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_test_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::vec2(1024.0, 1024.0)),
    });
    harness.init_recording();
    harness.set_selection_panel_opened(false);

    // Log some data
    harness.log_entity("boxes3d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::Boxes3D::from_centers_and_half_sizes(
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
pub async fn test_foo() {
    let mut harness = make_test_harness();

    // Click on the view title widget
    harness.click_nth_label("3D view", 1);
    harness.key_press(egui::Key::Escape);

    harness.click_nth_label("3D view", 1);
    harness.set_selection_panel_opened(true);

    harness.click_label("GradientDark");

    harness.snapshot_app("xtemp");

    //     // Test context menus of view panel title widgets
    //     harness.right_click_nth_label("3D view", 1);
    //     harness.hover_label_contains("Move to new container");
    //     harness.snapshot_app("context_menu_invalid_sub_container_02");
    //     harness.key_press(egui::Key::Escape);

    //     harness.right_click_nth_label("2D view", 1);
    //     harness.hover_label_contains("Move to new container");
    //     harness.snapshot_app("context_menu_invalid_sub_container_03");
    //     harness.key_press(egui::Key::Escape);

    //     // Test context menus of view items in the blueprint panel
    //     harness.right_click_nth_label("3D view", 0);
    //     harness.hover_label_contains("Move to new container");
    //     harness.snapshot_app("context_menu_invalid_sub_container_04");
    //     harness.key_press(egui::Key::Escape);

    //     harness.right_click_nth_label("2D view", 0);
    //     harness.hover_label_contains("Move to new container");
    //     harness.snapshot_app("context_menu_invalid_sub_container_05");
    //     harness.key_press(egui::Key::Escape);
}
