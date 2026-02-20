//! This example shows how to add custom visualizers to the Rerun Viewer.
//!
//! It defines a `HeightField` archetype that stores a 2D grid of height values
//! as an image buffer, and a custom visualizer + GPU renderer that dynamically
//! generates a 3D triangle mesh from the heightfield data each frame, with
//! GPU-side colormap application.

use rerun::external::re_sdk_types::{self, View as _};
use rerun::external::{
    glam, re_crash_handler, re_grpc_server, re_log, re_log_channel, re_memory, re_viewer, tokio,
};

mod height_field_archetype;
mod height_field_renderer;
mod height_field_visualizer;

use height_field_visualizer::HeightFieldVisualizer;

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
    // There are other ways of "feeding" the viewer though - all you need is a `re_log_channel::LogReceiver`.
    let grpc_rx = re_grpc_server::spawn_with_recv(
        "0.0.0.0:9876".parse()?,
        re_grpc_server::ServerOptions::default(),
        re_grpc_server::shutdown::never(),
    );

    // Provide a builtin recording with an example recording using the custom archetype.
    let builtin_recording_rx = builtin_recording()?;

    let startup_options = re_viewer::StartupOptions::default();

    // This is used for analytics, if the `analytics` feature is on in `Cargo.toml`
    let app_env = re_viewer::AppEnvironment::Custom("My extended Rerun Viewer".to_owned());

    println!("This example starts a custom Rerun Viewer with a built-in recording.");
    println!(
        "You can also connect through the SDK, e.g.: `cargo run -p minimal_options -- --connect`"
    );

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
            app.add_log_receiver(grpc_rx);
            app.add_log_receiver(builtin_recording_rx);

            // Register the custom heightfield visualizer for the builtin 3D view.
            app.extend_view_class(
                re_sdk_types::blueprint::views::Spatial3DView::identifier(),
                |registrator| {
                    registrator.register_visualizer::<HeightFieldVisualizer>()?;

                    // Register fallback provider for the colormap, so the visualizer UI is in sync with the visualizer's internal default.
                    registrator.register_fallback_provider(
                        height_field_archetype::HeightField::descriptor_colormap().component,
                        |_ctx| height_field_visualizer::DEFAULT_COLOR_MAP,
                    );

                    Ok(())
                },
            )
            .unwrap();

            Ok(Box::new(app))
        }),
        None,
    )?;

    Ok(())
}

fn builtin_recording() -> Result<re_log_channel::LogReceiver, rerun::RecordingStreamError> {
    let (rec, memory_sink) =
        rerun::RecordingStreamBuilder::new("rerun_example_custom_visualizer").memory()?;

    // Generate animated 512x512 heightfield spanning 10x10 metres with rippling terrain.
    let cols = 512u32;
    let rows = 512u32;
    let num_terrain_frames = 60;
    let format = rerun::components::ImageFormat(rerun::datatypes::ImageFormat::from_color_model(
        [cols, rows],
        rerun::datatypes::ColorModel::L,
        rerun::datatypes::ChannelDatatype::F32,
    ));

    for frame in 0..num_terrain_frames {
        let t = frame as f32 / num_terrain_frames as f32 * std::f32::consts::TAU;
        // Blend from fully voxelized (0.0) to fully smooth (1.0).
        let smoothness = frame as f32 / (num_terrain_frames - 1) as f32;
        rec.set_duration_secs("time", t);

        let mut heights: Vec<f32> = Vec::with_capacity((rows * cols) as usize);
        for row in 0..rows {
            for col in 0..cols {
                let voxel_size = 0.25;
                let x = col as f32 / (cols - 1) as f32 * 10.0;
                let z = row as f32 / (rows - 1) as f32 * 10.0;
                let xq = (x / voxel_size).floor() * voxel_size;
                let zq = (z / voxel_size).floor() * voxel_size;

                // Blend coordinates from quantized to continuous.
                let xb = xq + (x - xq) * smoothness;
                let zb = zq + (z - zq) * smoothness;

                let h_raw = (xb * 0.5 + t).sin() * (zb * 0.4 + t * 0.7).cos() * 0.3
                    + (xb * 1.1 + zb * 0.7 + t * 1.3).sin() * 0.15
                    + (xb * 2.5 + t * 0.5).cos() * (zb * 2.0 + t * 0.8).sin() * 0.06
                    + ((xb - 5.0).powi(2) + (zb - 5.0).powi(2)).sqrt().cos() * 0.12
                    + (xb * 5.0 + zb * 4.0 + t * 2.0).sin() * 0.03;
                let h_snapped = (h_raw / voxel_size).round() * voxel_size;
                // Blend height from snapped to raw.
                let h = h_snapped + (h_raw - h_snapped) * smoothness;
                heights.push(h);
            }
        }

        let height_bytes: &[u8] = bytemuck::cast_slice(&heights);
        let buffer = rerun::components::ImageBuffer(height_bytes.to_vec().into());
        rec.log(
            "terrain",
            &height_field_archetype::HeightField::new(buffer, format),
        )?;
    }

    // Log a solid box that orbits the terrain for reference.
    rec.log_static(
        "box",
        &rerun::Boxes3D::from_half_sizes([[0.1, 0.3, 0.1]])
            .with_fill_mode(rerun::FillMode::Solid)
            .with_colors([rerun::Color::from_rgb(255, 100, 50)]),
    )?;

    for i in 0..(std::f32::consts::TAU * 100.0) as i32 {
        rec.set_duration_secs("time", i as f32 / 100.0);
        rec.log(
            "box",
            &rerun::Transform3D::from_rotation(glam::Quat::from_rotation_z(i as f32 / 100.0))
                .with_translation([5.0, 5.0, 1.0]),
        )?;
    }

    // Forward the content of the memory recording to a log channel.
    let (builtin_recording_tx, builtin_recording_rx) =
        re_log_channel::log_channel(re_log_channel::LogSource::Sdk);
    rec.flush_blocking().ok();
    for msg in memory_sink.take() {
        builtin_recording_tx
            .send(re_log_channel::DataSourceMessage::LogMsg(msg))
            .expect("Failed to send message to builtin recording");
    }

    Ok(builtin_recording_rx)
}
