#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::{self, prelude::*};

/// This is the entry-point for all the web-assembly.
/// This is called once from the HTML.
/// It loads the app, installs some callbacks, then returns.
/// You can add more callbacks like this if you want to call in to your code.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start(canvas_id: &str) -> std::result::Result<(), eframe::wasm_bindgen::JsValue> {
    // Make sure panics are logged using `console.error`.
    console_error_panic_hook::set_once();

    // Redirect tracing to `console.log`:
    redirect_tracing_to_console_log();

    let web_options = eframe::WebOptions {
        follow_system_theme: false,
        default_theme: eframe::Theme::Dark,
        wgpu_options: crate::wgpu_options(),
    };

    eframe::start_web(
        canvas_id,
        web_options,
        Box::new(move |cc| {
            let design_tokens = crate::customize_eframe(cc);
            let url = get_url(&cc.integration_info);
            let app = crate::RemoteViewerApp::new(&cc.egui_ctx, design_tokens, cc.storage, url);
            Box::new(app)
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

fn redirect_tracing_to_console_log() {
    use tracing_subscriber::layer::SubscriberExt as _;
    tracing::subscriber::set_global_default(
        tracing_subscriber::Registry::default()
            // Filtering out wgpu spam seems to mitigate https://linear.app/rerun/issue/PRO-256/wgpu-crashes-on-web
            .with(tracing_subscriber::EnvFilter::new(
                "info,wgpu_core=warn,wgpu_hal=warn",
            ))
            .with(tracing_wasm::WASMLayer::new(
                tracing_wasm::WASMLayerConfig::default(),
            )),
    )
    .expect("Failed to set tracing subscriber.");
}
