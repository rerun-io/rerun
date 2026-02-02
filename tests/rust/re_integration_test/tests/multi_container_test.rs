use egui::vec2;
use egui_kittest::kittest::Queryable as _;
use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_viewer_context::{ContainerId, RecommendedView, ViewClass as _};
use re_viewer::external::{re_sdk_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_multi_view_test_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::Vec2::new(1024.0, 1024.0)),
        max_steps: Some(100), // Allow animations to finish
        ..Default::default()
    });
    harness.init_recording();

    // Log some data
    harness.log_entity("boxes3d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
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
            TimePoint::STATIC,
            &re_sdk_types::archetypes::Boxes2D::from_centers_and_half_sizes(
                [(-1.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
                [(0.2, 0.4), (0.2, 0.2), (0.4, 0.2)],
            )
            .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF]),
        )
    });

    let vector = (0..16).map(|i| i as f32).collect::<Vec<_>>();
    harness.log_entity("bar_chart", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::BarChart::new(vector),
        )
    });

    let timeline = re_sdk::Timeline::new_sequence("timeline_a");
    harness.log_entity("text_log", |builder| {
        builder.with_archetype(
            RowId::new(),
            [(timeline, 1)],
            &re_sdk_types::archetypes::TextLog::new("Hello World!")
                .with_level(re_sdk_types::components::TextLogLevel::INFO),
        )
    });

    harness.clear_current_blueprint();
    harness.set_selection_panel_opened(false);
    harness
}

// Adds `count` views to the given container, names them sequentially from an index base.
fn add_views_to_container(
    harness: &mut egui_kittest::Harness<'_, re_viewer::App>,
    cid: Option<ContainerId>,
    count: usize,
    view_index_base: usize,
) {
    let x = (0..)
        .flat_map(|i| {
            let mut view_3d = ViewBlueprint::new(
                re_view_spatial::SpatialView3D::identifier(),
                RecommendedView::new_single_entity("boxes3d"),
            );
            view_3d.display_name = Some(format!("3D view {}", view_index_base + i * 4));
            let mut view_2d = ViewBlueprint::new(
                re_view_spatial::SpatialView2D::identifier(),
                RecommendedView::new_single_entity("boxes2d"),
            );
            view_2d.display_name = Some(format!("2D view {}", view_index_base + i * 4 + 1));
            let mut view_barchart = ViewBlueprint::new_with_root_wildcard(
                re_view_bar_chart::BarChartView::identifier(),
            );
            view_barchart.display_name =
                Some(format!("Bar chart view {}", view_index_base + i * 4 + 2));
            let mut view_textlog =
                ViewBlueprint::new_with_root_wildcard(re_view_text_log::TextView::identifier());
            view_textlog.display_name =
                Some(format!("Text log view {}", view_index_base + i * 4 + 3));
            [view_3d, view_2d, view_barchart, view_textlog]
        })
        .take(count)
        .collect::<Vec<_>>();

    harness.setup_viewport_blueprint(move |_viewer_context, blueprint| {
        blueprint.add_views(x.into_iter(), cid, None);
    });
}

// Returns the number of total views added
fn add_containers_recursive(
    harness: &mut egui_kittest::Harness<'_, re_viewer::App>,
    cid: Option<ContainerId>,
    level: i32,
    leaf_view_count: usize,
    view_index_base: usize,
) -> usize {
    if level == 0 {
        add_views_to_container(harness, cid, leaf_view_count, view_index_base);
        return leaf_view_count;
    }

    let mut view_index = view_index_base;
    let kind = match level % 2 {
        0 => egui_tiles::ContainerKind::Horizontal,
        _ => egui_tiles::ContainerKind::Vertical,
    };
    for _ in 0..2 {
        let child_cid = harness.add_blueprint_container(kind, cid);
        view_index += add_containers_recursive(
            harness,
            Some(child_cid),
            level - 1,
            leaf_view_count,
            view_index,
        );
    }
    view_index
}

// Tests 3-level deep nested containers
#[tokio::test(flavor = "multi_thread")]
pub async fn test_multi_container_deep_nested() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 3, 2, 0);
    harness.snapshot_app("multi_container_deep_nested");
}

// Tests a lot of views in a single container
#[tokio::test(flavor = "multi_thread")]
pub async fn test_multi_container_many_views() {
    let mut harness = make_multi_view_test_harness();
    add_views_to_container(&mut harness, None, 200, 0);
    harness.snapshot_app("multi_container_many_views");
}

// Tests drag-and-drop of a single view in the blueprint panel
#[tokio::test(flavor = "multi_thread")]
pub async fn test_multi_container_drag_single_view() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 2, 4, 0);

    harness.blueprint_tree().drag_label("3D view 0");
    harness.snapshot_app("multi_container_drag_single_view_1");

    harness
        .blueprint_tree()
        .hover_nth_label("Vertical container", 1);
    harness.snapshot_app("multi_container_drag_single_view_2");

    harness.blueprint_tree().drop_nth_label("2D view 9", 0);
    harness.snapshot_app("multi_container_drag_single_view_3");
}

// Tests drag-and-drop of a container in the blueprint panel
#[tokio::test(flavor = "multi_thread")]
pub async fn test_multi_container_drag_container() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 2, 4, 0);

    harness
        .blueprint_tree()
        .drag_nth_label("Vertical container", 0);
    harness.snapshot_app("multi_container_drag_container_1");

    // Hovering the same kind of container should be disallowed
    harness
        .blueprint_tree()
        .hover_nth_label("Vertical container", 1);
    harness.snapshot_app("multi_container_drag_container_2");

    // Hovering a different kind of container should be allowed
    harness
        .blueprint_tree()
        .hover_nth_label("Horizontal container", 1);
    harness.snapshot_app("multi_container_drag_container_3");

    // Hover a bit over root container to drop before it.
    // It should be disallowed to drop an item before the root container.
    let upper_edge = harness
        .blueprint_tree()
        .get_label("Viewport (Grid container)")
        .rect()
        .center_top();
    harness.hover_at(upper_edge);
    harness.snapshot_app("multi_container_drag_container_4");

    // Hovering the root container otherwise should be allowed
    harness
        .blueprint_tree()
        .hover_label("Viewport (Grid container)");
    harness.snapshot_app("multi_container_drag_container_5");

    harness
        .blueprint_tree()
        .drop_label("Viewport (Grid container)");
    harness.snapshot_app("multi_container_drag_container_6");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_add_container_from_blueprint_panel_menu() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 1, 4, 0);

    // Blueprint panel "…" icon
    harness.click_label("Open menu with more options");
    harness.snapshot_app("add_container_from_blueprint_panel_menu_1");

    harness.click_label_contains("Add view or container");
    harness.snapshot_app("add_container_from_blueprint_panel_menu_2");

    harness.click_label("Horizontal");
    harness.snapshot_app("add_container_from_blueprint_panel_menu_3");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_add_container_from_selection_panel() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 1, 2, 0);
    harness.set_selection_panel_opened(true);

    harness.click_label("Viewport (Grid container)");
    harness.snapshot_app("add_container_from_selection_panel_1");

    // Selection panel "+" icon
    harness.click_label("Add a new view or container to this container");
    harness.snapshot_app("add_container_from_selection_panel_2");

    harness.click_label("Vertical");
    harness.snapshot_app("add_container_from_selection_panel_3");

    // TODO(aedm): count the labels in the selection panel only
    // See: https://github.com/rerun-io/rerun/issues/11628
    let vertical_container_count = harness.query_all_by_label("Vertical container").count();
    assert_eq!(vertical_container_count, 6);
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_multi_change_container_type() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 1, 2, 0);
    harness.set_selection_panel_opened(true);

    harness
        .blueprint_tree()
        .click_nth_label("Vertical container", 0);
    harness.snapshot_app("change_container_type_1");

    harness.change_dropdown_value("Container kind", "Horizontal");
    harness.snapshot_app("change_container_type_2");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_simplify_container_hierarchy() {
    let mut harness = make_multi_view_test_harness();

    // Set up a horizontal container with two vertical containers as its children
    let root_cid = harness.add_blueprint_container(egui_tiles::ContainerKind::Horizontal, None);
    let child_cid_1 =
        harness.add_blueprint_container(egui_tiles::ContainerKind::Vertical, Some(root_cid));
    harness.add_blueprint_container(egui_tiles::ContainerKind::Vertical, Some(root_cid));
    harness.snapshot_app("simplify_container_hierarchy_1");

    // Only add content to the first child, leave the second child empty
    add_views_to_container(&mut harness, Some(child_cid_1), 2, 0);
    harness.snapshot_app("simplify_container_hierarchy_2");

    harness.set_selection_panel_opened(true);
    harness.blueprint_tree().click_label("Horizontal container");
    harness.click_label("Simplify hierarchy");
    harness.snapshot_app("simplify_container_hierarchy_3");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_simplify_root_hierarchy() {
    let mut harness = make_multi_view_test_harness();

    // Set up a horizontal container with two vertical containers as its children
    let root_cid = harness.add_blueprint_container(egui_tiles::ContainerKind::Horizontal, None);
    let child_cid_1 =
        harness.add_blueprint_container(egui_tiles::ContainerKind::Vertical, Some(root_cid));
    harness.add_blueprint_container(egui_tiles::ContainerKind::Vertical, Some(root_cid));
    harness.snapshot_app("simplify_root_hierarchy_1");

    // Only add content to the first child, leave the second child empty
    add_views_to_container(&mut harness, Some(child_cid_1), 2, 0);

    harness
        .blueprint_tree()
        .click_label("Viewport (Grid container)");
    harness.snapshot_app("simplify_root_hierarchy_2");

    harness.set_selection_panel_opened(true);
    harness.selection_panel().click_label("Simplify hierarchy");
    harness.snapshot_app("simplify_root_hierarchy_3");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_drag_view_to_other_view_right() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 1, 4, 0);

    let target_pos = harness.get_panel_position("2D view 1").right_center() + vec2(-50.0, 0.0);

    // Drag the view panel widget
    harness.drag_nth_label("3D view 4", 1);
    harness.hover_at(target_pos);
    harness.snapshot_app("drag_view_to_other_view_right_1");

    harness.drop_at(target_pos);
    harness.snapshot_app("drag_view_to_other_view_right_2");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_drag_view_to_other_view_left() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 1, 4, 0);

    let target_pos = harness.get_panel_position("2D view 1").left_center() + vec2(50.0, 0.0);

    // Drag the view panel widget
    harness.drag_nth_label("3D view 4", 1);
    harness.hover_at(target_pos);
    harness.snapshot_app("drag_view_to_other_view_left_1");

    harness.drop_at(target_pos);
    harness.snapshot_app("drag_view_to_other_view_left_2");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_drag_view_to_other_view_center() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 1, 4, 0);

    let target_pos = harness.get_panel_position("2D view 1").center();

    // Drag the view panel widget
    harness.drag_nth_label("3D view 4", 1);
    harness.hover_at(target_pos);
    harness.snapshot_app("drag_view_to_other_view_center_1");

    harness.drop_at(target_pos);
    harness.snapshot_app("drag_view_to_other_view_center_2");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_drag_view_to_other_view_top() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 1, 4, 0);

    let target_pos = harness.get_panel_position("2D view 1").center_top() + vec2(0.0, 50.0);

    // Drag the view panel widget
    harness.drag_nth_label("3D view 4", 1);
    harness.hover_at(target_pos);
    harness.snapshot_app("drag_view_to_other_view_top_1");

    harness.drop_at(target_pos);
    harness.snapshot_app("drag_view_to_other_view_top_2");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_drag_view_to_other_view_bottom() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 1, 4, 0);

    let target_pos = harness.get_panel_position("2D view 1").center_bottom() + vec2(0.0, -50.0);

    // Drag the view panel widget
    harness.drag_nth_label("3D view 4", 1);
    harness.hover_at(target_pos);
    harness.snapshot_app("drag_view_to_other_view_bottom_1");

    harness.drop_at(target_pos);
    harness.snapshot_app("drag_view_to_other_view_bottom_2");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_resize_view_vertical() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 1, 4, 0);

    let centerline = harness.get_panel_position("3D view 4").left_center();
    let target_pos = centerline + vec2(100.0, 0.0);

    harness.drag_at(centerline);
    harness.snapshot_app("resize_view_vertical_1");

    harness.hover_at(target_pos);
    harness.snapshot_app("resize_view_vertical_2");

    harness.drop_at(target_pos);
    harness.snapshot_app("resize_view_vertical_3");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_resize_view_horizontal() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 1, 4, 0);

    let centerline = harness.get_panel_position("3D view 4").center_bottom();
    let target_pos = centerline + vec2(0.0, 100.0);

    harness.drag_at(centerline);
    harness.snapshot_app("resize_view_horizontal_1");

    harness.hover_at(target_pos);
    harness.snapshot_app("resize_view_horizontal_2");

    harness.drop_at(target_pos);
    harness.snapshot_app("resize_view_horizontal_3");
}
