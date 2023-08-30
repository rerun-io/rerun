use re_log_types::LogMsg;

/// Used by `eframe` to decide where to store the app state.
pub const APP_ID: &str = "rerun";

type AppCreator =
    Box<dyn FnOnce(&eframe::CreationContext<'_>, re_ui::ReUi) -> Box<dyn eframe::App>>;

// NOTE: the name of this function is hard-coded in `crates/rerun/src/crash_handler.rs`!
pub fn run_native_app(app_creator: AppCreator) -> eframe::Result<()> {
    let native_options = eframe_options();

    let window_title = "Rerun Viewer";
    eframe::run_native(
        window_title,
        native_options,
        Box::new(move |cc| {
            let re_ui = crate::customize_eframe(cc);
            app_creator(cc, re_ui)
        }),
    )
}

pub fn eframe_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        // Controls where on disk the app state is persisted.
        app_id: Some(APP_ID.to_owned()),

        initial_window_size: Some([1600.0, 1200.0].into()),
        min_window_size: Some([320.0, 450.0].into()), // Should be high enough to fit the rerun menu

        icon_data: icon_data(),

        #[cfg(target_os = "macos")]
        fullsize_content: re_ui::FULLSIZE_CONTENT,

        // Maybe hide the OS-specific "chrome" around the window:
        decorated: !re_ui::CUSTOM_WINDOW_DECORATIONS,
        // To have rounded corners we need transparency:
        transparent: re_ui::CUSTOM_WINDOW_DECORATIONS,

        follow_system_theme: false,
        default_theme: eframe::Theme::Dark,

        renderer: eframe::Renderer::Wgpu,
        wgpu_options: crate::wgpu_options(),
        depth_buffer: 0,
        multisampling: 0, // the 3D views do their own MSAA

        ..Default::default()
    }
}

#[allow(clippy::unnecessary_wraps)]
fn icon_data() -> Option<eframe::IconData> {
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
    match eframe::IconData::try_from_png_bytes(app_icon_png_bytes) {
        Ok(icon_data) => Some(icon_data),
        Err(err) => {
            #[cfg(debug_assertions)]
            panic!("Failed to load app icon: {err}");

            #[cfg(not(debug_assertions))]
            {
                re_log::warn!("Failed to load app icon: {err}");
                None
            }
        }
    }
}

pub fn run_native_viewer_with_messages(
    build_info: re_build_info::BuildInfo,
    app_env: crate::AppEnvironment,
    startup_options: crate::StartupOptions,
    log_messages: Vec<LogMsg>,
) -> eframe::Result<()> {
    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::Sdk,
        re_smart_channel::SmartChannelSource::Sdk,
    );
    for log_msg in log_messages {
        tx.send(log_msg).ok();
    }
    run_native_app(Box::new(move |cc, re_ui| {
        let mut app = crate::App::new(build_info, &app_env, startup_options, re_ui, cc.storage);
        app.add_receiver(rx);
        Box::new(app)
    }))
}
