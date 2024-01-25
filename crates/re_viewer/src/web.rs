#![allow(clippy::mem_forget)] // False positives from #[wasm_bindgen] macro

use eframe::wasm_bindgen::{self, prelude::*};

use std::ops::ControlFlow;
use std::sync::Arc;

use re_log::ResultExt as _;
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
        re_log::setup_logging();

        Self {
            runner: eframe::WebRunner::new(),
        }
    }

    /// - `url` is an optional URL to either an .rrd file over http, or a Rerun WebSocket server.
    /// - `manifest_url` is an optional URL to an `examples_manifest.json` file over http.
    #[wasm_bindgen]
    pub async fn start(
        &self,
        canvas_id: &str,
        url: Option<String>,
        manifest_url: Option<String>,
    ) -> Result<(), wasm_bindgen::JsValue> {
        let web_options = eframe::WebOptions {
            follow_system_theme: false,
            default_theme: eframe::Theme::Dark,
            wgpu_options: crate::wgpu_options(),
            depth_buffer: 0,
            ..Default::default()
        };

        self.runner
            .start(
                canvas_id,
                web_options,
                Box::new(move |cc| {
                    let app = create_app(cc, &url, &manifest_url);
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

    #[wasm_bindgen]
    pub fn add_receiver(&self, url: &str) {
        let Some(mut app) = self.runner.app_mut::<crate::App>() else {
            return;
        };
        let rx = url_to_receiver(url, app.re_ui.egui_ctx.clone());
        app.add_receiver(rx);
    }

    #[wasm_bindgen]
    pub fn remove_receiver(&self, url: &str) {
        let Some(mut app) = self.runner.app_mut::<crate::App>() else {
            return;
        };
        app.msg_receive_set().remove_by_uri(url);
        if let Some(store_hub) = app.store_hub.as_mut() {
            store_hub.remove_recording_by_uri(url);
        }
    }
}

fn create_app(
    cc: &eframe::CreationContext<'_>,
    url: &Option<String>,
    manifest_url: &Option<String>,
) -> crate::App {
    let build_info = re_build_info::build_info!();
    let app_env = crate::AppEnvironment::Web;
    let startup_options = crate::StartupOptions {
        memory_limit: re_memory::MemoryLimit {
            // On wasm32 we only have 4GB of memory to play around with.
            max_bytes: Some(2_500_000_000),
        },
        location: Some(cc.integration_info.web_info.location.clone()),
        persist_state: get_persist_state(&cc.integration_info),
        is_in_notebook: is_in_notebook(&cc.integration_info),
        skip_welcome_screen: false,
    };
    let re_ui = crate::customize_eframe(cc);

    let egui_ctx = cc.egui_ctx.clone();

    let mut app = crate::App::new(build_info, &app_env, startup_options, re_ui, cc.storage);

    let query_map = &cc.integration_info.web_info.location.query_map;

    let manifest_url = match &manifest_url {
        Some(url) => Some(url.as_str()),
        None => query_map.get("manifest_url").map(String::as_str),
    };
    if let Some(url) = manifest_url {
        app.set_examples_manifest_url(url.into());
        re_log::info!("Using manifest_url={url:?}");
    }

    let url = match &url {
        Some(url) => Some(url.as_str()),
        None => query_map.get("url").map(String::as_str),
    };
    if let Some(url) = url {
        let rx = url_to_receiver(url, egui_ctx.clone());
        app.add_receiver(rx);
    }

    app
}

fn url_to_receiver(
    url: &str,
    egui_ctx: egui::Context,
) -> re_smart_channel::Receiver<re_log_types::LogMsg> {
    let ui_waker = Box::new(move || {
        // Spend a few more milliseconds decoding incoming messages,
        // then trigger a repaint (https://github.com/rerun-io/rerun/issues/963):
        egui_ctx.request_repaint_after(std::time::Duration::from_millis(10));
    });
    match categorize_uri(url) {
        EndpointCategory::HttpRrd(url) => {
            re_log_encoding::stream_rrd_from_http::stream_rrd_from_http_to_channel(
                url,
                Some(ui_waker),
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
                    ui_waker();
                    use re_log_encoding::stream_rrd_from_http::HttpMessage;
                    match msg {
                        HttpMessage::LogMsg(msg) => {
                            if tx.send(msg).is_ok() {
                                ControlFlow::Continue(())
                            } else {
                                re_log::info!("Failed to send log message to viewer - closing");
                                ControlFlow::Break(())
                            }
                        }
                        HttpMessage::Success => {
                            tx.quit(None).warn_on_err_once("failed to send quit marker");
                            ControlFlow::Break(())
                        }
                        HttpMessage::Failure(err) => {
                            tx.quit(Some(err))
                                .warn_on_err_once("failed to send quit marker");
                            ControlFlow::Break(())
                        }
                    }
                }
            }));
            rx
        }
        EndpointCategory::WebSocket(url) => re_data_source::connect_to_ws_url(&url, Some(ui_waker))
            .unwrap_or_else(|err| panic!("Failed to connect to WebSocket server at {url}: {err}")),
    }
}

/// Used to set the "email" property in the analytics config,
/// in the same way as `rerun analytics email YOURNAME@rerun.io`.
///
/// This one just panics when it fails, as it's only ever really run
/// by rerun employees manually in `app.rerun.io`.
#[cfg(feature = "analytics")]
#[wasm_bindgen]
pub fn set_email(email: String) {
    let mut config = re_analytics::Config::load().unwrap().unwrap_or_default();
    config.opt_in_metadata.insert("email".into(), email.into());
    config.save().unwrap();
}

enum EndpointCategory {
    /// Could be a local path (`/foo.rrd`) or a remote url (`http://foo.com/bar.rrd`).
    HttpRrd(String),

    /// A remote Rerun server.
    WebSocket(String),

    /// An eventListener for rrd posted from containing html
    WebEventListener,
}

fn categorize_uri(uri: &str) -> EndpointCategory {
    if uri.starts_with("http") || uri.ends_with(".rrd") {
        EndpointCategory::HttpRrd(uri.into())
    } else if uri.starts_with("ws:") || uri.starts_with("wss:") {
        EndpointCategory::WebSocket(uri.into())
    } else if uri.starts_with("web_event:") {
        EndpointCategory::WebEventListener
    } else {
        // If this is something like `foo.com` we can't know what it is until we connect to it.
        // We could/should connect and see what it is, but for now we just take a wild guess instead:
        re_log::info!("Assuming WebSocket endpoint");
        if uri.contains("://") {
            EndpointCategory::WebSocket(uri.into())
        } else {
            EndpointCategory::WebSocket(format!("{}://{uri}", re_ws_comms::PROTOCOL))
        }
    }
}

fn is_in_notebook(info: &eframe::IntegrationInfo) -> bool {
    get_query_bool(info, "notebook", false)
}

fn get_persist_state(info: &eframe::IntegrationInfo) -> bool {
    get_query_bool(info, "persist", true)
}

fn get_query_bool(info: &eframe::IntegrationInfo, key: &str, default: bool) -> bool {
    let default_int = default as i32;
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
