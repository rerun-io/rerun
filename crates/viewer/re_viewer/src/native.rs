use re_capabilities::MainThreadToken;
use re_log_types::LogMsg;

/// Used by `eframe` to decide where to store the app state.
pub const APP_ID: &str = "rerun";

type AppCreator = Box<dyn FnOnce(&eframe::CreationContext<'_>) -> Box<dyn eframe::App>>;

// NOTE: the name of this function is hard-coded in `crates/top/rerun/src/crash_handler.rs`!
pub fn run_native_app(
    // `eframe::run_native` may only be called on the main thread.
    _: crate::MainThreadToken,
    app_creator: AppCreator,
    force_wgpu_backend: Option<String>,
) -> eframe::Result {
    let native_options = eframe_options(force_wgpu_backend);

    let window_title = "Rerun Viewer";
    eframe::run_native(
        window_title,
        native_options,
        Box::new(move |cc| {
            crate::customize_eframe_and_setup_renderer(cc)?;
            Ok(app_creator(cc))
        }),
    )
}

pub fn eframe_options(force_wgpu_backend: Option<String>) -> eframe::NativeOptions {
    re_tracing::profile_function!();
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id(APP_ID) // Controls where on disk the app state is persisted
            .with_decorations(!re_ui::CUSTOM_WINDOW_DECORATIONS) // Maybe hide the OS-specific "chrome" around the window
            .with_fullsize_content_view(re_ui::FULLSIZE_CONTENT)
            .with_icon(icon_data())
            .with_inner_size([1600.0, 1200.0])
            .with_min_inner_size([320.0, 450.0]) // Should be high enough to fit the rerun menu
            .with_title_shown(!re_ui::FULLSIZE_CONTENT)
            .with_titlebar_buttons_shown(!re_ui::CUSTOM_WINDOW_DECORATIONS)
            .with_titlebar_shown(!re_ui::FULLSIZE_CONTENT)
            .with_transparent(re_ui::CUSTOM_WINDOW_DECORATIONS), // To have rounded corners without decorations we need transparency

        renderer: eframe::Renderer::Wgpu,
        wgpu_options: crate::wgpu_options(force_wgpu_backend),
        depth_buffer: 0,
        multisampling: 0, // the 3D views do their own MSAA

        ..Default::default()
    }
}

#[allow(clippy::unnecessary_wraps)]
fn icon_data() -> egui::IconData {
    re_tracing::profile_function!();

    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            let app_icon_png_bytes = include_bytes!("../data/app_icon_mac.png");
        } else if #[cfg(target_os = "windows")] {
            let app_icon_png_bytes = include_bytes!("../data/app_icon_windows.png");
        } else {
            // Use the same icon for X11 as for Windows, at least for now.
            let app_icon_png_bytes = include_bytes!("../data/app_icon_windows.png");
        }
    };

    // We include the .png with `include_bytes`. If that fails, things are extremely broken.
    match eframe::icon_data::from_png_bytes(app_icon_png_bytes) {
        Ok(icon_data) => icon_data,
        Err(err) => {
            #[cfg(debug_assertions)]
            panic!("Failed to load app icon: {err}");

            #[cfg(not(debug_assertions))]
            {
                re_log::warn!("Failed to load app icon: {err}");
                Default::default()
            }
        }
    }
}

pub fn run_native_viewer_with_messages(
    main_thread_token: MainThreadToken,
    build_info: re_build_info::BuildInfo,
    app_env: crate::AppEnvironment,
    startup_options: crate::StartupOptions,
    log_messages: Vec<LogMsg>,
) -> eframe::Result {
    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::Sdk,
        re_smart_channel::SmartChannelSource::Sdk,
    );
    for log_msg in log_messages {
        tx.send(log_msg).ok();
    }

    let force_wgpu_backend = startup_options.force_wgpu_backend.clone();
    run_native_app(
        main_thread_token,
        Box::new(move |cc| {
            let mut app = crate::App::new(
                main_thread_token,
                build_info,
                &app_env,
                startup_options,
                cc.egui_ctx.clone(),
                cc.storage,
            );
            app.add_receiver(rx);
            Box::new(app)
        }),
        force_wgpu_backend,
    )
}
