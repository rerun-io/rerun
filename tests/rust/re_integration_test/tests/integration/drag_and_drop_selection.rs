//! Verifies the three selection-state behaviors of drag-and-drop onto a view:
//! - A successful drop selects the view.
//! - A failed (rejected) drop leaves the selection unchanged.
//! - Dragging an unselected item does not change the selection.

use std::f64::consts::TAU;

use re_integration_test::HarnessExt as _;
use re_integration_test::ViewerHarnessExt as _;
use re_sdk::external::re_log_types::EntityPathFilter;
use re_sdk::log::RowId;
use re_viewer::external::re_sdk_types;
use re_viewer::external::re_viewer_context::{
    Item, ItemCollection, RecommendedView, SystemCommand, SystemCommandSender as _, ViewClass as _,
    ViewId,
};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_harness<'a>() -> (egui_kittest::Harness<'a, re_viewer::App>, ViewId) {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();
    harness.set_selection_panel_opened(true);

    let timeline = re_sdk::Timeline::new_sequence("frame");
    for i in 0..100 {
        harness.log_entity("cos_curve", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &re_sdk_types::archetypes::Scalars::single((i as f64 / 100.0 * TAU).cos()),
            )
        });
        harness.log_entity("line_curve", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i)],
                &re_sdk_types::archetypes::Scalars::single(i as f64 / 100.0 + 0.2),
            )
        });
    }

    harness.clear_current_blueprint();

    let mut view = ViewBlueprint::new(
        re_view_time_series::TimeSeriesView::identifier(),
        RecommendedView {
            origin: "/".into(),
            query_filter: EntityPathFilter::default(),
        },
    );
    view.display_name = Some("PLOT".into());
    let view_id = view.id;

    harness.setup_viewport_blueprint(move |_viewer_context, blueprint| {
        blueprint.add_views(std::iter::once(view), None, None);
    });

    (harness, view_id)
}

fn current_selection(harness: &mut egui_kittest::Harness<'_, re_viewer::App>) -> ItemCollection {
    harness.run_with_viewer_context(|ctx| ctx.selection().clone())
}

/// Dragging `cos_curve` from the streams tree onto the PLOT view adds the entity
/// and selects the view.
#[tokio::test(flavor = "multi_thread")]
pub async fn successful_drop_selects_view() {
    let (mut harness, view_id) = make_harness();

    let view_item = Item::View(view_id);
    assert!(
        !current_selection(&mut harness).contains_item(&view_item),
        "view should not be pre-selected before the drop"
    );

    harness.streams_tree().click_label("cos_curve");
    harness.streams_tree().drag_label("cos_curve");

    let drop_point = harness.get_panel_position("PLOT").center();
    harness.hover_at(drop_point);
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Grabbing);

    harness.drop_at(drop_point);
    harness.run_ok();

    let selection = current_selection(&mut harness);
    assert!(
        selection.contains_item(&view_item),
        "view must be selected after a successful drop, got {:?}",
        selection.iter_items().collect::<Vec<_>>()
    );
    assert_eq!(
        selection.len(),
        1,
        "only the view should be selected after a successful drop"
    );

    harness.snapshot_app("drag_and_drop_selection_successful_drop");
}

/// Re-dragging an entity that is already in the view is rejected; the selection
/// must remain whatever it was just before the drop (the `cos_curve` streams-tree item).
#[tokio::test(flavor = "multi_thread")]
pub async fn failed_drop_does_not_change_selection() {
    let (mut harness, _view_id) = make_harness();

    // First drop: add `cos_curve` so the second drop will be rejected.
    harness.streams_tree().click_label("cos_curve");
    harness.streams_tree().drag_label("cos_curve");
    let drop_point = harness.get_panel_position("PLOT").center();
    harness.hover_at(drop_point);
    harness.drop_at(drop_point);
    harness.run_ok();

    // Re-select `cos_curve` in the streams tree, then try to drop it again onto the PLOT view.
    harness.streams_tree().click_label("cos_curve");
    let before = current_selection(&mut harness);

    harness.streams_tree().drag_label("cos_curve");
    harness.hover_at(drop_point);
    assert_eq!(
        harness.cursor_icon(),
        egui::CursorIcon::NoDrop,
        "re-dropping an entity already in the view should be rejected"
    );

    harness.drop_at(drop_point);
    harness.run_ok();

    let after = current_selection(&mut harness);
    assert_eq!(
        after.iter_items().collect::<Vec<_>>(),
        before.iter_items().collect::<Vec<_>>(),
        "selection must not change after a rejected drop"
    );

    harness.snapshot_app("drag_and_drop_selection_failed_drop");
}

/// Dragging an item that is not part of the current selection must not change the
/// selection, even on a successful drop.
#[tokio::test(flavor = "multi_thread")]
pub async fn dragging_unselected_item_does_not_change_selection() {
    let (mut harness, view_id) = make_harness();

    // Pre-select the PLOT view.
    harness.run_with_viewer_context(move |ctx| {
        ctx.command_sender()
            .send_system(SystemCommand::set_selection(Item::View(view_id)));
    });
    harness.run_ok();

    let view_item = Item::View(view_id);
    let before = current_selection(&mut harness);
    assert!(
        before.contains_item(&view_item) && before.len() == 1,
        "precondition: only the PLOT view should be selected, got {:?}",
        before.iter_items().collect::<Vec<_>>()
    );

    // Drag `line_curve` without clicking it first — it is not part of the current selection.
    harness.streams_tree().drag_label("line_curve");
    let drop_point = harness.get_panel_position("PLOT").center();
    harness.hover_at(drop_point);
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Grabbing);
    harness.drop_at(drop_point);
    harness.run_ok();

    let after = current_selection(&mut harness);
    assert!(
        after.contains_item(&view_item) && after.len() == 1,
        "selection must remain the PLOT view when dragging an unselected item, got {:?}",
        after.iter_items().collect::<Vec<_>>()
    );

    harness.snapshot_app("drag_and_drop_selection_unselected_item");
}
