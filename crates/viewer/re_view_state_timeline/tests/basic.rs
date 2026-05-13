use re_chunk_store::RowId;
use re_log_types::{TimePoint, Timeline};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_view_state_timeline::StateTimelineView;
use re_viewer_context::{TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            StateTimelineView::identifier(),
        ))
    })
}

// TODO(RR-4254): Add a test for multiple state change instances.

#[test]
fn test_state_timeline_basic() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();

    let timeline = Timeline::log_tick();

    // Log state transitions for multiple entities using StateChange.
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

    for (tick, entity, state) in &state_data {
        let timepoint = TimePoint::from([(timeline, *tick)]);
        test_context.log_entity(*entity, |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint,
                &re_sdk_types::archetypes::StateChange::new().with_state(*state),
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
        "state_timeline_basic",
        egui::vec2(500.0, 250.0),
        None,
    ));
}

#[test]
fn test_state_timeline_time_cursor() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();

    let timeline = Timeline::log_tick();

    let state_data: Vec<(i64, &str, &str)> = vec![
        (0, "state/mode", "Idle"),
        (20, "state/mode", "Active"),
        (40, "state/mode", "Idle"),
    ];

    for (tick, entity, state) in &state_data {
        let timepoint = TimePoint::from([(timeline, *tick)]);
        test_context.log_entity(*entity, |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint,
                &re_sdk_types::archetypes::StateChange::new().with_state(*state),
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
        "state_timeline_time_cursor",
        egui::vec2(400.0, 120.0),
        None,
    ));
}

/// A null state is a fallthrough: it must not terminate the preceding phase.
#[test]
fn test_state_timeline_null_is_fallthrough() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();

    let timeline = Timeline::log_tick();

    // Log a state, then a null in the middle, then another state.
    // The null should be ignored so that the first phase extends all the way
    // until the next non-null state.
    let timepoint_0 = TimePoint::from([(timeline, 0)]);
    test_context.log_entity("state/mode", |builder| {
        builder.with_archetype(
            RowId::new(),
            timepoint_0,
            &re_sdk_types::archetypes::StateChange::new().with_state("Idle"),
        )
    });

    let timepoint_20 = TimePoint::from([(timeline, 20)]);
    let null_state_array =
        <re_sdk_types::components::Text as re_sdk_types::external::re_types_core::Loggable>::to_arrow_opt(
            [None::<re_sdk_types::components::Text>],
        )
        .expect("serializing a single null text should not fail");
    let null_state = re_sdk_types::archetypes::StateChange {
        state: Some(re_sdk_types::SerializedComponentBatch::new(
            null_state_array,
            re_sdk_types::archetypes::StateChange::descriptor_state(),
        )),
    };
    test_context.log_entity("state/mode", |builder| {
        builder.with_archetype(RowId::new(), timepoint_20, &null_state)
    });

    let timepoint_40 = TimePoint::from([(timeline, 40)]);
    test_context.log_entity("state/mode", |builder| {
        builder.with_archetype(
            RowId::new(),
            timepoint_40,
            &re_sdk_types::archetypes::StateChange::new().with_state("Active"),
        )
    });

    test_context.set_active_timeline(*timeline.name());

    // Place the cursor in the null region to confirm the previous phase
    // visibly extends through the null.
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
        "state_timeline_null_is_fallthrough",
        egui::vec2(400.0, 120.0),
        None,
    ));
}

/// Log data on both a sequence and a timestamp timeline, switch between them,
/// and verify the time axis labels update to match the active timeline.
#[test]
fn test_state_timeline_timeline_switch() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();

    let seq_timeline = Timeline::log_tick();
    // Base timestamp: 2025-04-01 12:00:00 UTC (in nanoseconds since epoch)
    let base_ns: i64 = 1_743_508_800_000_000_000;
    let step_ns: i64 = 5_000_000_000; // 5 seconds
    let ts_timeline = Timeline::new_timestamp("timestamp");

    let state_data: Vec<(i64, &str, &str)> = vec![
        (0, "state/robot_mode", "Idle"),
        (10, "state/robot_mode", "Moving"),
        (25, "state/robot_mode", "Working"),
        (40, "state/robot_mode", "Idle"),
        (0, "state/power", "On"),
        (20, "state/power", "Low"),
        (35, "state/power", "Critical"),
        (45, "state/power", "On"),
    ];

    for (tick, entity, state) in &state_data {
        let timepoint = TimePoint::from([
            (seq_timeline, *tick),
            (ts_timeline, base_ns + *tick * step_ns),
        ]);
        test_context.log_entity(*entity, |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint,
                &re_sdk_types::archetypes::StateChange::new().with_state(*state),
            )
        });
    }

    let view_id = setup_blueprint(&mut test_context);
    let egui_ctx = egui::Context::default();

    // Snapshot with the sequence timeline active.
    test_context.set_active_timeline(*seq_timeline.name());
    let store_id = test_context.active_store_id();
    test_context.send_time_commands(
        store_id.clone(),
        [TimeControlCommand::SetTime(
            re_log_types::TimeInt::new_temporal(20).into(),
        )],
    );
    test_context.handle_system_commands(&egui_ctx);
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "state_timeline_timeline_switch_sequence",
        egui::vec2(500.0, 200.0),
        None,
    ));

    // Switch to the timestamp timeline and snapshot again.
    test_context.set_active_timeline(*ts_timeline.name());
    test_context.send_time_commands(
        store_id,
        [TimeControlCommand::SetTime(
            re_log_types::TimeInt::new_temporal(base_ns + 20 * step_ns).into(),
        )],
    );
    test_context.handle_system_commands(&egui_ctx);
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "state_timeline_timeline_switch_timestamp",
        egui::vec2(500.0, 200.0),
        None,
    ));
}

/// `StateConfiguration` overrides the label, color, and visibility per raw state value.
///
/// This test logs three raw values, then logs a `StateConfiguration` that renames two of them,
/// recolors one, and hides another. The snapshot verifies the overrides apply end-to-end.
#[test]
fn test_state_configuration() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();

    let timeline = Timeline::log_tick();

    let state_data: Vec<(i64, &str)> =
        vec![(0, "Idle"), (10, "Moving"), (25, "Hidden"), (40, "Idle")];
    for (tick, state) in &state_data {
        let timepoint = TimePoint::from([(timeline, *tick)]);
        test_context.log_entity("state/robot_mode", |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint,
                &re_sdk_types::archetypes::StateChange::new().with_state(*state),
            )
        });
    }

    // Configure labels/colors/visibility. `Hidden` is marked not visible and
    // should not be drawn; `Moving` is relabeled and recolored.
    test_context.log_entity("state/robot_mode", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::StateConfiguration::new()
                .with_values(["Idle", "Moving", "Hidden"])
                .with_labels(["At rest", "In motion", "Hidden"])
                .with_colors([0x4CAF50FFu32, 0x42A5F5FFu32, 0xAB47BCFFu32])
                .with_visible([true, true, false]),
        )
    });

    test_context.set_active_timeline(*timeline.name());

    let view_id = setup_blueprint(&mut test_context);
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "state_configuration",
        egui::vec2(500.0, 120.0),
        None,
    ));
}

/// When phases are too narrow to render individually, consecutive narrow phases
/// should be merged into a flat gray region. Wide phases on a separate lane
/// remain rendered with their own colors.
#[test]
fn test_state_timeline_merge_small_phases() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();

    let timeline = Timeline::log_tick();

    // Lane 1: many tightly-packed phases that should collapse into a merged region.
    let dense_values = ["A", "B", "C"];
    for tick in 0..200i64 {
        let timepoint = TimePoint::from([(timeline, tick)]);
        let state = dense_values[(tick as usize) % dense_values.len()];
        test_context.log_entity("state/dense", |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint,
                &re_sdk_types::archetypes::StateChange::new().with_state(state),
            )
        });
    }

    // Lane 2: a few wide phases that should render normally.
    let sparse_data: Vec<(i64, &str)> = vec![(0, "Idle"), (60, "Active"), (130, "Idle")];
    for (tick, state) in &sparse_data {
        let timepoint = TimePoint::from([(timeline, *tick)]);
        test_context.log_entity("state/sparse", |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint,
                &re_sdk_types::archetypes::StateChange::new().with_state(*state),
            )
        });
    }

    test_context.set_active_timeline(*timeline.name());

    let view_id = setup_blueprint(&mut test_context);
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "state_timeline_merge_small_phases",
        egui::vec2(400.0, 150.0),
        None,
    ));
}

/// Cmd+scroll over the state timeline view should zoom in around the pointer.
#[test]
fn test_state_timeline_zoom() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();

    let timeline = Timeline::new_sequence("tick");

    let state_data: Vec<(i64, &str, &str)> = vec![
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

    for (tick, entity, state) in &state_data {
        let timepoint = TimePoint::from([(timeline, *tick)]);
        test_context.log_entity(*entity, |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint,
                &re_sdk_types::archetypes::StateChange::new().with_state(*state),
            )
        });
    }

    test_context.set_active_timeline(*timeline.name());

    let view_id = setup_blueprint(&mut test_context);

    let size = egui::vec2(800.0, 400.0);
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    // Let the view auto-fit and settle.
    harness.run();
    snapshot_results.add(harness.try_snapshot("state_timeline_zoom_before"));

    // Cmd+scroll over the center of the view to zoom in. `handle_pan_zoom`
    // only zooms when the pointer is hovering over the view, so we hover first.
    let center = egui::pos2(size.x * 0.5, size.y * 0.5);
    harness.hover_at(center);
    for _ in 0..5 {
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, 1.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::COMMAND,
        });
        harness.run();
    }

    snapshot_results.add(harness.try_snapshot("state_timeline_zoom_after"));
}
