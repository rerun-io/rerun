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
            check_graphics_driver(cc.wgpu_render_state.as_ref());
            let re_ui = crate::customize_eframe(cc);
            app_creator(cc, re_ui)
        }),
    )
}

fn check_graphics_driver(wgpu_render_state: Option<&egui_wgpu::RenderState>) {
    re_tracing::profile_function!();
    let wgpu_render_state = wgpu_render_state.expect("Expected wgpu to be enabled");
    let info = wgpu_render_state.adapter.get_info();

    let human_readable_summary = {
        let wgpu::AdapterInfo {
            name,
            vendor: _, // skip integer id
            device: _, // skip integer id
            device_type,
            driver,
            driver_info,
            backend,
        } = &info;

        // Example outputs:
        // > wgpu adapter name: "llvmpipe (LLVM 16.0.6, 256 bits)", device_type: Cpu, backend: Vulkan, driver: "llvmpipe", driver_info: "Mesa 23.1.6-arch1.4 (LLVM 16.0.6)"
        // > wgpu adapter name: "Apple M1 Pro", device_type: IntegratedGpu, backend: Metal, driver: "", driver_info: ""

        format!(
            "wgpu adapter name: {name:?}, \
             device_type: {device_type:?}, \
             backend: {backend:?}, \
             driver: {driver:?}, \
             driver_info: {driver_info:?}"
        )
    };

    let is_software_rasterizer_with_known_crashes = {
        // See https://github.com/rerun-io/rerun/issues/3089
        const KNOWN_SOFTWARE_RASTERIZERS: &[&str] = &[
            "lavapipe", // Vulkan software rasterizer
            "llvmpipe", // OpenGL software rasterizer
        ];

        // I'm not sure where the incriminating string will appear, so check all fields at once:
        let info_string = format!("{info:?}").to_lowercase();

        KNOWN_SOFTWARE_RASTERIZERS
            .iter()
            .any(|&software_rasterizer| info_string.contains(software_rasterizer))
    };

    if is_software_rasterizer_with_known_crashes {
        re_log::warn!("Software rasterizer detected - expect poor performance and crashes. See: https://www.rerun.io/docs/getting-started/troubleshooting#graphics-issues");
        re_log::info!("{human_readable_summary}");
    } else if info.device_type == wgpu::DeviceType::Cpu {
        re_log::warn!("Software rasterizer detected - expect poor performance. See: https://www.rerun.io/docs/getting-started/troubleshooting#graphics-issues");
        re_log::info!("{human_readable_summary}");
    } else {
        re_log::debug!("{human_readable_summary}");
    }
}

pub fn eframe_options() -> eframe::NativeOptions {
    re_tracing::profile_function!();
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id(APP_ID) // Controls where on disk the app state is persisted
            .with_decorations(!re_ui::CUSTOM_WINDOW_DECORATIONS) // Maybe hide the OS-specific "chrome" around the window
            .with_fullsize_content_view(re_ui::FULLSIZE_CONTENT)
            .with_icon(icon_data())
            .with_inner_size([1600.0, 1200.0])
            .with_min_inner_size([320.0, 450.0]) // Should be high enough to fit the rerun menu
            .with_transparent(re_ui::CUSTOM_WINDOW_DECORATIONS), // To have rounded corners we need transparency

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
