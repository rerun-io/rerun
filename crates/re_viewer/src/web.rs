use eframe::wasm_bindgen::{self, prelude::*};

use re_memory::AccountingAllocator;

#[global_allocator]
static GLOBAL: AccountingAllocator<std::alloc::System> =
    AccountingAllocator::new(std::alloc::System);

/// This is the entry-point for all the Wasm.
///
/// This is called once from the HTML.
/// It loads the app, installs some callbacks, then returns.
/// The `url` is an optional URL to either an .rrd file over http, or a Rerun WebSocket server.
#[wasm_bindgen]
pub async fn start(
    canvas_id: &str,
    url: Option<String>,
) -> std::result::Result<(), eframe::wasm_bindgen::JsValue> {
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
            let startup_options = crate::StartupOptions {
                memory_limit: re_memory::MemoryLimit {
                    // On wasm32 we only have 4GB of memory to play around with.
                    limit: Some(3_500_000_000),
                },
            };
            let re_ui = crate::customize_eframe(cc);
            let url = url.unwrap_or_else(|| get_url(&cc.integration_info));

            match categorize_uri(url) {
                EndpointCategory::HttpRrd(url) => {
                    // Download an .rrd file over http:
                    let (tx, rx) =
                        re_smart_channel::smart_channel(re_smart_channel::Source::RrdHttpStream {
                            url: url.clone(),
                        });
                    let egui_ctx = cc.egui_ctx.clone();
                    re_transport::stream_rrd_from_http::stream_rrd_from_http(
                        url,
                        Box::new(move |msg| {
                            egui_ctx.request_repaint(); // wake up ui thread
                            tx.send(msg).ok();
                        }),
                    );

                    Box::new(crate::App::from_receiver(
                        build_info,
                        &app_env,
                        startup_options,
                        re_ui,
                        cc.storage,
                        rx,
                        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                    ))
                }
                EndpointCategory::WebSocket(url) => {
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
            }
        }),
    )
    .await?;

    Ok(())
}

enum EndpointCategory {
    /// Could be a local path (`/foo.rrd`) or a remote url (`http://foo.com/bar.rrd`).
    HttpRrd(String),

    /// A remote Rerun server.
    WebSocket(String),
}

fn categorize_uri(mut uri: String) -> EndpointCategory {
    if uri.starts_with("http") || uri.ends_with(".rrd") {
        EndpointCategory::HttpRrd(uri)
    } else if uri.starts_with("ws") {
        EndpointCategory::WebSocket(uri)
    } else {
        // If this is sometyhing like `foo.com` we can't know what it is until we connect to it.
        // We could/should connect and see what it is, but for now we just take a wild guess instead:
        re_log::info!("Assuming WebSocket endpoint");
        if !uri.contains("://") {
            uri = format!("{}://{uri}", re_ws_comms::PROTOCOL);
        }
        EndpointCategory::WebSocket(uri)
    }
}

fn get_url(info: &eframe::IntegrationInfo) -> String {
    let mut url = String::new();
    if let Some(param) = info.web_info.location.query_map.get("url") {
        url = param.clone();
    }
    if url.is_empty() {
        re_ws_comms::default_server_url(&info.web_info.location.hostname)
    } else {
        url
    }
}
