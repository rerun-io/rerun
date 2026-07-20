//! Logs test state data for the state timeline view.

use std::sync::Arc;

/// Build a `StateChange` whose state array can contain nulls.
///
/// A null entry resets that instance's state, showing a gap in its lane.
fn multi_state(states: &[Option<&str>]) -> rerun::StateChange {
    rerun::StateChange::new().with_state_opt(states.iter().copied())
}

fn main() -> anyhow::Result<()> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_state_timeline").spawn()?;

    // An example of a static annotation context. An edge case for the state timeline view.
    rec.log_static(
        "/",
        &rerun::AnnotationContext::new([
            (1, "person", rerun::Rgba32::from_rgb(220, 20, 60)),
            (2, "bicycle", rerun::Rgba32::from_rgb(119, 11, 32)),
            (3, "car", rerun::Rgba32::from_rgb(0, 0, 142)),
            (4, "motorcycle", rerun::Rgba32::from_rgb(0, 0, 230)),
            (5, "airplane", rerun::Rgba32::from_rgb(106, 0, 228)),
        ]),
    )?;

    // Base timestamp: 2025-04-01 12:00:00 UTC
    let base_ts: f64 = 1_743_508_800.0;
    let step_secs: f64 = 5.0;

    let states: Vec<(i64, &str, &str)> = vec![
        (0, "state/robot_mode", "1"),
        (10, "state/robot_mode", "2"),
        (25, "state/robot_mode", "3"),
        (40, "state/robot_mode", "1"),
        (0, "state/power", "On"),
        (20, "state/power", "Low"),
        (35, "state/power", "Critical"),
        (45, "state/power", "On"),
        (0, "state/connection", "Connected"),
        (15, "state/connection", "Disconnected"),
        (30, "state/connection", "Connected"),
    ];

    for (tick, entity, label) in &states {
        rec.set_time_sequence("tick", *tick);
        rec.set_timestamp_secs_since_epoch("timestamp", base_ts + *tick as f64 * step_secs);
        rec.log(*entity, &rerun::StateChange::single(*label))?;
    }

    // Multi-instance state: a gamepad's buttons logged as one state array, in the spirit of
    // ROS `sensor_msgs/Joy`. Each instance gets its own lane, grouped under a single label.
    // Every row is a full assignment of the array: `None` resets its instance (gap in that
    // lane), and the shorter row at tick 28 resets the omitted third button the same way.
    #[rustfmt::skip]
    let button_states: Vec<(i64, Vec<Option<&str>>)> = vec![
        (0,  vec![Some("Released"), Some("Released"), Some("Released")]),
        (5,  vec![Some("Pressed"),  Some("Released"), Some("Released")]),
        (12, vec![Some("Pressed"),  Some("Pressed"),  Some("Released")]),
        (18, vec![Some("Released"), None,             Some("Pressed")]),
        (28, vec![Some("Released"), Some("Released")]),
        (38, vec![Some("Pressed"),  Some("Pressed"),  Some("Pressed")]),
        (46, vec![Some("Pressed"),  Some("Released"), Some("Released")]),
    ];
    for (tick, states) in &button_states {
        rec.set_time_sequence("tick", *tick);
        rec.set_timestamp_secs_since_epoch("timestamp", base_ts + *tick as f64 * step_secs);
        rec.log("state/gamepad_buttons", &multi_state(states))?;
    }

    // One shared configuration styles every instance lane of the group.
    rec.log_static(
        "state/gamepad_buttons",
        &rerun::StateConfiguration::new()
            .with_values(["Pressed", "Released"])
            .with_colors([
                rerun::Rgba32::from_rgb(239, 83, 80),
                rerun::Rgba32::from_rgb(76, 175, 80),
            ]),
    )?;

    // Log an alternative string component on robot_mode via DynamicArchetype.
    // This allows switching the state source in the source selector dropdown.
    let alt_states: Vec<(i64, &str)> =
        vec![(0, "IDLE"), (10, "MOVING"), (25, "WORK"), (40, "NOPE")];
    for (tick, state) in &alt_states {
        rec.set_time_sequence("tick", *tick);
        rec.set_timestamp_secs_since_epoch("timestamp", base_ts + 2.0 * *tick as f64 * step_secs);
        rec.log(
            "state/robot_mode",
            &rerun::DynamicArchetype::new("sensor_data").with_component_from_data(
                "state",
                Arc::new(arrow::array::StringArray::from(vec![*state])),
            ),
        )?;
    }

    // Log a boolean signal as an alternative state source — the user can remap it onto the
    // state slot via the source selector. Exercises the polymorphic state cast (Bool
    // passthrough) and the simplified true/false editor in the selection panel.
    let bool_states: Vec<(i64, bool)> = vec![
        (0, true),
        (8, false),
        (15, false),
        (22, false),
        (30, true),
        (38, false),
        (45, true),
    ];
    for (tick, state) in &bool_states {
        rec.set_time_sequence("tick", *tick);
        rec.set_timestamp_secs_since_epoch("timestamp", base_ts + *tick as f64 * step_secs);
        rec.log(
            "state/heartbeat",
            &rerun::DynamicArchetype::new("heartbeat_signal").with_component_from_data(
                "alive",
                Arc::new(arrow::array::BooleanArray::from(vec![*state])),
            ),
        )?;
    }

    // Log scalar data on the same timelines so a time series view can be added.
    for tick in 0..50 {
        let t = tick as f64;
        rec.set_time_sequence("tick", tick);
        rec.set_timestamp_secs_since_epoch("timestamp", base_ts + t * step_secs);
        rec.log("scalar/sine", &rerun::Scalars::new([f64::sin(t * 0.3)]))?;
    }

    // Bad data: changes component type from string to boolean.
    rec.set_time_sequence("tick", 1);
    rec.log(
        "foo",
        &rerun::DynamicArchetype::new("bar").with_component_from_data(
            "state",
            Arc::new(arrow::array::StringArray::from(vec!["ponies"])),
        ),
    )?;
    rec.set_time_sequence("tick", 2);
    rec.log(
        "foo",
        &rerun::DynamicArchetype::new("bar").with_component_from_data(
            "state",
            Arc::new(arrow::array::BooleanArray::from(vec![true])),
        ),
    )?;

    let _ = rec.flush_blocking();

    Ok(())
}
