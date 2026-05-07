//! Example app that opens a Rerun Viewer with the Status view showing test state data.

use std::sync::Arc;

use rerun::external::{re_crash_handler, re_grpc_server, re_log, re_viewer, tokio};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main_thread_token = rerun::MainThreadToken::i_promise_i_am_on_the_main_thread();

    re_log::setup_logging();
    re_crash_handler::install_crash_handlers(re_viewer::build_info());

    // Listen for gRPC connections.
    let rx = re_grpc_server::spawn_with_recv(
        "0.0.0.0:9876".parse()?,
        Default::default(),
        re_grpc_server::shutdown::never(),
    );

    let startup_options = re_viewer::StartupOptions::default();
    let app_env = re_viewer::AppEnvironment::Custom("Status view example".to_owned());

    // Log some status data via SDK so the Status view has something to show.
    log_status_data()?;

    re_viewer::run_native_app(
        main_thread_token,
        Box::new(move |cc| {
            let mut app = re_viewer::App::new(
                main_thread_token,
                re_viewer::build_info(),
                app_env,
                startup_options,
                cc,
                None,
                re_viewer::AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().expect(
                    "Could not get a runtime handle from the current Tokio runtime or Wasm bindgen.",
                ),
            );
            app.add_log_receiver(rx);
            Ok(Box::new(app))
        }),
        None,
    )?;

    Ok(())
}

fn log_status_data() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_status")
        .default_enabled(true)
        .connect_grpc()
        .map_err(|err| format!("Failed to connect: {err}"))?;

    // An example of a static annotation context. An edge case for the status view.
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
        rec.log(*entity, &rerun::Status::new().with_status(*label))?;
    }

    // Log an alternative string component on robot_mode via DynamicArchetype.
    // This allows switching the state source in the source selector dropdown.
    let alt_states: Vec<(i64, &str)> =
        vec![(0, "IDLE"), (10, "MOVING"), (25, "WORK"), (40, "NOPE")];
    for (tick, status) in &alt_states {
        rec.set_time_sequence("tick", *tick);
        rec.set_timestamp_secs_since_epoch("timestamp", base_ts + 2.0 * *tick as f64 * step_secs);
        rec.log(
            "state/robot_mode",
            &rerun::DynamicArchetype::new("sensor_data").with_component_from_data(
                "status",
                Arc::new(arrow::array::StringArray::from(vec![*status])),
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

    let _ = rec.flush_blocking();

    Ok(())
}
