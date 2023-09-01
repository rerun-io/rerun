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
                    let app = create_app(cc, url.clone());
                    Box::new(app)
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

fn create_app(cc: &eframe::CreationContext<'_>, url: Option<String>) -> crate::App {
    let build_info = re_build_info::build_info!();
    let app_env = crate::AppEnvironment::Web;
    let startup_options = crate::StartupOptions {
        memory_limit: re_memory::MemoryLimit {
            // On wasm32 we only have 4GB of memory to play around with.
            limit: Some(2_500_000_000),
        },
        persist_state: get_persist_state(&cc.integration_info),
        is_in_notebook: is_in_notebook(&cc.integration_info),
        skip_welcome_screen: false,
    };
    let re_ui = crate::customize_eframe(cc);
    let url = url.unwrap_or_else(|| get_url(&cc.integration_info));

    let egui_ctx = cc.egui_ctx.clone();
    let wake_up_ui_on_msg = Box::new(move || {
        // Spend a few more milliseconds decoding incoming messages,
        // then trigger a repaint (https://github.com/rerun-io/rerun/issues/963):
        egui_ctx.request_repaint_after(std::time::Duration::from_millis(10));
    });

    let rx = match categorize_uri(url) {
        EndpointCategory::HttpRrd(url) => {
            re_log_encoding::stream_rrd_from_http::stream_rrd_from_http_to_channel(
                url,
                Some(wake_up_ui_on_msg),
            )
        }
        EndpointCategory::WebEventListener => {
            // Process an rrd when it's posted via `window.postMessage`
            let (tx, rx) = re_smart_channel::smart_channel(
                re_smart_channel::SmartMessageSource::RrdWebEventCallback,
                re_smart_channel::SmartChannelSource::RrdWebEventListener,
            );
            re_log_encoding::stream_rrd_from_http::stream_rrd_from_event_listener(Arc::new({
                move |msg| {
                    wake_up_ui_on_msg();
                    use re_log_encoding::stream_rrd_from_http::HttpMessage;
                    match msg {
                        HttpMessage::LogMsg(msg) => {
                            tx.send(msg).warn_on_err_once("failed to send message")
                        }
                        HttpMessage::Success => {
                            tx.quit(None).warn_on_err_once("failed to send quit marker")
                        }
                        HttpMessage::Failure(err) => tx
                            .quit(Some(err))
                            .warn_on_err_once("failed to send quit marker"),
                    };
                }
            }));
            rx
        }
        EndpointCategory::WebSocket(url) => {
            re_data_source::connect_to_ws_url(&url, Some(wake_up_ui_on_msg)).unwrap_or_else(|err| {
                panic!("Failed to connect to WebSocket server at {url}: {err}")
            })
        }
    };

    let mut app = crate::App::new(build_info, &app_env, startup_options, re_ui, cc.storage);
    app.add_receiver(rx);
    app
}

#[cfg(feature = "analytics")]
#[wasm_bindgen]
pub fn set_email(email: String) {
    let mut config = re_analytics::Config::load().unwrap().unwrap_or_default();
    config.opt_in_metadata.insert("email".into(), email.into());
    config.save().unwrap()
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

fn is_in_notebook(info: &eframe::IntegrationInfo) -> bool {
    get_query_bool(info, "notebook", false)
}

fn get_persist_state(info: &eframe::IntegrationInfo) -> bool {
    get_query_bool(info, "persist", true)
}

fn get_query_bool(info: &eframe::IntegrationInfo, key: &str, default: bool) -> bool {
    let default_int = i32::from(default);
    match info
        .web_info
        .location
        .query_map
        .get(key)
        .map(String::as_str)
    {
        Some("0") => false,
        Some("1") => true,
        Some(other) => {
            re_log::warn!(
                "Unexpected value for '{key}' query: {other:?}. Expected either '0' or '1'. Defaulting to '{default_int}'."
            );
            default
        }
        _ => default,
    }
}
