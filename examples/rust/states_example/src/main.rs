//! Example app that opens a Rerun Viewer with the States view showing test state data.

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
    let app_env = re_viewer::AppEnvironment::Custom("States view example".to_owned());

    // Log some state data via SDK so the States view has something to show.
    log_state_data()?;

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

fn log_state_data() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_states")
        .default_enabled(true)
        .connect_grpc()
        .map_err(|err| format!("Failed to connect: {err}"))?;

    let states: Vec<(i64, &str, &str)> = vec![
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

    for (tick, entity, label) in states {
        rec.set_time_sequence("tick", tick);
        rec.log(entity, &rerun::TextLog::new(label))?;
    }

    let _ = rec.flush_blocking();

    Ok(())
}
