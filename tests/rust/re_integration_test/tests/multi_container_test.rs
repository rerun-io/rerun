use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_viewer_context::{ContainerId, ViewClass as _};
use re_viewer::external::{re_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_multi_view_test_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::Vec2::new(1024.0, 1024.0)),
    });
    harness.init_recording();
    harness.set_selection_panel_opened(false);

    // Log some data
    harness.log_entity("boxes3d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (1.0, 1.0, 0.0)],
                [(0.2, 0.4, 0.2), (0.2, 0.2, 0.4), (0.4, 0.2, 0.2)],
            )
            .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF])
            .with_fill_mode(re_types::components::FillMode::Solid),
        )
    });
    harness.log_entity("boxes2d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_types::archetypes::Boxes2D::from_centers_and_half_sizes(
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
            &re_types::archetypes::BarChart::new(vector),
        )
    });

    let timeline = re_sdk::Timeline::new_sequence("timeline_a");
    harness.log_entity("text_log", |builder| {
        builder.with_archetype(
            RowId::new(),
            [(timeline, 1)],
            &re_types::archetypes::TextLog::new("Hello World!")
                .with_level(re_types::components::TextLogLevel::INFO),
        )
    });

    harness.clear_current_blueprint();
    harness
}

fn add_views_to_container(
    harness: &mut egui_kittest::Harness<'_, re_viewer::App>,
    cid: Option<ContainerId>,
    count: usize,
    view_index_base: usize,
) {
    let x = (0..)
        .flat_map(|i| {
            let mut view_3d =
                ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());
            view_3d.display_name = Some(format!("3D view {}", view_index_base + i * 4));
            let mut view_2d =
                ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView2D::identifier());
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

// Return the number of views added
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
    for _ in 0..2 {
        let kind = if level % 2 == 0 {
            egui_tiles::ContainerKind::Horizontal
        } else {
            egui_tiles::ContainerKind::Vertical
        };
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

#[tokio::test(flavor = "multi_thread")]
pub async fn test_multi_container_deep_nested() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 3, 2, 0);
    harness.snapshot_app("multi_container_deep_nested");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_multi_container_many_views() {
    let mut harness = make_multi_view_test_harness();
    add_views_to_container(&mut harness, None, 200, 0);
    harness.snapshot_app("multi_container_many_views");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_multi_container_drag_n_drop_single_view() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 2, 4, 0);

    harness.drag_nth_label("3D view 0", 0);
    harness.snapshot_app("multi_container_drag_n_drop_single_view_1");

    harness.hover_nth_label("Vertical container", 1);
    harness.snapshot_app("multi_container_drag_n_drop_single_view_2");

    harness.drop_nth_label("2D view 9", 0);
    harness.snapshot_app("multi_container_drag_n_drop_single_view_3");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_multi_container_drag_single_view() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 2, 4, 0);

    harness.drag_nth_label("3D view 0", 0);
    harness.snapshot_app("multi_container_drag_single_view_1");

    harness.hover_nth_label("Vertical container", 1);
    harness.snapshot_app("multi_container_drag_single_view_2");

    harness.drop_nth_label("2D view 9", 0);
    harness.snapshot_app("multi_container_drag_single_view_3");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_multi_container_drag_container() {
    let mut harness = make_multi_view_test_harness();
    add_containers_recursive(&mut harness, None, 2, 4, 0);

    harness.drag_nth_label("Vertical container", 0);
    harness.snapshot_app("multi_container_drag_container_1");

    harness.hover_nth_label("Vertical container", 1);
    harness.snapshot_app("multi_container_drag_container_2");

    harness.hover_nth_label("Horizontal container", 1);
    harness.snapshot_app("multi_container_drag_container_3");

    // Hover a little over root container
    let point = harness
        .get_nth_label("Viewport (Grid container)", 0)
        .rect()
        .center_top();
    harness.event(egui::Event::PointerMoved(point));
    harness.run_ok();
    harness.snapshot_app("multi_container_drag_container_4");

    harness.hover_nth_label("Viewport (Grid container)", 0);
    harness.snapshot_app("multi_container_drag_container_5");

    harness.drop_nth_label("Viewport (Grid container)", 0);
    harness.snapshot_app("multi_container_drag_container_6");
}
