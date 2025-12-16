use egui::Modifiers;
use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewer::external::{re_sdk_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_multi_view_test_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();
    harness.set_selection_panel_opened(false);

    // Log some data
    harness.log_entity("boxes3d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (1.0, 1.0, 0.0)],
                [(0.2, 0.4, 0.2), (0.2, 0.2, 0.4), (0.4, 0.2, 0.2)],
            )
            .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF])
            .with_fill_mode(re_sdk_types::components::FillMode::Solid),
        )
    });
    harness.log_entity("boxes2d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Boxes2D::from_centers_and_half_sizes(
                [(-1.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
                [(0.2, 0.4), (0.2, 0.2), (0.4, 0.2)],
            )
            .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF]),
        )
    });

    // Set up a multi-view blueprint
    harness.clear_current_blueprint();

    let root_cid = harness.add_blueprint_container(egui_tiles::ContainerKind::Grid, None);
    let vertical_cid =
        harness.add_blueprint_container(egui_tiles::ContainerKind::Vertical, Some(root_cid));
    let horizontal_cid =
        harness.add_blueprint_container(egui_tiles::ContainerKind::Horizontal, Some(root_cid));

    let mut view3d = ViewBlueprint::new(
        re_view_spatial::SpatialView3D::identifier(),
        RecommendedView::new_single_entity("boxes3d"),
    );
    view3d.display_name = Some("3D view".to_owned());
    let mut view2d = ViewBlueprint::new(
        re_view_spatial::SpatialView2D::identifier(),
        RecommendedView::new_single_entity("boxes2d"),
    );
    view2d.display_name = Some("2D view".into());

    harness.setup_viewport_blueprint(move |_viewer_context, blueprint| {
        blueprint.add_views(std::iter::once(view3d), Some(vertical_cid), None);
        blueprint.add_views(std::iter::once(view2d), Some(horizontal_cid), None);
    });

    harness
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_context_menu_invalid_sub_container() {
    let mut harness = make_multi_view_test_harness();

    harness.snapshot_app("context_menu_invalid_sub_container_01");

    // Test context menus of view panel title widgets
    harness.right_click_nth_label("3D view", 1);
    harness.hover_label_contains("Move to new container");
    harness.snapshot_app("context_menu_invalid_sub_container_02");
    harness.key_press(egui::Key::Escape);

    harness.right_click_nth_label("2D view", 1);
    harness.hover_label_contains("Move to new container");
    harness.snapshot_app("context_menu_invalid_sub_container_03");
    harness.key_press(egui::Key::Escape);

    // Test context menus of view items in the blueprint panel
    harness.blueprint_tree().right_click_label("3D view");
    harness.hover_label_contains("Move to new container");
    harness.snapshot_app("context_menu_invalid_sub_container_04");
    harness.key_press(egui::Key::Escape);

    harness.blueprint_tree().right_click_label("2D view");
    harness.hover_label_contains("Move to new container");
    harness.snapshot_app("context_menu_invalid_sub_container_05");
    harness.key_press(egui::Key::Escape);
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_context_menu_multi_selection() {
    let mut harness = make_multi_view_test_harness();

    harness.snapshot_app("context_menu_multi_selection_01");

    // Expand both views and the boxes2d entity
    harness.blueprint_tree().right_click_label("3D view");
    harness.click_label("Expand all");
    harness.blueprint_tree().right_click_label("2D view");
    harness.click_label("Expand all");
    harness.streams_tree().right_click_label("boxes2d");
    harness.click_label("Expand all");
    harness.snapshot("context_menu_multi_selection_02");

    // Select 3D View and 2D View, check context menu
    harness.blueprint_tree().click_label("3D view");
    harness
        .blueprint_tree()
        .click_label_modifiers("2D view", Modifiers::COMMAND);
    harness.blueprint_tree().right_click_label("2D view");
    harness.snapshot_app("context_menu_multi_selection_03");
    harness.key_press(egui::Key::Escape);

    // Add container to selection, check context menu
    harness
        .blueprint_tree()
        .click_label_modifiers("Grid container", Modifiers::COMMAND);
    harness.blueprint_tree().right_click_label("2D view");
    harness.snapshot_app("context_menu_multi_selection_04");
    harness.key_press(egui::Key::Escape);

    // Select viewport and check context menu
    harness
        .blueprint_tree()
        .click_label_modifiers("Viewport (Grid container)", Modifiers::COMMAND);
    harness
        .blueprint_tree()
        .right_click_label("Viewport (Grid container)");
    harness.snapshot_app("context_menu_multi_selection_05");
    harness.key_press(egui::Key::Escape);

    // View + data result
    harness.blueprint_tree().click_label("2D view");
    harness
        .blueprint_tree()
        .click_label_modifiers("boxes2d", Modifiers::COMMAND);
    harness.blueprint_tree().right_click_label("boxes2d");
    harness.snapshot("context_menu_multi_selection_06");
    harness.key_press(egui::Key::Escape);

    harness.streams_tree().click_label("boxes2d");
    harness
        .blueprint_tree()
        .click_label_modifiers("boxes3d", Modifiers::COMMAND);
    harness.blueprint_tree().right_click_label("boxes3d");
    harness.snapshot("context_menu_multi_selection_07");
    harness.key_press(egui::Key::Escape);

    harness
        .streams_tree()
        .click_label_modifiers("half_sizes", Modifiers::COMMAND);
    harness.streams_tree().right_click_label("half_sizes");
    harness.snapshot("context_menu_multi_selection_08");
}
