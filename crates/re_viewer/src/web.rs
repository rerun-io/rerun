use eframe::wasm_bindgen::{self, prelude::*};

use std::sync::Arc;

use re_error::ResultExt as _;
use re_memory::AccountingAllocator;

#[global_allocator]
static GLOBAL: AccountingAllocator<std::alloc::System> =
    AccountingAllocator::new(std::alloc::System);

#[wasm_bindgen]
pub struct WebHandle {
    runner: eframe::WebRunner,
}

#[wasm_bindgen]
impl WebHandle {
    #[allow(clippy::new_without_default)]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        re_log::setup_web_logging();

        Self {
            runner: eframe::WebRunner::new(),
        }
    }

    /// The `url` is an optional URL to either an .rrd file over http, or a Rerun WebSocket server.
    #[wasm_bindgen]
    pub async fn start(
        &self,
        canvas_id: &str,
        url: Option<String>,
    ) -> Result<(), wasm_bindgen::JsValue> {
        let web_options = eframe::WebOptions {
            follow_system_theme: false,
            default_theme: eframe::Theme::Dark,
            wgpu_options: crate::wgpu_options(),
            depth_buffer: 0,
        };

        self.runner
            .start(
                canvas_id,
                web_options,
                Box::new(move |cc| {
                    let build_info = re_build_info::build_info!();
                    let app_env = crate::AppEnvironment::Web;
                    let persist_state = get_persist_state(&cc.integration_info);
                    let startup_options = crate::StartupOptions {
                        memory_limit: re_memory::MemoryLimit {
                            // On wasm32 we only have 4GB of memory to play around with.
                            limit: Some(2_500_000_000),
                        },
                        persist_state,
                    };
                    let re_ui = crate::customize_eframe(cc);
                    let url = url.unwrap_or_else(|| get_url(&cc.integration_info));

                    match categorize_uri(url) {
                        EndpointCategory::HttpRrd(url) => {
                            // Download an .rrd file over http:
                            let (tx, rx) = re_smart_channel::smart_channel(
                                re_smart_channel::SmartMessageSource::RrdHttpStream {
                                    url: url.clone(),
                                },
                                re_smart_channel::SmartChannelSource::RrdHttpStream {
                                    url: url.clone(),
                                },
                            );
                            re_log_encoding::stream_rrd_from_http::stream_rrd_from_http(
                                url,
                                Arc::new({
                                    let egui_ctx = cc.egui_ctx.clone();
                                    move |msg| {
                                        egui_ctx.request_repaint(); // wake up ui thread
                                        use re_log_encoding::stream_rrd_from_http::HttpMessage;
                                        match msg {
                                            HttpMessage::LogMsg(msg) => tx
                                                .send(msg)
                                                .warn_on_err_once("failed to send message"),
                                            HttpMessage::Success => tx
                                                .quit(None)
                                                .warn_on_err_once("failed to send quit marker"),
                                            HttpMessage::Failure(err) => tx
                                                .quit(Some(err))
                                                .warn_on_err_once("failed to send quit marker"),
                                        };
                                    }
                                }),
                            );

                            Box::new(crate::App::from_receiver(
                                build_info,
                                &app_env,
                                startup_options,
                                re_ui,
                                cc.storage,
                                rx,
                            ))
                        }
                        EndpointCategory::WebEventListener => {
                            // Process an rrd when it's posted via `window.postMessage`
                            let (tx, rx) = re_smart_channel::smart_channel(
                                re_smart_channel::SmartMessageSource::RrdWebEventCallback,
                                re_smart_channel::SmartChannelSource::RrdWebEventListener,
                            );
                            re_log_encoding::stream_rrd_from_http::stream_rrd_from_event_listener(
                                Arc::new({
                                    let egui_ctx = cc.egui_ctx.clone();
                                    move |msg| {
                                        egui_ctx.request_repaint(); // wake up ui thread
                                        use re_log_encoding::stream_rrd_from_http::HttpMessage;
                                        match msg {
                                            HttpMessage::LogMsg(msg) => tx
                                                .send(msg)
                                                .warn_on_err_once("failed to send message"),
                                            HttpMessage::Success => tx
                                                .quit(None)
                                                .warn_on_err_once("failed to send quit marker"),
                                            HttpMessage::Failure(err) => tx
                                                .quit(Some(err))
                                                .warn_on_err_once("failed to send quit marker"),
                                        };
                                    }
                                }),
                            );

                            Box::new(crate::App::from_receiver(
                                build_info,
                                &app_env,
                                startup_options,
                                re_ui,
                                cc.storage,
                                rx,
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

        re_log::debug!("Web app started.");

        Ok(())
    }

    #[wasm_bindgen]
    pub fn destroy(&self) {
        self.runner.destroy();
    }

    #[wasm_bindgen]
    pub fn has_panicked(&self) -> bool {
        self.runner.panic_summary().is_some()
    }

    #[wasm_bindgen]
    pub fn panic_message(&self) -> Option<String> {
        self.runner.panic_summary().map(|s| s.message())
    }

    #[wasm_bindgen]
    pub fn panic_callstack(&self) -> Option<String> {
        self.runner.panic_summary().map(|s| s.callstack())
    }
}

#[wasm_bindgen]
pub fn is_webgpu_build() -> bool {
    !cfg!(feature = "webgl")
}

enum EndpointCategory {
    /// Could be a local path (`/foo.rrd`) or a remote url (`http://foo.com/bar.rrd`).
    HttpRrd(String),

    /// A remote Rerun server.
    WebSocket(String),

    /// An eventListener for rrd posted from containing html
    WebEventListener,
}

fn categorize_uri(mut uri: String) -> EndpointCategory {
    if uri.starts_with("http") || uri.ends_with(".rrd") {
        EndpointCategory::HttpRrd(uri)
    } else if uri.starts_with("ws:") || uri.starts_with("wss:") {
        EndpointCategory::WebSocket(uri)
    } else if uri.starts_with("web_event:") {
        EndpointCategory::WebEventListener
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
        format!(
            "{}://{}:{}",
            re_ws_comms::PROTOCOL,
            &info.web_info.location.hostname,
            re_ws_comms::DEFAULT_WS_SERVER_PORT
        )
    } else {
        url
    }
}

fn get_persist_state(info: &eframe::IntegrationInfo) -> bool {
    match info
        .web_info
        .location
        .query_map
        .get("persist")
        .map(String::as_str)
    {
        Some("0") => false,
        Some("1") => true,
        Some(other) => {
            re_log::warn!(
                "Unexpected value for 'persist' query: {other:?}. Expected either '0' or '1'. Defaulting to '1'."
            );
            true
        }
        _ => true,
    }
}
