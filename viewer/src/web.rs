#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::{self, prelude::*};

/// This is the entry-point for all the web-assembly.
/// This is called once from the HTML.
/// It loads the app, installs some callbacks, then returns.
/// You can add more callbacks like this if you want to call in to your code.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start(canvas_id: &str) -> std::result::Result<(), eframe::wasm_bindgen::JsValue> {
    // Make sure panics are logged using `console.error`.
    console_error_panic_hook::set_once();

    // Redirect tracing to console.log and friends:
    tracing_wasm::set_as_global_default();

    eframe::start_web(
        canvas_id,
        Box::new(move |cc| {
            let url = get_url(&cc.integration_info);
            let app = crate::RemoteViewerApp::new(cc.egui_ctx.clone(), cc.storage.as_deref(), url);
            Box::new(app)
        }),
    )
}

fn get_url(info: &eframe::IntegrationInfo) -> String {
    let mut url = String::new();
    if let Some(web_info) = &info.web_info {
        if let Some(param) = web_info.location.query_map.get("url") {
            url = param.clone();
        }
    }
    if url.is_empty() {
        comms::default_server_url()
    } else {
        url
    }
}
