//! Tests dragging a component from the streams tree onto a State Timeline view, which
//! should add a `StateVisualizer` instruction that remaps `StateChange.state` from
//! the dropped component.

use re_integration_test::HarnessExt as _;
use re_sdk::log::RowId;
use re_viewer::external::re_sdk_types;
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::Vec2::new(1200.0, 800.0)),
        ..Default::default()
    });
    harness.init_recording();

    let timeline = re_sdk::Timeline::new_sequence("tick");

    // `/mode` carries a native `StateChange` archetype. The `StateVisualizer` picks
    // it up automatically, so the view has one lane before any drop.
    let states = ["Idle", "Moving", "Working", "Idle"];
    for (i, state) in states.iter().enumerate() {
        let tick = i64::try_from(i).expect("test index fits in i64") * 10;
        harness.log_entity("mode", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, tick)],
                &re_sdk_types::archetypes::StateChange::new().with_state(*state),
            )
        });
    }

    // `/level` carries `Scalars`. There is no auto-attached visualizer for it;
    // it should only appear in the view after the scalar component is dropped
    // onto it.
    for i in 0_i64..4 {
        harness.log_entity("level", |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline, i * 10)],
                &re_sdk_types::archetypes::Scalars::single(i as f64),
            )
        });
    }

    harness.clear_current_blueprint();
    harness.setup_viewport_blueprint(|_viewer_context, blueprint| {
        let mut view = ViewBlueprint::new_with_root_wildcard("StateTimeline".into());
        view.display_name = Some("State Timeline view".into());
        blueprint.add_view_at_root(view);
    });

    harness
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_drop_component_to_state_timeline_view() {
    let mut harness = make_harness();

    let drop_point = harness.get_panel_position("State Timeline view").center();

    // Expand the streams tree so component-level rows become visible and
    // therefore draggable.
    harness.streams_tree().right_click_label("/");
    harness.click_label("Expand all");
    harness.snapshot_app("drop_component_to_state_timeline_view_1_initial");

    // Drag the `scalars` component onto the State Timeline view.
    harness.streams_tree().drag_label("scalars");
    harness.hover_at(drop_point);
    harness.snapshot_app("drop_component_to_state_timeline_view_2_hover");
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Grabbing);

    harness.drop_at(drop_point);
    harness.snapshot_app("drop_component_to_state_timeline_view_3_after_drop");
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Default);

    // Dragging the same component a second time should be a no-op: the
    // visualizer instruction is already present, so the snapshot after the
    // second drop should match the snapshot after the first.
    harness.streams_tree().drag_label("scalars");
    harness.hover_at(drop_point);
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Grabbing);
    harness.drop_at(drop_point);
    harness.snapshot_app("drop_component_to_state_timeline_view_4_after_redrop");
    assert_eq!(harness.cursor_icon(), egui::CursorIcon::Default);
}
