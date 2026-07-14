/// Used by `eframe` to decide where to store the app state.
pub const APP_ID: &str = "rerun";

type DynError = Box<dyn std::error::Error + Send + Sync>;

type AppCreator =
    Box<dyn FnOnce(&eframe::CreationContext<'_>) -> Result<Box<dyn eframe::App>, DynError>>;

// NOTE: the name of this function is hard-coded in `crates/top/rerun/src/crash_handler.rs`!
pub fn run_native_app(
    // `eframe::run_native` may only be called on the main thread.
    _: crate::MainThreadToken,
    app_creator: AppCreator,
    force_wgpu_backend: Option<&str>,
) -> eframe::Result {
    if crate::docker_detection::is_docker() {
        re_log::warn_once!(
            "It looks like you are running the Rerun Viewer inside a Docker container. This is not officially supported, and may lead to performance issues and bugs. See https://github.com/rerun-io/rerun/issues/6835 for more.",
        );
    }

    let native_options = eframe_options(force_wgpu_backend);

    let window_title = "Rerun";
    eframe::run_native(
        window_title,
        native_options,
        Box::new(move |cc| {
            crate::customize_eframe_and_setup_renderer(cc)?;
            app_creator(cc)
        }),
    )
}

pub fn eframe_options(force_wgpu_backend: Option<&str>) -> eframe::NativeOptions {
    re_tracing::profile_function!();
    let os = egui::os::OperatingSystem::default();
    let custom_window_decorations = re_ui::supports_custom_decorations(os);
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id(APP_ID) // Controls where on disk the app state is persisted
            .with_decorations(!custom_window_decorations) // Maybe hide the OS-specific "chrome" around the window
            .with_fullsize_content_view(re_ui::fullsize_content(os))
            .with_icon(icon_data())
            .with_inner_size([1600.0, 1200.0])
            .with_min_inner_size([320.0, 450.0]) // Should be high enough to fit the rerun menu
            .with_title_shown(!re_ui::fullsize_content(os))
            .with_titlebar_buttons_shown(!custom_window_decorations)
            .with_titlebar_shown(!re_ui::fullsize_content(os))
            .with_transparent(custom_window_decorations), // To have rounded corners without decorations we need transparency on Linux. On Windows this mostly affects resizing which looks a bit better with this.

        renderer: eframe::Renderer::Wgpu,
        wgpu_options: crate::wgpu_options(force_wgpu_backend),
        depth_buffer: 0,
        multisampling: 0, // the 3D views do their own MSAA

        ..Default::default()
    }
}

fn icon_data() -> egui::IconData {
    re_tracing::profile_function!();

    cfg_select! {
        target_os = "macos" => {
            let app_icon_png_bytes = include_bytes!("../data/app_icon_mac.png");
        }
        target_os = "windows" => {
            let app_icon_png_bytes = include_bytes!("../data/app_icon.png");
        }
        _ => {
            // Use the same icon for X11 as for Windows, at least for now.
            let app_icon_png_bytes = include_bytes!("../data/app_icon.png");
        }
    }

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
