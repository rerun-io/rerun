use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::external::re_log_types::EntityPathFilter;
use re_sdk::log::RowId;
use re_viewer::external::re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewer::external::{re_sdk_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_test_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();
    harness.set_selection_panel_opened(false);

    // Log some data
    harness.log_entity("group/boxes3d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                [(-1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (1.0, 1.0, 0.0)],
                [(0.2, 0.4, 0.2), (0.2, 0.2, 0.4), (0.4, 0.2, 0.2)],
            )
            .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF])
            .with_fill_mode(re_sdk_types::components::FillMode::Solid),
        )
    });
    harness
}

fn setup_single_view_blueprint(harness: &mut egui_kittest::Harness<'_, re_viewer::App>) {
    harness.clear_current_blueprint();

    let mut view3d = ViewBlueprint::new(
        re_view_spatial::SpatialView3D::identifier(),
        RecommendedView {
            origin: "group".into(),
            query_filter: EntityPathFilter::all(),
        },
    );
    view3d.display_name = Some("Test view".into());

    harness.setup_viewport_blueprint(|_viewer_context, blueprint| {
        blueprint.add_view_at_root(view3d);
    });
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_blueprint_tree_context_menu() {
    let mut harness = make_test_harness();
    setup_single_view_blueprint(&mut harness);

    harness.snapshot_app("blueprint_tree_context_menu_01");

    // Click on the view panel widget
    harness.right_click_nth_label("Test view", 1);
    harness.snapshot_app("blueprint_tree_context_menu_02");

    harness.click_label("Expand all");
    harness.snapshot_app("blueprint_tree_context_menu_03");

    harness
        .blueprint_tree()
        .right_click_label("Viewport (Grid container)");
    harness.snapshot_app("blueprint_tree_context_menu_04");

    harness.key_press(egui::Key::Escape);
    harness.snapshot_app("blueprint_tree_context_menu_05");

    harness.blueprint_tree().right_click_label("Test view");
    harness.snapshot_app("blueprint_tree_context_menu_06");

    harness.key_press(egui::Key::Escape);
    harness.snapshot_app("blueprint_tree_context_menu_07");

    harness.blueprint_tree().right_click_label("group");
    harness.snapshot_app("blueprint_tree_context_menu_08");

    harness.key_press(egui::Key::Escape);
    harness.snapshot_app("blueprint_tree_context_menu_09");

    harness.blueprint_tree().right_click_label("boxes3d");
    harness.snapshot_app("blueprint_tree_context_menu_10");
}
