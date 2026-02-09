//! Android entry point for the Rerun Viewer.
//!
//! This module provides the `android_main` function that serves as the entry point
//! when the viewer is launched as an Android application using the GameActivity backend.
//!
//! The viewer spawns a gRPC server on the device so that Rerun SDKs (Python, Rust, C++)
//! can stream data directly to the Android viewer over the network.

use re_capabilities::MainThreadToken;
use re_viewer_context::AsyncRuntimeHandle;

/// Default port for the gRPC server on Android.
const GRPC_PORT: u16 = re_grpc_server::DEFAULT_SERVER_PORT;

/// The Android entry point.
///
/// This is called by the Android GameActivity runtime when the app is launched.
/// It sets up logging, creates a tokio runtime, spawns a gRPC server for incoming
/// SDK connections, configures eframe for Android, and launches the Rerun Viewer.
///
/// # Safety
///
/// This function uses `#[no_mangle]` to export the symbol for the Android runtime.
/// It must only be called by the Android GameActivity infrastructure.
#[expect(unsafe_code, reason = "Required for Android entry point export")]
#[unsafe(no_mangle)]
unsafe fn android_main(app: winit::platform::android::activity::AndroidApp) {
    // Initialize Android logging so that `log` macros go to logcat.
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    // The android_main entry point runs on the main thread.
    let main_thread_token = MainThreadToken::i_promise_i_am_on_the_main_thread();

    // Create a tokio runtime for async operations (gRPC, data loading, etc.).
    let tokio_runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(err) => {
            log::error!("Failed to create tokio runtime: {err}");
            return;
        }
    };
    let _guard = tokio_runtime.enter();
    let async_runtime = AsyncRuntimeHandle::new_native(tokio_runtime.handle().clone());

    // --- Spawn the gRPC server so SDKs can stream data to this device ---
    let server_addr: std::net::SocketAddr = ([0, 0, 0, 0], GRPC_PORT).into();
    let server_options = re_grpc_server::ServerOptions {
        memory_limit: re_memory::MemoryLimit::from_fraction_of_total(0.25),
        ..Default::default()
    };

    let log_rx = re_grpc_server::spawn_with_recv(
        server_addr,
        server_options,
        re_grpc_server::shutdown::never(),
    );

    let build_info = crate::build_info();
    let app_env = crate::AppEnvironment::Custom("Android".to_owned());

    let startup_options = crate::StartupOptions {
        // Android devices typically have less RAM; use a conservative memory limit.
        memory_limit: re_memory::MemoryLimit::from_fraction_of_total(0.5),
        persist_state: true,
        is_in_notebook: false,
        screenshot_to_path_then_quit: None,
        hide_welcome_screen: false,
        detach_process: false,
        resolution_in_points: None,
        // Hint that data will arrive soon (SDK will connect via gRPC).
        expect_data_soon: Some(true),
        // Force Vulkan on Android (the primary graphics API for Android).
        force_wgpu_backend: Some("vulkan".to_owned()),
        video_decoder_hw_acceleration: None,
        on_event: None,
        panel_state_overrides: Default::default(),
        viewer_base_url: None,
    };

    let native_options = eframe::NativeOptions {
        android_app: Some(app),
        viewport: egui::ViewportBuilder::default()
            .with_app_id(crate::native::APP_ID)
            .with_decorations(false)
            .with_fullscreen(true)
            .with_min_inner_size([320.0, 450.0]),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: crate::wgpu_options(Some("vulkan")),
        depth_buffer: 0,
        multisampling: 0,
        ..Default::default()
    };

    eframe::run_native(
        "Rerun Viewer",
        native_options,
        Box::new(move |cc| {
            crate::customize_eframe_and_setup_renderer(cc)?;

            // Apply Android-specific touch-friendly style adjustments.
            crate::ui::android_ui::apply_android_style(&cc.egui_ctx);

            let mut app = crate::App::new(
                main_thread_token,
                build_info,
                app_env,
                startup_options,
                cc,
                None,
                async_runtime,
            );

            // Wire the gRPC server receiver into the viewer.
            app.add_log_receiver(log_rx);

            Ok(Box::new(app))
        }),
    )
    .unwrap_or_else(|err| {
        log::error!("Failed to run Rerun Viewer on Android: {err:?}");
    });
}
