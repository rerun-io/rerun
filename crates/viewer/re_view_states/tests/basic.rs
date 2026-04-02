use re_chunk_store::RowId;
use re_log_types::{TimePoint, Timeline};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_view_states::StatesView;
use re_viewer_context::{TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            StatesView::identifier(),
        ))
    })
}

#[test]
fn test_states_basic() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StatesView>();

    let timeline = Timeline::log_tick();

    // Log state transitions for multiple entities using TextLog.
    let state_data: Vec<(i64, &str, &str)> = vec![
        // (tick, entity, state_label)
        (0, "state/robot_mode", "Idle"),
        (10, "state/robot_mode", "Moving"),
        (25, "state/robot_mode", "Working"),
        (40, "state/robot_mode", "Idle"),
        (0, "state/power", "On"),
        (20, "state/power", "Low"),
        (35, "state/power", "Critical"),
        (45, "state/power", "On"),
        (0, "state/connection", "Connected"),
        (15, "state/connection", "Disconnected"),
        (30, "state/connection", "Connected"),
    ];

    for (tick, entity, label) in &state_data {
        let timepoint = TimePoint::from([(timeline, *tick)]);
        test_context.log_entity(*entity, |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint,
                &re_sdk_types::archetypes::TextLog::new(*label),
            )
        });
    }

    test_context.set_active_timeline(*timeline.name());

    // Set time cursor to tick 20 (mid-range).
    let store_id = test_context.active_store_id();
    test_context.send_time_commands(
        store_id,
        [TimeControlCommand::SetTime(
            re_log_types::TimeInt::new_temporal(20).into(),
        )],
    );
    test_context.handle_system_commands(&egui::Context::default());

    let view_id = setup_blueprint(&mut test_context);
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "states_basic",
        egui::vec2(500.0, 250.0),
        None,
    ));
}

#[test]
fn test_states_time_cursor() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StatesView>();

    let timeline = Timeline::log_tick();

    let state_data: Vec<(i64, &str, &str)> = vec![
        (0, "state/mode", "Idle"),
        (20, "state/mode", "Active"),
        (40, "state/mode", "Idle"),
    ];

    for (tick, entity, label) in &state_data {
        let timepoint = TimePoint::from([(timeline, *tick)]);
        test_context.log_entity(*entity, |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint,
                &re_sdk_types::archetypes::TextLog::new(*label),
            )
        });
    }

    test_context.set_active_timeline(*timeline.name());

    // Set time cursor to tick 30.
    let store_id = test_context.active_store_id();
    test_context.send_time_commands(
        store_id,
        [TimeControlCommand::SetTime(
            re_log_types::TimeInt::new_temporal(30).into(),
        )],
    );
    test_context.handle_system_commands(&egui::Context::default());

    let view_id = setup_blueprint(&mut test_context);
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "states_time_cursor",
        egui::vec2(400.0, 120.0),
        None,
    ));
}
