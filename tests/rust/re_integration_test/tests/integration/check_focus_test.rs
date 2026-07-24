//! This test logs a few boxes and performs the following focus checks:
//!
//! - Double-click on a box in the first view
//!   - check ONLY the corresponding view expands and scrolls
//!   - check the streams view expands and scrolls
//! - Double-click on the leaf "boxes3d" entity in the streams view, check both views expand (manual scrolling might be needed).

use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_viewer_context::ViewClass as _;
use re_viewer::external::{re_sdk_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_test_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::vec2(1024.0, 768.0)),
        max_steps: Some(200),      // Allow animations to finish.
        step_dt: Some(1.0 / 60.0), // Allow double clicks to go through.
        ..Default::default()
    });
    harness.init_recording();
    harness.set_selection_panel_opened(false);

    // Log some data.
    harness.log_entity("group/boxes3d", |builder| {
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
    harness.log_entity("txt/hello", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::TextDocument::new("Hello World!"),
        )
    });

    harness
}

fn setup_single_view_blueprint(harness: &mut egui_kittest::Harness<'_, re_viewer::App>) {
    harness.clear_current_blueprint();

    let root_cid = harness.add_blueprint_container(egui_tiles::ContainerKind::Horizontal, None);
    let tab_cid = harness.add_blueprint_container(egui_tiles::ContainerKind::Tabs, Some(root_cid));
    let vertical_cid =
        harness.add_blueprint_container(egui_tiles::ContainerKind::Vertical, Some(root_cid));

    let mut view_1 =
        ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());
    view_1.display_name = Some("3D view 1".into());
    let mut view_2 =
        ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());
    view_2.display_name = Some("3D view 2".into());

    harness.setup_viewport_blueprint(move |_viewer_context, blueprint| {
        let text_views = (0..20).map(|i| {
            let mut view = ViewBlueprint::new_with_root_wildcard(
                re_view_text_document::TextDocumentView::identifier(),
            );
            view.display_name = Some(format!("Text view {i}"));
            view
        });
        blueprint.add_views(text_views, Some(tab_cid), None);
        blueprint.add_views([view_1, view_2].into_iter(), Some(vertical_cid), None);
    });
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_check_focus() {
    let mut harness = make_test_harness();
    setup_single_view_blueprint(&mut harness);

    // Make the left panel wider.
    let centerline = harness.get_panel_position("Text view 0").left_center();
    let target_pos = centerline + egui::vec2(100.0, 0.0);
    harness.drag_at(centerline);
    harness.snapshot_app("check_focus_1");
    harness.hover_at(target_pos);
    harness.snapshot_app("check_focus_2");
    harness.drop_at(target_pos);
    harness.snapshot_app("check_focus_3");

    // One of the boxes is at the center of the view.
    let pixel_of_a_box = harness.get_panel_position("3D view 1").center();

    // Hover over the box.
    harness.hover_at(pixel_of_a_box);
    harness.run_ok();

    // Let the app render. This will run the picking logic which needs the GPU
    // and lets the app find the hovered box.
    harness.render().expect("Cannot render app");
    harness.run();
    harness.snapshot_app("check_focus_4");

    // Double click on the box, see how it expands the view.
    harness.click_at(pixel_of_a_box);
    harness.click_at(pixel_of_a_box);
    harness.snapshot_app("check_focus_5");

    // Scroll down to see the second view stays collapsed.
    harness.blueprint_tree().hover_label("3D view 1");
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Page,
        delta: egui::vec2(0.0, -1.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    harness.snapshot_app("check_focus_6");

    // Double click the entity on the streams tree and see all views expand.
    harness.streams_tree().hover_label("boxes3d");
    harness.streams_tree().click_label("boxes3d");
    harness.streams_tree().click_label("boxes3d");
    harness.snapshot_app("check_focus_7");

    // Scroll down to see the second view is entirely expanded.
    harness.blueprint_tree().hover_label("3D view 1");
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Page,
        delta: egui::vec2(0.0, -1.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    harness.snapshot_app("check_focus_8");
}
