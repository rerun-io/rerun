//! Tests for adding entities to new views. Checks whether the origin and entity path filter are set correctly.

use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
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
        ..Default::default()
    });
    harness.init_recording();
    harness.set_selection_panel_opened(true);

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
    harness.log_entity("tensor", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::Tensor::new(re_sdk_types::datatypes::TensorData::new(
                vec![2, 4],
                re_sdk_types::datatypes::TensorBuffer::U8(
                    vec![0, 100, 255, 22, 211, 64, 155, 40].into(),
                ),
            )),
        )
    });

    harness.clear_current_blueprint();
    harness
}

// Adds `count` number of views to the given container, names them sequentially from an index base.
fn add_views_to_container(
    harness: &mut egui_kittest::Harness<'_, re_viewer::App>,
    cid: Option<ContainerId>,
) {
    let mut view_3d = ViewBlueprint::new(
        re_view_spatial::SpatialView3D::identifier(),
        RecommendedView::new_single_entity("boxes3d"),
    );
    view_3d.display_name = Some("3D view".to_owned());
    let mut view_2d = ViewBlueprint::new(
        re_view_spatial::SpatialView2D::identifier(),
        RecommendedView::new_single_entity("boxes2d"),
    );
    view_2d.display_name = Some("2D view".to_owned());
    let mut view_barchart =
        ViewBlueprint::new_with_root_wildcard(re_view_bar_chart::BarChartView::identifier());
    view_barchart.display_name = Some("Bar chart view".to_owned());
    let mut view_textlog =
        ViewBlueprint::new_with_root_wildcard(re_view_text_log::TextView::identifier());
    view_textlog.display_name = Some("Text log view".to_owned());
    let mut view_tensor =
        ViewBlueprint::new_with_root_wildcard(re_view_tensor::TensorView::identifier());
    view_tensor.display_name = Some("Tensor view".to_owned());

    harness.setup_viewport_blueprint(move |_viewer_context, blueprint| {
        blueprint.add_views(
            [view_3d, view_2d, view_barchart, view_textlog, view_tensor].into_iter(),
            cid,
            None,
        );
    });
}

fn get_origin(harness: &mut egui_kittest::Harness<'_, re_viewer::App>) -> String {
    harness
        .selection_panel()
        .root()
        .get_by_label("Space origin")
        .parent()
        .expect("Space origin label should have a parent")
        .get_by_role(Role::TextInput)
        .value()
        .expect("No value found for space origin")
}

fn get_entity_path_filter(harness: &mut egui_kittest::Harness<'_, re_viewer::App>) -> String {
    harness
        .selection_panel()
        .root()
        .get_by_role(Role::MultilineTextInput)
        .value()
        .expect("No value found for multiline text input")
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_add_entity_to_view_boxes3d() {
    let mut harness = make_multi_view_test_harness();
    add_views_to_container(&mut harness, None);

    harness.right_click_label("Viewport (Grid container)");
    harness.click_label("Expand all");

    harness.blueprint_tree().right_click_label("boxes3d");
    harness.snapshot_app("add_entity_to_view_boxes3d_1");

    harness.hover_label_contains("Add to new view");
    harness.snapshot_app("add_entity_to_view_boxes3d_2");

    harness.click_label("3D");
    harness.snapshot_app("add_entity_to_view_boxes3d_3");

    assert_eq!("/", get_origin(&mut harness));
    assert_eq!("+ /boxes3d/**", get_entity_path_filter(&mut harness));

    harness.blueprint_tree().right_click_label("/");
    harness.snapshot_app("add_entity_to_view_boxes3d_4");

    harness.click_label("Remove");
    harness.set_selection_panel_opened(false);
    harness.snapshot_app("add_entity_to_view_boxes3d_5");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_add_entity_to_view_boxes2d() {
    let mut harness = make_multi_view_test_harness();
    add_views_to_container(&mut harness, None);

    harness.right_click_label("Viewport (Grid container)");
    harness.click_label("Expand all");

    harness.blueprint_tree().right_click_label("boxes2d");
    harness.snapshot_app("add_entity_to_view_boxes2d_1");

    harness.hover_label_contains("Add to new view");
    harness.snapshot_app("add_entity_to_view_boxes2d_2");

    harness.click_label("2D");
    harness.snapshot_app("add_entity_to_view_boxes2d_3");

    assert_eq!("/", get_origin(&mut harness));
    assert_eq!("+ /boxes2d/**", get_entity_path_filter(&mut harness));

    harness.blueprint_tree().right_click_label("/");
    harness.snapshot_app("add_entity_to_view_boxes2d_4");

    harness.click_label("Remove");
    harness.set_selection_panel_opened(false);
    harness.snapshot_app("add_entity_to_view_boxes2d_5");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_add_entity_to_view_bar_chart() {
    let mut harness = make_multi_view_test_harness();
    add_views_to_container(&mut harness, None);

    harness.right_click_label("Viewport (Grid container)");
    harness.click_label("Expand all");

    harness.blueprint_tree().right_click_label("bar_chart");
    harness.snapshot_app("add_entity_to_view_bar_chart_1");

    harness.hover_label_contains("Add to new view");
    harness.snapshot_app("add_entity_to_view_bar_chart_2");

    harness.click_label("Bar chart");
    harness.snapshot_app("add_entity_to_view_bar_chart_3");

    // When adding a bar chart, to a new view, the origin is set to the entity path
    assert_eq!(get_origin(&mut harness), "/bar_chart");
    assert_eq!(get_entity_path_filter(&mut harness), "+ /bar_chart/**");

    harness
        .blueprint_tree()
        .right_click_nth_label("bar_chart", 1);
    harness.snapshot_app("add_entity_to_view_bar_chart_4");

    harness.click_label("Remove");
    harness.set_selection_panel_opened(false);
    harness.snapshot_app("add_entity_to_view_bar_chart_5");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_add_entity_to_view_text_log() {
    let mut harness = make_multi_view_test_harness();
    add_views_to_container(&mut harness, None);

    harness.right_click_label("Viewport (Grid container)");
    harness.click_label("Expand all");

    harness.blueprint_tree().right_click_label("text_log");
    harness.snapshot_app("add_entity_to_view_text_log_1");

    harness.hover_label_contains("Add to new view");
    harness.snapshot_app("add_entity_to_view_text_log_2");

    harness.click_label("Text log");
    harness.snapshot_app("add_entity_to_view_text_log_3");

    // When adding a text log, to a new view, the origin is set to the entity path
    assert_eq!(get_origin(&mut harness), "/text_log");
    assert_eq!(get_entity_path_filter(&mut harness), "+ /text_log/**");

    harness
        .blueprint_tree()
        .right_click_nth_label("text_log", 1);
    harness.snapshot_app("add_entity_to_view_text_log_4");

    harness.click_label("Remove");
    harness.set_selection_panel_opened(false);
    harness.snapshot_app("add_entity_to_view_text_log_5");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_add_entity_to_view_tensor() {
    let mut harness = make_multi_view_test_harness();
    add_views_to_container(&mut harness, None);

    harness.right_click_label("Viewport (Grid container)");
    harness.click_label("Expand all");

    harness.blueprint_tree().right_click_label("tensor");
    harness.snapshot_app("add_entity_to_view_tensor_1");

    harness.hover_label_contains("Add to new view");
    harness.snapshot_app("add_entity_to_view_tensor_2");

    harness.click_nth_label("Tensor", 2);
    harness.snapshot_app("add_entity_to_view_tensor_3");

    // When adding a text log, to a new view, the origin is set to the entity path
    assert_eq!(get_origin(&mut harness), "/tensor");
    assert_eq!(get_entity_path_filter(&mut harness), "+ /tensor/**");

    harness.blueprint_tree().right_click_nth_label("tensor", 1);
    harness.snapshot_app("add_entity_to_view_tensor_4");

    harness.click_label("Remove");
    harness.set_selection_panel_opened(false);
    harness.snapshot_app("add_entity_to_view_tensor_5");
}
