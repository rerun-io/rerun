use eframe::wasm_bindgen::{self, prelude::*};

use re_memory::AccountingAllocator;

#[global_allocator]
static GLOBAL: AccountingAllocator<std::alloc::System> =
    AccountingAllocator::new(std::alloc::System);

/// This is the entry-point for all the Wasm.
/// This is called once from the HTML.
/// It loads the app, installs some callbacks, then returns.
#[wasm_bindgen]
pub async fn start(canvas_id: &str) -> std::result::Result<(), eframe::wasm_bindgen::JsValue> {
    // Make sure panics are logged using `console.error`.
    console_error_panic_hook::set_once();

    re_log::setup_web_logging();

    let web_options = eframe::WebOptions {
        follow_system_theme: false,
        default_theme: eframe::Theme::Dark,
        wgpu_options: crate::wgpu_options(),
    };

    eframe::start_web(
        canvas_id,
        web_options,
        Box::new(move |cc| {
            let build_info = re_build_info::build_info!();
            let app_env = crate::AppEnvironment::Web;
            let startup_options = crate::StartupOptions::default();
            let re_ui = crate::customize_eframe(cc);
            let url = get_url(&cc.integration_info);

            if url.starts_with("http") {
                // Download an .rrd file over http
                let rx = crate::stream_rrd_from_http(url);
                Box::new(crate::App::from_receiver(
                    build_info,
                    &app_env,
                    startup_options,
                    re_ui,
                    cc.storage,
                    rx,
                ))
            } else {
                // Connect to a Rerun server over WebSockets.
                Box::new(crate::RemoteViewerApp::new(
                    build_info,
                    app_env,
                    startup_options,
                    re_ui,
                    cc.storage,
                    url,
                ))
            }
        }),
    )
    .await?;

    Ok(())
}

fn get_url(info: &eframe::IntegrationInfo) -> String {
    let mut url = String::new();
    if let Some(param) = info.web_info.location.query_map.get("url") {
        url = param.clone();
    }
    if url.is_empty() {
        re_ws_comms::default_server_url()
    } else {
        url
    }
}
