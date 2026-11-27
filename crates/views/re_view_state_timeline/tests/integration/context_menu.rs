//! Right-clicking a state opens a context menu for it.

use re_chunk_store::RowId;
use re_log_types::{TimeInt, TimePoint, Timeline};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::Harness;
use re_test_context::external::egui_kittest::kittest::Queryable as _;
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

fn move_pointer(harness: &mut Harness<'_>, pos: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(pos));
    harness.step();
}

fn current_time(test_context: &TestContext) -> Option<TimeInt> {
    test_context.time_ctrl.read().time_int()
}

fn right_click_at(harness: &mut Harness<'_>, pos: egui::Pos2) {
    move_pointer(harness, pos);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
    }
    // Hovering a state republishes its highlight every frame, so the UI never fully settles.
    harness.run_steps(3);
}

const MENU_ITEMS: [&str; 3] = [
    "Move time cursor here",
    "Jump to start of state",
    "Set time selection to state",
];

#[test]
fn test_right_clicking_empty_space_moves_the_time_cursor_without_a_menu() {
    let (test_context, view_id) = setup();
    let mut harness = test_context
        .setup_kittest_for_rendering_ui(VIEW_SIZE)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    harness.run();
    let initial_time = current_time(&test_context);

    right_click_at(
        &mut harness,
        egui::pos2(VIEW_SIZE.x * 0.75, VIEW_SIZE.y - 10.0),
    );

    assert_ne!(current_time(&test_context), initial_time);
    for item in MENU_ITEMS {
        assert!(
            harness.query_by_label(item).is_none(),
            "no state menu expected outside a state, found {item:?}"
        );
    }
}

#[test]
fn test_state_context_menu() {
    let (test_context, view_id) = setup();
    let mut harness = test_context
        .setup_kittest_for_rendering_ui(VIEW_SIZE)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    harness.run();
    let lane_pos = egui::pos2(VIEW_SIZE.x * 0.75, LANE_Y);

    // "Move time cursor here" keeps the plain right-click behavior available on a state.
    right_click_at(&mut harness, lane_pos);
    harness.snapshot("state_timeline_context_menu");
    harness.get_by_label("Move time cursor here").click();
    harness.run_steps(3);
    let here = current_time(&test_context).expect("time cursor").as_i64();
    assert!(
        here > 0,
        "expected the cursor to move into the data, got {here}"
    );

    // States change every 10 ticks, the last one at 30.
    let state_start = (here / 10 * 10).min(30);

    right_click_at(&mut harness, lane_pos);
    harness.get_by_label("Jump to start of state").click();
    harness.run_steps(3);
    assert_eq!(
        current_time(&test_context),
        Some(TimeInt::new_temporal(state_start))
    );

    right_click_at(&mut harness, lane_pos);
    harness.get_by_label("Set time selection to state").click();
    harness.run_steps(3);
    let time_ctrl = test_context.time_ctrl.read();
    let selection = time_ctrl.time_selection().expect("a time selection");
    assert_eq!(selection.min.floor().as_i64(), state_start);
    if state_start < 30 {
        assert_eq!(selection.max.floor().as_i64(), state_start + 10);
    }
    assert_eq!(
        time_ctrl.loop_mode(),
        re_sdk_types::blueprint::components::LoopMode::Selection
    );
}
