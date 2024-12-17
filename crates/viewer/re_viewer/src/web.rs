//! Main entry-point of the web app.

#![allow(clippy::mem_forget)] // False positives from #[wasm_bindgen] macro

use ahash::HashMap;
use serde::Deserialize;
use std::str::FromStr as _;
use wasm_bindgen::prelude::*;

use re_log::ResultExt as _;
use re_memory::AccountingAllocator;
use re_viewer_context::{SystemCommand, SystemCommandSender};

use crate::history::install_popstate_listener;
use crate::web_tools::{url_to_receiver, Callback, JsResultExt as _, StringOrStringArray};

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

    #[wasm_bindgen]
    pub async fn start(&self, canvas: JsValue) -> Result<(), wasm_bindgen::JsValue> {
        let main_thread_token = crate::MainThreadToken::i_promise_i_am_on_the_main_thread();

        let canvas = if let Some(canvas_id) = canvas.as_string() {
            // For backwards compatibility with old JS/HTML written before 2024-08-30
            let document = web_sys::window().unwrap().document().unwrap();
            let element = document
                .get_element_by_id(&canvas_id)
                .ok_or_else(|| format!("Canvas element '{canvas_id}' not found."))?;
            element
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .map_err(|element| {
                    format!("Expected a canvas element or canvas id, got {element:?}")
                })?
        } else {
            canvas
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .map_err(|element| {
                    format!("Expected a canvas element or canvas id, got {element:?}")
                })?
        };

        let app_options = self.app_options.clone();
        let web_options = eframe::WebOptions {
            wgpu_options: crate::wgpu_options(app_options.render_backend.clone()),
            depth_buffer: 0,
            dithering: true,
            ..Default::default()
        };

        self.runner
            .start(
                canvas,
                web_options,
                Box::new(move |cc| Ok(Box::new(create_app(main_thread_token, cc, app_options)?))),
            )
            .await?;

        re_log::debug!("Web app started.");

        Ok(())
    }

    #[wasm_bindgen]
    pub fn toggle_panel_overrides(&self, value: Option<bool>) {
        let Some(mut app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        match value {
            Some(value) => app.panel_state_overrides_active = value,
            None => app.panel_state_overrides_active ^= true,
        }

        // request repaint, because the overrides may cause panels to expand/collapse
        app.egui_ctx.request_repaint();
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
        app.egui_ctx.request_repaint();

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
        let rx = url_to_receiver(app.egui_ctx.clone(), follow_if_http, url.to_owned());
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
        app.egui_ctx
            .request_repaint_after(std::time::Duration::from_millis(10));
    }

    /// Add an rrd to the viewer directly from a byte array.
    #[wasm_bindgen]
    pub fn send_rrd_to_channel(&self, id: &str, data: &[u8]) {
        use std::{ops::ControlFlow, sync::Arc};
        let Some(app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        if let Some(tx) = self.tx_channels.get(id).cloned() {
            let data: Vec<u8> = data.to_vec();

            let egui_ctx = app.egui_ctx.clone();

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

// Keep in sync with the `AppOptions` interface in `rerun_js/web-viewer/index.ts`.
#[derive(Clone, Default, Deserialize)]
pub struct AppOptions {
    url: Option<StringOrStringArray>,
    manifest_url: Option<String>,
    render_backend: Option<String>,
    video_decoder: Option<String>,
    hide_welcome_screen: Option<bool>,
    panel_state_overrides: Option<PanelStateOverrides>,
    fullscreen: Option<FullscreenOptions>,
    enable_history: Option<bool>,

    notebook: Option<bool>,
    persist: Option<bool>,
}

// Keep in sync with the `FullscreenOptions` interface in `rerun_js/web-viewer/index.ts`
#[derive(Clone, Deserialize)]
pub struct FullscreenOptions {
    /// This returns the current fullscreen state, which is a boolean representing on/off.
    pub get_state: Callback,

    /// This calls the JS version of "toggle fullscreen".
    pub on_toggle: Callback,
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

fn create_app(
    main_thread_token: crate::MainThreadToken,
    cc: &eframe::CreationContext<'_>,
    app_options: AppOptions,
) -> Result<crate::App, re_renderer::RenderContextError> {
    let build_info = re_build_info::build_info!();
    let app_env = crate::AppEnvironment::Web {
        url: cc.integration_info.web_info.location.url.clone(),
    };

    let AppOptions {
        url,
        manifest_url,
        render_backend,
        video_decoder,
        hide_welcome_screen,
        panel_state_overrides,
        fullscreen,
        enable_history,

        notebook,
        persist,
    } = app_options;

    let enable_history = enable_history.unwrap_or(false);

    let video_decoder_hw_acceleration = video_decoder.and_then(|s| match s.parse() {
        Err(()) => {
            re_log::warn_once!("Failed to parse --video-decoder value: {s}. Ignoring.");
            None
        }
        Ok(hw_accell) => Some(hw_accell),
    });

    let startup_options = crate::StartupOptions {
        memory_limit: re_memory::MemoryLimit {
            // On wasm32 we only have 4GB of memory to play around with.
            max_bytes: Some(2_500_000_000),
        },
        location: Some(cc.integration_info.web_info.location.clone()),
        persist_state: persist.unwrap_or(true),
        is_in_notebook: notebook.unwrap_or(false),
        expect_data_soon: None,
        force_wgpu_backend: render_backend.clone(),
        video_decoder_hw_acceleration,
        hide_welcome_screen: hide_welcome_screen.unwrap_or(false),
        fullscreen_options: fullscreen.clone(),
        panel_state_overrides: panel_state_overrides.unwrap_or_default().into(),

        enable_history,
    };
    crate::customize_eframe_and_setup_renderer(cc)?;

    let mut app = crate::App::new(
        main_thread_token,
        build_info,
        &app_env,
        startup_options,
        cc.egui_ctx.clone(),
        cc.storage,
    );

    if enable_history {
        install_popstate_listener(&mut app).ok_or_log_js_error();
    }

    if let Some(manifest_url) = manifest_url {
        app.set_examples_manifest_url(manifest_url);
    }

    if let Some(urls) = url {
        let follow_if_http = false;
        for url in urls.into_inner() {
            if let Some(receiver) =
                url_to_receiver(cc.egui_ctx.clone(), follow_if_http, url).ok_or_log_error()
            {
                app.command_sender
                    .send_system(SystemCommand::AddReceiver(receiver));
            }
        }
    }

    Ok(app)
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
