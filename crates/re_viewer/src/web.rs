//! Main entry-point of the web app.

#![allow(clippy::mem_forget)] // False positives from #[wasm_bindgen] macro

use ahash::HashMap;
use serde::Deserialize;
use std::str::FromStr as _;
use wasm_bindgen::prelude::*;

use re_log::ResultExt as _;
use re_memory::AccountingAllocator;
use re_viewer_context::CommandSender;

use crate::web_tools::{string_from_js_value, translate_query_into_commands, url_to_receiver};

#[global_allocator]
static GLOBAL: AccountingAllocator<std::alloc::System> =
    AccountingAllocator::new(std::alloc::System);

#[wasm_bindgen]
pub struct WebHandle {
    runner: eframe::WebRunner,

    /// A dedicated smart channel used by the [`WebHandle::add_rrd_from_bytes`] API.
    ///
    /// This exists because the direct bytes API is expected to submit many small RRD chunks
    /// and allocating a new tx pair for each chunk doesn't make sense.
    tx_channels: HashMap<String, re_smart_channel::Sender<re_log_types::LogMsg>>,

    app_options: AppOptions,
}

#[wasm_bindgen]
impl WebHandle {
    #[allow(clippy::new_without_default, clippy::use_self)] // Can't use `Self` here because of `#[wasm_bindgen]`.
    #[wasm_bindgen(constructor)]
    pub fn new(app_options: JsValue) -> Result<WebHandle, JsValue> {
        re_log::setup_logging();

        let app_options: Option<AppOptions> = serde_wasm_bindgen::from_value(app_options)?;

        Ok(Self {
            runner: eframe::WebRunner::new(),
            tx_channels: Default::default(),
            app_options: app_options.unwrap_or_default(),
        })
    }

    /// - `url` is an optional URL to either an .rrd file over http, or a Rerun WebSocket server.
    /// - `manifest_url` is an optional URL to an `examples_manifest.json` file over http.
    /// - `force_wgpu_backend` is an optional string to force a specific backend, either `webgl` or `webgpu`.
    #[wasm_bindgen]
    pub async fn start(&self, canvas_id: String) -> Result<(), wasm_bindgen::JsValue> {
        let app_options = self.app_options.clone();
        let web_options = eframe::WebOptions {
            follow_system_theme: false,
            default_theme: eframe::Theme::Dark,
            wgpu_options: crate::wgpu_options(app_options.render_backend.clone()),
            depth_buffer: 0,
        };

        self.runner
            .start(
                &canvas_id,
                web_options,
                Box::new(move |cc| Ok(Box::new(create_app(cc, app_options)?))),
            )
            .await?;

        re_log::debug!("Web app started.");

        Ok(())
    }

    #[wasm_bindgen]
    pub fn toggle_panel_overrides(&self) {
        let Some(mut app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        app.panel_state_overrides_active ^= true;
    }

    #[wasm_bindgen]
    pub fn override_panel_state(&self, panel: &str, state: Option<String>) -> Result<(), JsValue> {
        let Some(mut app) = self.runner.app_mut::<crate::App>() else {
            return Ok(());
        };

        let panel = Panel::from_str(panel)
            .map_err(|err| js_sys::TypeError::new(&format!("invalid panel: {err}")))?;

        let state = match state {
            Some(state) => Some(
                PanelState::from_str(&state)
                    .map_err(|err| js_sys::TypeError::new(&format!("invalid state: {err}")))?
                    .into(),
            ),
            None => None,
        };

        let overrides = &mut app.panel_state_overrides;
        match panel {
            Panel::Top => overrides.top = state,
            Panel::Blueprint => overrides.blueprint = state,
            Panel::Selection => overrides.selection = state,
            Panel::Time => overrides.time = state,
        }

        // request repaint, because the overrides may cause panels to expand/collapse
        app.re_ui.egui_ctx.request_repaint();

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

    /// Add a new receiver streaming data from the given url.
    ///
    /// If `follow_if_http` is `true`, and the url is an HTTP source, the viewer will open the stream
    /// in `Following` mode rather than `Playing` mode.
    ///
    /// Websocket streams are always opened in `Following` mode.
    ///
    /// It is an error to open a channel twice with the same id.
    #[wasm_bindgen]
    pub fn add_receiver(&self, url: &str, follow_if_http: Option<bool>) {
        let Some(mut app) = self.runner.app_mut::<crate::App>() else {
            return;
        };
        let follow_if_http = follow_if_http.unwrap_or(false);
        let rx = url_to_receiver(app.re_ui.egui_ctx.clone(), follow_if_http, url);
        if let Some(rx) = rx.ok_or_log_error() {
            app.add_receiver(rx);
        }
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

    /// Open a new channel for streaming data.
    ///
    /// It is an error to open a channel twice with the same id.
    #[wasm_bindgen]
    pub fn open_channel(&mut self, id: &str, channel_name: &str) {
        let Some(mut app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        if self.tx_channels.contains_key(id) {
            re_log::warn!("Channel with id '{}' already exists.", id);
            return;
        }

        let (tx, rx) = re_smart_channel::smart_channel(
            re_smart_channel::SmartMessageSource::JsChannelPush,
            re_smart_channel::SmartChannelSource::JsChannel {
                channel_name: channel_name.to_owned(),
            },
        );

        app.add_receiver(rx);
        self.tx_channels.insert(id.to_owned(), tx);
    }

    /// Close an existing channel for streaming data.
    ///
    /// No-op if the channel is already closed.
    #[wasm_bindgen]
    pub fn close_channel(&mut self, id: &str) {
        let Some(app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        if let Some(tx) = self.tx_channels.remove(id) {
            tx.quit(None).warn_on_err_once("Failed to send quit marker");
        }

        // Request a repaint since closing the channel may update the top bar.
        app.re_ui
            .egui_ctx
            .request_repaint_after(std::time::Duration::from_millis(10));
    }

    /// Add an rrd to the viewer directly from a byte array.
    #[wasm_bindgen]
    pub fn send_rrd_to_channel(&mut self, id: &str, data: &[u8]) {
        use std::{ops::ControlFlow, sync::Arc};
        let Some(app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        if let Some(tx) = self.tx_channels.get(id).cloned() {
            let data: Vec<u8> = data.to_vec();

            let egui_ctx = app.re_ui.egui_ctx.clone();

            let ui_waker = Box::new(move || {
                // Spend a few more milliseconds decoding incoming messages,
                // then trigger a repaint (https://github.com/rerun-io/rerun/issues/963):
                egui_ctx.request_repaint_after(std::time::Duration::from_millis(10));
            });

            re_log_encoding::stream_rrd_from_http::web_decode::decode_rrd(
                data,
                Arc::new({
                    move |msg| {
                        ui_waker();
                        use re_log_encoding::stream_rrd_from_http::HttpMessage;
                        match msg {
                            HttpMessage::LogMsg(msg) => {
                                if tx.send(msg).is_ok() {
                                    ControlFlow::Continue(())
                                } else {
                                    re_log::info_once!("Failed to dispatch log message to viewer.");
                                    ControlFlow::Break(())
                                }
                            }
                            // TODO(jleibs): Unclear what we want to do here. More data is coming.
                            HttpMessage::Success => ControlFlow::Continue(()),
                            HttpMessage::Failure(err) => {
                                tx.quit(Some(err))
                                    .warn_on_err_once("Failed to send quit marker");
                                ControlFlow::Break(())
                            }
                        }
                    }
                }),
            );
        }
    }
}

// TODO(jprochazk): figure out a way to auto-generate these types on JS side

// Keep in sync with the `Panel` typedef in `rerun_js/web-viewer/index.js`
#[derive(Clone, Deserialize, strum_macros::EnumString)]
#[strum(serialize_all = "snake_case")]
enum Panel {
    Top,
    Blueprint,
    Selection,
    Time,
}

// Keep in sync with the `PanelState` typedef in `rerun_js/web-viewer/index.js`
#[derive(Clone, Deserialize, strum_macros::EnumString)]
#[strum(serialize_all = "snake_case")]
enum PanelState {
    Hidden,
    Collapsed,
    Expanded,
}

impl From<PanelState> for re_types::blueprint::components::PanelState {
    fn from(value: PanelState) -> Self {
        match value {
            PanelState::Hidden => Self::Hidden,
            PanelState::Collapsed => Self::Collapsed,
            PanelState::Expanded => Self::Expanded,
        }
    }
}

// Keep in sync with the `AppOptions` typedef in `rerun_js/web-viewer/index.js`
#[derive(Clone, Default, Deserialize)]
pub struct AppOptions {
    url: Option<String>,
    manifest_url: Option<String>,
    render_backend: Option<String>,
    hide_welcome_screen: Option<bool>,
    panel_state_overrides: Option<PanelStateOverrides>,
}

#[derive(Clone, Default, Deserialize)]
pub struct PanelStateOverrides {
    top: Option<PanelState>,
    blueprint: Option<PanelState>,
    selection: Option<PanelState>,
    time: Option<PanelState>,
}

impl From<PanelStateOverrides> for crate::app_blueprint::PanelStateOverrides {
    fn from(value: PanelStateOverrides) -> Self {
        Self {
            top: value.top.map(|v| v.into()),
            blueprint: value.blueprint.map(|v| v.into()),
            selection: value.selection.map(|v| v.into()),
            time: value.time.map(|v| v.into()),
        }
    }
}

// Can't deserialize `Option<js_sys::Function>` directly, so newtype it is.
#[derive(Clone, Deserialize)]
#[repr(transparent)]
struct Callback(#[serde(with = "serde_wasm_bindgen::preserve")] js_sys::Function);

fn create_app(
    cc: &eframe::CreationContext<'_>,
    app_options: AppOptions,
) -> Result<crate::App, re_renderer::RenderContextError> {
    let build_info = re_build_info::build_info!();
    let app_env = crate::AppEnvironment::Web {
        url: cc.integration_info.web_info.location.url.clone(),
    };
    let startup_options = crate::StartupOptions {
        memory_limit: re_memory::MemoryLimit {
            // On wasm32 we only have 4GB of memory to play around with.
            max_bytes: Some(2_500_000_000),
        },
        location: Some(cc.integration_info.web_info.location.clone()),
        persist_state: get_persist_state(&cc.integration_info),
        is_in_notebook: is_in_notebook(&cc.integration_info),
        expect_data_soon: None,
        force_wgpu_backend: None,
        hide_welcome_screen: app_options.hide_welcome_screen.unwrap_or(false),
        panel_state_overrides: app_options.panel_state_overrides.unwrap_or_default().into(),
    };
    let re_ui = crate::customize_eframe_and_setup_renderer(cc)?;

    let mut app = crate::App::new(build_info, &app_env, startup_options, re_ui, cc.storage);

    let query_map = &cc.integration_info.web_info.location.query_map;

    if let Some(manifest_url) = &app_options.manifest_url {
        app.set_examples_manifest_url(manifest_url.into());
    } else {
        for url in query_map.get("manifest_url").into_iter().flatten() {
            app.set_examples_manifest_url(url.clone());
        }
    }

    if let Some(url) = &app_options.url {
        let follow_if_http = false;
        if let Some(receiver) =
            url_to_receiver(cc.egui_ctx.clone(), follow_if_http, url).ok_or_log_error()
        {
            app.add_receiver(receiver);
        }
    } else {
        translate_query_into_commands(&cc.egui_ctx, &app.command_sender);
    }

    install_popstate_listener(cc.egui_ctx.clone(), app.command_sender.clone());

    Ok(app)
}

/// Listen for `popstate` event, which comes when the user hits the back/forward buttons.
///
/// <https://developer.mozilla.org/en-US/docs/Web/API/Window/popstate_event>
fn install_popstate_listener(egui_ctx: egui::Context, command_sender: CommandSender) -> Option<()> {
    let window = web_sys::window()?;
    let closure = Closure::wrap(Box::new(move |_: web_sys::Event| {
        translate_query_into_commands(&egui_ctx, &command_sender);
    }) as Box<dyn FnMut(_)>);
    window
        .add_event_listener_with_callback("popstate", closure.as_ref().unchecked_ref())
        .map_err(|err| {
            format!(
                "Failed to add popstate event listener: {}",
                string_from_js_value(err)
            )
        })
        .ok_or_log_error()?;
    closure.forget();
    Some(())
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

fn is_in_notebook(info: &eframe::IntegrationInfo) -> bool {
    get_query_bool(info, "notebook", false)
}

fn get_persist_state(info: &eframe::IntegrationInfo) -> bool {
    get_query_bool(info, "persist", true)
}

fn get_query_bool(info: &eframe::IntegrationInfo, key: &str, default: bool) -> bool {
    let default_int = default as i32;

    if let Some(values) = info.web_info.location.query_map.get(key) {
        if values.len() == 1 {
            match values[0].as_str() {
                "0" => false,
                "1" => true,
                other => {
                    re_log::warn!(
                            "Unexpected value for '{key}' query: {other:?}. Expected either '0' or '1'. Defaulting to '{default_int}'."
                        );
                    default
                }
            }
        } else {
            re_log::warn!(
                "Found {} values for '{key}' query. Expected one or none. Defaulting to '{default_int}'.",
                values.len()
            );
            default
        }
    } else {
        default
    }
}
