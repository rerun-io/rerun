//! This example shows how to add custom Views to the Rerun Viewer.

use rerun::external::{
    glam, re_crash_handler, re_grpc_server, re_log, re_memory, re_smart_channel,
    re_types::{self, View as _},
    re_viewer, tokio,
};

mod custom_archetype;
mod custom_renderer;
mod custom_visualizer;

use custom_visualizer::CustomVisualizer;

// By using `re_memory::AccountingAllocator` Rerun can keep track of exactly how much memory it is using,
// and prune the data store when it goes above a certain limit.
// By using `mimalloc` we get faster allocations.
#[global_allocator]
static GLOBAL: re_memory::AccountingAllocator<mimalloc::MiMalloc> =
    re_memory::AccountingAllocator::new(mimalloc::MiMalloc);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main_thread_token = re_viewer::MainThreadToken::i_promise_i_am_on_the_main_thread();

    // Direct calls using the `log` crate to stderr. Control with `RUST_LOG=debug` etc.
    re_log::setup_logging();

    // Install handlers for panics and crashes that prints to stderr and send
    // them to Rerun analytics (if the `analytics` feature is on in `Cargo.toml`).
    re_crash_handler::install_crash_handlers(re_viewer::build_info());

    // Listen for gRPC connections from Rerun's logging SDKs.
    // There are other ways of "feeding" the viewer though - all you need is a `re_smart_channel::Receiver`.
    let (grpc_rx, _) = re_grpc_server::spawn_with_recv(
        "0.0.0.0:9876".parse()?,
        "75%".parse()?,
        re_grpc_server::shutdown::never(),
    );

    // Provide a builtin recording with an example recording using the custom archetype.
    let builtin_recording_rx = builtin_recording()?;

    let startup_options = re_viewer::StartupOptions::default();

    // This is used for analytics, if the `analytics` feature is on in `Cargo.toml`
    let app_env = re_viewer::AppEnvironment::Custom("My extended Rerun Viewer".to_owned());

    println!(
        "This example starts a custom Rerun Viewer that is ready to accept data. But for convenience it comes with a built-in recording!"
    );
    println!(
        "You can connect through the SDK as per usual, for example to run: `cargo run -p minimal_options -- --connect` in another terminal instance."
    );

    re_viewer::run_native_app(
        main_thread_token,
        Box::new(move |cc| {
            let mut app = re_viewer::App::new(
                main_thread_token,
                re_viewer::build_info(),
                &app_env,
                startup_options,
                cc,
                re_viewer::AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().expect(
                    "Could not get a runtime handle from the current Tokio runtime or Wasm bindgen.",
                ),
            );
            app.add_log_receiver(grpc_rx);
            app.add_log_receiver(builtin_recording_rx);

            // Register a custom visualizer for the builtin 3D view.
            app.view_class_registry()
                .register_visualizer::<CustomVisualizer>(
                    re_types::blueprint::views::Spatial3DView::identifier(),
                )
                .unwrap();

            Box::new(app)
        }),
        None,
    )?;

    Ok(())
}

pub fn builtin_recording(
) -> Result<re_smart_channel::Receiver<rerun::log::LogMsg>, rerun::RecordingStreamError> {
    // TODO(andreas): Would be great if there was a log sink that's directly tied to a smartchannel
    // so that this could run in the background.
    let (rec, memory_sink) =
        rerun::RecordingStreamBuilder::new("rerun_example_custom_visualizer").memory()?;

    // Log an entity with two custom ???TODO??.
    rec.log_static(
        "custom",
        &custom_archetype::Custom::new([[0.0, 0.0, 0.0], [2.0, 2.0, 2.0]]).with_colors([
            rerun::Color::from_rgb(255, 0, 0),
            rerun::Color::from_rgb(0, 0, 255),
        ]),
    )?;

    // Log a solid box to demonstrate interaction of the custom ???TODO?? with existing view contents.
    rec.log_static(
        "box",
        &rerun::Boxes3D::from_half_sizes([[0.5, 0.5, 0.5]])
            .with_fill_mode(rerun::FillMode::Solid)
            .with_colors([rerun::Color::from_rgb(0, 255, 0)]),
    )?;

    // Move things around a little bit.
    for i in 0..(std::f32::consts::TAU * 100.0) as i32 {
        rec.set_duration_secs("time", i as f32 / 100.0);
        rec.log(
            "box",
            &rerun::Transform3D::from_rotation(glam::Quat::from_rotation_x(i as f32 / 100.0)),
        )?;
        rec.log(
            "custom",
            &rerun::Transform3D::from_rotation(glam::Quat::from_rotation_z(i as f32 / 100.0)),
        )?;
    }

    // Forward the content of the memory recording to a smartchannel.
    let (builtin_recording_tx, builtin_recording_rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::Sdk,
        re_smart_channel::SmartChannelSource::Sdk,
    );
    rec.flush_blocking();
    for msg in memory_sink.take() {
        builtin_recording_tx
            .send(msg)
            .expect("Failed to send message to builtin recording");
    }

    Ok(builtin_recording_rx)
}
