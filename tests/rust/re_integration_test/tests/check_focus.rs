use egui::accesskit::Role;
use egui_kittest::SnapshotOptions;
use egui_kittest::kittest::Queryable;
use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::external::re_log_types::EntityPathFilter;
use re_sdk::log::RowId;
use re_viewer::external::re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewer::external::{re_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_test_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();
    harness.set_selection_panel_opened(true);

    // Log some data
    harness.log_entity("group/boxes3d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                [(-1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (1.0, 1.0, 0.0)],
                [(0.2, 0.4, 0.2), (0.2, 0.2, 0.4), (0.4, 0.2, 0.2)],
            )
            .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF])
            .with_fill_mode(re_types::components::FillMode::Solid),
        )
    });
    harness
}

fn setup_single_view_blueprint(harness: &mut egui_kittest::Harness<'_, re_viewer::App>) {
    harness.clear_current_blueprint();

    let mut view_1 =
        ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());
    view_1.display_name = Some("3D view 1".into());

    harness.setup_viewport_blueprint(|_viewer_context, blueprint| {
        blueprint.add_view_at_root(view_1);
    });
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_foo() {
    let mut harness = make_test_harness();
    harness
        .state_mut()
        .app_options_mut()
        .show_picking_debug_overlay = true;

    setup_single_view_blueprint(&mut harness);

    // Zoom out a bit to see every box
    harness.event(egui::Event::Zoom(0.5));

    // One of the boxes is a bit left to the center
    let pixel_of_a_box = harness.get_panel_position("3D view 1").center() + egui::vec2(-30.0, 0.0);

    // Click on the view panel widget

    // harness.blueprint_tree().drag_label("3D view 2");
    harness.run_ok();
    harness.run_ok();
    harness.run_ok();
    harness.hover_at(pixel_of_a_box);

    harness.run_ok();
    harness.hover_at(pixel_of_a_box);
    // 3D picking only works if we actually render the app
    harness.render().expect("Cannot render app");
    // harness.run_ok();

    // harness.run_ok();
    // harness.run_ok();
    // harness.run_ok();
    // // harness.streams_tree().hover_label("group/");
    // std::thread::sleep(std::time::Duration::from_millis(1000));
    // harness.click_at(pixel_of_a_box);
    // harness.click_at(pixel_of_a_box);
    // harness.click_at(pixel_of_a_box);
    // harness.click_at(pixel_of_a_box);
    // harness.click_at(pixel_of_a_box);
    // harness.click_at(pixel_of_a_box);
    // harness.click_at(pixel_of_a_box);
    // harness.click_at(pixel_of_a_box);
    // harness.click_at(pixel_of_a_box);
    // harness.click_at(pixel_of_a_box);
    // harness.click_at(pixel_of_a_box);
    // harness.click_at(pixel_of_a_box);
    // harness.click_at(pixel_of_a_box);
    // harness.click_at(pixel_of_a_box);
    // harness.click_at(pixel_of_a_box);

    harness.snapshot_app("xtemp");

    // harness.click_label("Expand all");
    // harness.snapshot_app("blueprint_tree_context_menu_03");

    // harness
    //     .blueprint_tree()
    //     .right_click_label("Viewport (Grid container)");
    // harness.snapshot_app("blueprint_tree_context_menu_04");

    // harness.key_press(egui::Key::Escape);
    // harness.snapshot_app("blueprint_tree_context_menu_05");

    // harness.blueprint_tree().right_click_label("Test view");
    // harness.snapshot_app("blueprint_tree_context_menu_06");

    // harness.key_press(egui::Key::Escape);
    // harness.snapshot_app("blueprint_tree_context_menu_07");

    // harness.blueprint_tree().right_click_label("group");
    // harness.snapshot_app("blueprint_tree_context_menu_08");

    // harness.key_press(egui::Key::Escape);
    // harness.snapshot_app("blueprint_tree_context_menu_09");

    // harness.blueprint_tree().right_click_label("boxes3d");
    // harness.snapshot_app("blueprint_tree_context_menu_10");
}
