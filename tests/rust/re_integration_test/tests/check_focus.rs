use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_viewer_context::ViewClass as _;
use re_viewer::external::{re_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_test_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();

    // Log some data
    harness.log_entity("group/boxes3d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (1.0, 1.0, 0.0)],
                [(0.2, 0.4, 0.2), (0.2, 0.2, 0.4), (0.4, 0.2, 0.2)],
            )
            .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF])
            .with_fill_mode(re_types::components::FillMode::Solid),
        )
    });
    harness.log_entity("txt/hello", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_types::archetypes::TextDocument::new("Hello World!"),
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
pub async fn test_foo() {
    let mut harness = make_test_harness();
    setup_single_view_blueprint(&mut harness);

    let centerline = harness.get_panel_position("Text view 0").left_center();
    let target_pos = centerline + egui::vec2(100.0, 0.0);
    harness.drag_at(centerline);
    harness.hover_at(target_pos);
    harness.drop_at(target_pos);

    // One of the boxes is a bit left to the center
    let pixel_of_a_box = harness.get_panel_position("3D view 1").center() + egui::vec2(-0.0, 10.0);

    // Hover over the box
    harness.hover_at(pixel_of_a_box);

    // Let the app render. This will run the picking logic which needs the GPU
    // and lets the app find the hovered box.
    harness.render().expect("Cannot render app");
    harness.run_steps(50);

    // Double click on the box
    harness.click_at(pixel_of_a_box);
    harness.click_at(pixel_of_a_box);

    harness.blueprint_tree().hover_label("3D view 1");
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Page,
        delta: egui::vec2(0.0, -1.0),
        modifiers: egui::Modifiers::NONE,
    });

    harness.blueprint_tree().click_label("boxes3d");
    harness.blueprint_tree().click_label("boxes3d");

    harness.snapshot_app("xtemp");
}
