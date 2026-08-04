//! Tests dragging and dropping streams from the streams tree onto specific views,
//! verifying that data streams can be properly assigned to target plots and that
//! the UI correctly handles drag-and-drop interactions.

use std::f64::consts::TAU;

use egui::Modifiers;
use re_integration_test::HarnessExt as _;
use re_sdk::log::RowId;
use re_viewer::external::re_sdk_types;
use re_viewer::external::re_viewer_context::{Item, RecommendedView, ViewClass as _, ViewId};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();
    harness.set_selection_panel_opened(true);

    let timeline = re_sdk::Timeline::new_sequence("timeline_a");
    for i in 0..100 {
        harness.log_entity("sin_curve", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &re_sdk_types::archetypes::Scalars::single((i as f64 / 100.0 * TAU).sin()),
            )
        });
        harness.log_entity("line_curve", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &re_sdk_types::archetypes::Scalars::single(i as f64 / 100.0),
            )
        });
    }

    // Set up a multi-view blueprint
    harness.clear_current_blueprint();

    let mut view_1 = ViewBlueprint::new(
        re_view_time_series::TimeSeriesView::identifier(),
        RecommendedView {
            origin: "/".into(),
            query_filter: re_sdk::external::re_log_types::EntityPathFilter::default(),
        },
    );
    view_1.display_name = Some("Plot 1".into());
    let mut view_2 = ViewBlueprint::new(
        re_view_time_series::TimeSeriesView::identifier(),
        RecommendedView {
            origin: "/".into(),
            query_filter: re_sdk::external::re_log_types::EntityPathFilter::default(),
        },
    );
    view_2.display_name = Some("Plot 2".into());

    harness.setup_viewport_blueprint(move |_viewer_context, blueprint| {
        blueprint.add_views([view_1, view_2].into_iter(), None, None);
    });

    harness
}

fn assert_view_entity_inclusion(
    harness: &mut egui_kittest::Harness<'_, re_viewer::App>,
    view_name: &str,
    expectations: &[(&str, bool)],
) {
    let view_name = view_name.to_owned();
    let assertion_view_name = view_name.clone();
    let expectations = expectations
        .iter()
        .map(|(entity_path, expected)| (re_sdk::EntityPath::from(*entity_path), *expected))
        .collect::<Vec<_>>();

    let results = harness.setup_viewport_blueprint(move |_ctx, blueprint| {
        let view = blueprint
            .views
            .values()
            .find(|view| view.display_name.as_deref() == Some(&view_name))
            .unwrap_or_else(|| panic!("Failed to find view {view_name:?}"));
        let filter = view.contents.entity_path_filter();

        expectations
            .into_iter()
            .map(|(entity_path, expected)| {
                let actual = filter.is_explicitly_included(&entity_path);
                (entity_path, expected, actual)
            })
            .collect::<Vec<_>>()
    });

    for (entity_path, expected, actual) in results {
        assert_eq!(
            actual, expected,
            "Unexpected inclusion state for {entity_path} in {assertion_view_name:?}"
        );
    }
}

fn view_id(harness: &mut egui_kittest::Harness<'_, re_viewer::App>, view_name: &str) -> ViewId {
    let view_name = view_name.to_owned();
    harness.setup_viewport_blueprint(move |_ctx, blueprint| {
        blueprint
            .views
            .values()
            .find(|view| view.display_name.as_deref() == Some(&view_name))
            .unwrap_or_else(|| panic!("Failed to find view {view_name:?}"))
            .id
    })
}

fn assert_selected_item(harness: &mut egui_kittest::Harness<'_, re_viewer::App>, expected: Item) {
    let actual = harness.run_with_app_context(|ctx| ctx.selection().single_item().cloned());
    assert_eq!(actual, Some(expected));
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_drop_stream_to_view() {
    let mut harness = make_harness();
    harness
        .blueprint_tree()
        .click_label("Viewport (Grid container)");

    let drop_point_1 = harness.get_panel_position("Plot 1").center();
    let plot_1_id = view_id(&mut harness, "Plot 1");
    let sin_curve = Item::from(re_sdk::EntityPath::from("sin_curve"));

    // A successful drop selects the target view.
    harness.streams_tree().click_label("sin_curve");
    assert_selected_item(&mut harness, sin_curve.clone());
    harness.streams_tree().drag_label("sin_curve");
    harness.hover_at(drop_point_1);
    harness.snapshot_app("drop_stream_to_view_1");
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Grabbing);

    harness.drop_at(drop_point_1);
    harness.snapshot_app("drop_stream_to_view_2");
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Default);
    assert_selected_item(&mut harness, Item::View(plot_1_id));

    // A rejected drop preserves the dragged entity's selection.
    harness.streams_tree().click_label("sin_curve");
    assert_selected_item(&mut harness, sin_curve.clone());
    harness.streams_tree().drag_label("sin_curve");
    harness.hover_at(drop_point_1);
    harness.snapshot_app("drop_stream_to_view_3");
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::NoDrop);

    harness.drop_at(drop_point_1);
    harness.snapshot_app("drop_stream_to_view_4");
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Default);
    assert_selected_item(&mut harness, sin_curve);

    // Dragging an unselected entity preserves the target view's selection.
    harness.blueprint_tree().click_label("Plot 1");
    assert_selected_item(&mut harness, Item::View(plot_1_id));
    harness.streams_tree().drag_label("line_curve");
    harness.hover_at(drop_point_1);
    harness.snapshot_app("drop_stream_to_view_5");
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Grabbing);

    harness.drop_at(drop_point_1);
    harness.snapshot_app("drop_stream_to_view_6");
    assert_selected_item(&mut harness, Item::View(plot_1_id));
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_drop_multiple_streams_to_view() {
    let mut harness = make_harness();
    harness
        .blueprint_tree()
        .click_label("Viewport (Grid container)");

    let drop_point_1 = harness.get_panel_position("Plot 1").center();
    let drop_point_2 = harness.get_panel_position("Plot 2").center();

    // Drag "sin_curve" to the plot
    harness.streams_tree().drag_label("sin_curve");
    harness.hover_at(drop_point_1);
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Grabbing);
    harness.drop_at(drop_point_1);
    harness.snapshot_app("drop_multiple_streams_to_view_1");
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Default);
    assert_view_entity_inclusion(
        &mut harness,
        "Plot 1",
        &[("sin_curve", true), ("line_curve", false)],
    );

    // Drag both entities to sine plot
    harness.streams_tree().click_label("sin_curve");
    harness
        .streams_tree()
        .click_label_modifiers("line_curve", Modifiers::COMMAND);
    harness.streams_tree().drag_label("line_curve");

    harness.hover_at(drop_point_1);
    harness.snapshot_app("drop_multiple_streams_to_view_2");
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Grabbing);

    harness.drop_at(drop_point_1);
    harness.snapshot_app("drop_multiple_streams_to_view_3");
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Default);
    assert_view_entity_inclusion(
        &mut harness,
        "Plot 1",
        &[("sin_curve", true), ("line_curve", true)],
    );

    // Drag both again, should fail, but should succeed to other plot
    harness.streams_tree().click_label("sin_curve");
    harness
        .streams_tree()
        .click_label_modifiers("line_curve", Modifiers::COMMAND);
    harness.streams_tree().drag_label("line_curve");

    harness.hover_at(drop_point_1);
    harness.snapshot_app("drop_multiple_streams_to_view_4");
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::NoDrop);

    harness.hover_at(drop_point_2);
    harness.snapshot_app("drop_multiple_streams_to_view_5");
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Grabbing);

    harness.drop_at(drop_point_2);
    harness.snapshot_app("drop_multiple_streams_to_view_6");
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Default);
    assert_view_entity_inclusion(
        &mut harness,
        "Plot 1",
        &[("sin_curve", true), ("line_curve", true)],
    );
    assert_view_entity_inclusion(
        &mut harness,
        "Plot 2",
        &[("sin_curve", true), ("line_curve", true)],
    );
}
