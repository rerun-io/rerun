//! Android entry point for the Rerun Viewer.
//!
//! This module provides the `android_main` function that serves as the entry point
//! when the viewer is launched as an Android application using the GameActivity backend.

use re_capabilities::MainThreadToken;
use re_viewer_context::AsyncRuntimeHandle;

/// The Android entry point.
///
/// This is called by the Android GameActivity runtime when the app is launched.
/// It sets up logging, creates a tokio runtime, configures eframe for Android,
/// and launches the Rerun Viewer.
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
    let tokio_runtime =
        tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let async_runtime = AsyncRuntimeHandle::new_native(tokio_runtime.handle().clone());

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
        expect_data_soon: None,
        // Force Vulkan on Android (the primary graphics API for Android).
        force_wgpu_backend: Some("vulkan".to_owned()),
        video_decoder_hw_acceleration: None,
        on_event: None,
        panel_state_overrides: Default::default(),
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

            let app = crate::App::new(
                main_thread_token,
                build_info,
                app_env,
                startup_options,
                cc,
                None, // No connection registry on Android (for now)
                async_runtime,
            );
            Ok(Box::new(app))
        }),
    )
    .unwrap_or_else(|err| {
        log::error!("Failed to run Rerun Viewer on Android: {err:?}");
    });
}
