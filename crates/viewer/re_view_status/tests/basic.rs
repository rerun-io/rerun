use re_chunk_store::RowId;
use re_log_types::{TimePoint, Timeline};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_view_status::StatusView;
use re_viewer_context::{TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            StatusView::identifier(),
        ))
    })
}

// TODO(RR-4254): Add a test for multiple status instances.

#[test]
fn test_status_basic() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StatusView>();

    let timeline = Timeline::log_tick();

    // Log state transitions for multiple entities using Status.
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

    for (tick, entity, status) in &state_data {
        let timepoint = TimePoint::from([(timeline, *tick)]);
        test_context.log_entity(*entity, |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint,
                &re_sdk_types::archetypes::Status::new().with_status(*status),
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
        "status_basic",
        egui::vec2(500.0, 250.0),
        None,
    ));
}

#[test]
fn test_status_time_cursor() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StatusView>();

    let timeline = Timeline::log_tick();

    let state_data: Vec<(i64, &str, &str)> = vec![
        (0, "state/mode", "Idle"),
        (20, "state/mode", "Active"),
        (40, "state/mode", "Idle"),
    ];

    for (tick, entity, status) in &state_data {
        let timepoint = TimePoint::from([(timeline, *tick)]);
        test_context.log_entity(*entity, |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint,
                &re_sdk_types::archetypes::Status::new().with_status(*status),
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
        "status_time_cursor",
        egui::vec2(400.0, 120.0),
        None,
    ));
}

/// A null status is a fallthrough: it must not terminate the preceding phase.
#[test]
fn test_status_null_is_fallthrough() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StatusView>();

    let timeline = Timeline::log_tick();

    // Log a status, then a null in the middle, then another status.
    // The null should be ignored so that the first phase extends all the way
    // until the next non-null status.
    let timepoint_0 = TimePoint::from([(timeline, 0)]);
    test_context.log_entity("state/mode", |builder| {
        builder.with_archetype(
            RowId::new(),
            timepoint_0,
            &re_sdk_types::archetypes::Status::new().with_status("Idle"),
        )
    });

    let timepoint_20 = TimePoint::from([(timeline, 20)]);
    let null_status_array =
        <re_sdk_types::components::Text as re_sdk_types::external::re_types_core::Loggable>::to_arrow_opt(
            [None::<re_sdk_types::components::Text>],
        )
        .expect("serializing a single null text should not fail");
    let null_status = re_sdk_types::archetypes::Status {
        status: Some(re_sdk_types::SerializedComponentBatch::new(
            null_status_array,
            re_sdk_types::archetypes::Status::descriptor_status(),
        )),
    };
    test_context.log_entity("state/mode", |builder| {
        builder.with_archetype(RowId::new(), timepoint_20, &null_status)
    });

    let timepoint_40 = TimePoint::from([(timeline, 40)]);
    test_context.log_entity("state/mode", |builder| {
        builder.with_archetype(
            RowId::new(),
            timepoint_40,
            &re_sdk_types::archetypes::Status::new().with_status("Active"),
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
        "status_null_is_fallthrough",
        egui::vec2(400.0, 120.0),
        None,
    ));
}

/// Log data on both a sequence and a timestamp timeline, switch between them,
/// and verify the time axis labels update to match the active timeline.
#[test]
fn test_status_timeline_switch() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StatusView>();

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

    for (tick, entity, status) in &state_data {
        let timepoint = TimePoint::from([
            (seq_timeline, *tick),
            (ts_timeline, base_ns + *tick * step_ns),
        ]);
        test_context.log_entity(*entity, |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint,
                &re_sdk_types::archetypes::Status::new().with_status(*status),
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
        "status_timeline_switch_sequence",
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
        "status_timeline_switch_timestamp",
        egui::vec2(500.0, 200.0),
        None,
    ));
}

/// Cmd+scroll over the Status view should zoom in around the pointer.
#[test]
fn test_status_zoom() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StatusView>();

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

    for (tick, entity, status) in &state_data {
        let timepoint = TimePoint::from([(timeline, *tick)]);
        test_context.log_entity(*entity, |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint,
                &re_sdk_types::archetypes::Status::new().with_status(*status),
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
    snapshot_results.add(harness.try_snapshot("status_zoom_before"));

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

    snapshot_results.add(harness.try_snapshot("status_zoom_after"));
}
