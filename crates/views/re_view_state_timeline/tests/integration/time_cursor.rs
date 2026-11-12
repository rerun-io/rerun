//! Pressing or dragging anywhere in the view moves the time cursor, like in the time panel.

use re_chunk_store::RowId;
use re_log_types::{TimeInt, TimePoint, Timeline};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::Harness;
use re_test_viewport::TestContextExt as _;
use re_view_state_timeline::StateTimelineView;
use re_viewer_context::{ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

// Same layout constants the view uses; see `view_class.rs`.
const LANE_BAND_HEIGHT: f32 = 22.0;
const LANE_LABEL_HEIGHT: f32 = 14.0;
const TIME_AXIS_HEIGHT: f32 = 20.0;
const TOP_MARGIN: f32 = 4.0;

const VIEW_SIZE: egui::Vec2 = egui::vec2(500.0, 250.0);

const LANE_Y: f32 = TIME_AXIS_HEIGHT + TOP_MARGIN + LANE_LABEL_HEIGHT + LANE_BAND_HEIGHT / 2.0;

fn setup() -> (TestContext, ViewId) {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let timeline = Timeline::new_sequence("tick");
    for (tick, state) in [(0, "Idle"), (10, "Moving"), (20, "Idle"), (30, "Charging")] {
        test_context.log_entity("state/robot", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::from([(timeline, tick)]),
                &re_sdk_types::archetypes::StateChange::single(state),
            )
        });
    }
    test_context.set_active_timeline(*timeline.name());
    test_context.set_time(0);

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            StateTimelineView::identifier(),
        ))
    });
    (test_context, view_id)
}

fn pointer_button(harness: &mut Harness<'_>, pos: egui::Pos2, pressed: bool) {
    harness.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
}

fn move_pointer(harness: &mut Harness<'_>, pos: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(pos));
    harness.step();
}

fn current_time(test_context: &TestContext) -> Option<TimeInt> {
    test_context.time_ctrl.read().time_int()
}

#[test]
fn test_pressing_and_dragging_anywhere_moves_the_time_cursor() {
    for (what, y) in [
        ("the ruler", TIME_AXIS_HEIGHT / 2.0),
        ("a lane", LANE_Y),
        ("empty space below the lanes", VIEW_SIZE.y - 10.0),
    ] {
        let (test_context, view_id) = setup();
        let mut harness = test_context
            .setup_kittest_for_rendering_ui(VIEW_SIZE)
            .build_ui(|ui| {
                test_context.run_with_single_view(ui, view_id);
            });
        harness.run();
        let initial_time = current_time(&test_context);

        // Pressing jumps the cursor there.
        let start = egui::pos2(VIEW_SIZE.x * 0.75, y);
        move_pointer(&mut harness, start);
        pointer_button(&mut harness, start, true);
        harness.step();
        let pressed_time = current_time(&test_context);
        assert_ne!(
            pressed_time, initial_time,
            "pressing on {what} should move the time cursor"
        );

        // Dragging scrubs it.
        let end = egui::pos2(VIEW_SIZE.x * 0.25, y);
        for i in 1..=5 {
            move_pointer(&mut harness, start.lerp(end, i as f32 / 5.0));
        }
        pointer_button(&mut harness, end, false);
        harness.step();
        let dragged_time = current_time(&test_context);
        assert!(
            dragged_time < pressed_time,
            "dragging left on {what} should move the cursor back: {dragged_time:?} vs {pressed_time:?}"
        );
    }
}
