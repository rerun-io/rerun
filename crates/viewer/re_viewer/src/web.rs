//! Main entry-point of the web app.

#![allow(clippy::allow_attributes, clippy::mem_forget)] // False positives from #[wasm_bindgen] macro

use std::rc::Rc;
use std::str::FromStr as _;

use ahash::HashMap;
use arrow::array::RecordBatch;
use re_log::ResultExt as _;
use re_log_channel::LogSender;
use re_log_types::{TableId, TableMsg};
use re_memory::AccountingAllocator;
use re_sdk_types::blueprint::components::PlayState;
use re_viewer_context::{
    AsyncRuntimeHandle, SystemCommand, SystemCommandSender as _, TimeControlCommand, open_url,
};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::web_history::install_popstate_listener;
use crate::web_tools::{Callback, JsResultExt as _, StringOrStringArray};

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
    log_senders: HashMap<String, LogSender>,

    /// The connection registry to use for the viewer.
    connection_registry: re_redap_client::ConnectionRegistryHandle,

    app_options: AppOptions,
}

#[wasm_bindgen]
impl WebHandle {
    #[allow(
        clippy::allow_attributes,
        clippy::new_without_default,
        clippy::use_self
    )] // Can't use `Self` here because of `#[wasm_bindgen]`.
    #[wasm_bindgen(constructor)]
    pub fn new(app_options: JsValue) -> Result<WebHandle, JsValue> {
        re_log::setup_logging();

        let app_options: Option<AppOptions> = serde_wasm_bindgen::from_value(app_options)?;

        let connection_registry =
            re_redap_client::ConnectionRegistry::new_with_stored_credentials();

        Ok(Self {
            runner: eframe::WebRunner::new(),
            log_senders: Default::default(),
            connection_registry,
            app_options: app_options.unwrap_or_default(),
        })
    }

    #[wasm_bindgen]
    pub async fn start(&self, canvas: JsValue) -> Result<(), wasm_bindgen::JsValue> {
        let main_thread_token = crate::MainThreadToken::i_promise_i_am_on_the_main_thread();

        let canvas = if let Some(canvas_id) = canvas.as_string() {
            // For backwards compatibility with old JS/HTML written before 2024-08-30
            let document = web_sys::window()
                .ok_or_else(|| "Failed to get window. Are we not in a browser?".to_owned())?
                .document()
                .ok_or_else(|| {
                    "Failed to get window.document. Are we not in a browser?".to_owned()
                })?;
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
            wgpu_options: crate::wgpu_options(app_options.render_backend.as_deref()),
            depth_buffer: 0,
            dithering: true,
            ..Default::default()
        };

        let connection_registry = self.connection_registry.clone();
        self.runner
            .start(
                canvas,
                web_options,
                Box::new(move |cc| {
                    Ok(Box::new(create_app(
                        main_thread_token,
                        cc,
                        connection_registry,
                        app_options,
                    )?))
                }),
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
        let Some(app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        match url.parse::<open_url::ViewerOpenUrl>() {
            Ok(url) => {
                url.open(
                    &app.egui_ctx,
                    &open_url::OpenUrlOptions {
                        // TODO(andreas): should follow_if_http be part of the fragments?
                        follow_if_http: follow_if_http.unwrap_or(false),
                        select_redap_source_when_loaded: true,
                        show_loader: false,
                    },
                    &app.command_sender,
                );
            }
            Err(err) => {
                re_log::warn!("Failed to open URL {url:?}: {err}");
            }
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

        app.egui_ctx
            .request_repaint_after(std::time::Duration::from_millis(10));
    }

    /// Open a new channel for streaming data.
    ///
    /// It is an error to open a channel twice with the same id.
    #[wasm_bindgen]
    pub fn open_channel(&mut self, id: &str, channel_name: &str) {
        let Some(mut app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        if self.log_senders.contains_key(id) {
            re_log::warn!("Channel with id '{}' already exists.", id);
            return;
        }

        let (log_tx, log_rx) = re_log_channel::log_channel(re_log_channel::LogSource::JsChannel {
            channel_name: channel_name.to_owned(),
        });

        app.add_log_receiver(log_rx);
        self.log_senders.insert(id.to_owned(), log_tx);
    }

    /// Close an existing channel for streaming data.
    ///
    /// No-op if the channel is already closed.
    #[wasm_bindgen]
    pub fn close_channel(&mut self, id: &str) {
        let Some(app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        if let Some(log_tx) = self.log_senders.remove(id) {
            log_tx
                .quit(None)
                .warn_on_err_once("Failed to send quit marker");
        }

        // Request a repaint since closing the channel may update the top bar.
        app.egui_ctx
            .request_repaint_after(std::time::Duration::from_millis(10));
    }

    /// Add an rrd to the viewer directly from a byte array.
    #[wasm_bindgen]
    pub fn send_rrd_to_channel(&self, id: &str, data: &[u8]) {
        use std::ops::ControlFlow;
        use std::sync::Arc;
        let Some(app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        if let Some(log_tx) = self.log_senders.get(id) {
            let log_tx = log_tx.clone();
            let data: Vec<u8> = data.to_vec();

            let egui_ctx = app.egui_ctx.clone();

            let ui_waker = Box::new(move || {
                // Spend a few more milliseconds decoding incoming messages,
                // then trigger a repaint (https://github.com/rerun-io/rerun/issues/963):
                egui_ctx.request_repaint_after(std::time::Duration::from_millis(10));
            });

            re_log_encoding::rrd::stream_from_http::web_decode::decode_rrd(
                data,
                Arc::new({
                    move |msg| {
                        ui_waker();
                        use re_log_encoding::rrd::stream_from_http::HttpMessage;
                        match msg {
                            HttpMessage::LogMsg(msg) => {
                                if log_tx.send(msg.into()).is_ok() {
                                    ControlFlow::Continue(())
                                } else {
                                    re_log::info_once!("Failed to dispatch log message to viewer.");
                                    ControlFlow::Break(())
                                }
                            }
                            // TODO(jleibs): Unclear what we want to do here. More data is coming.
                            HttpMessage::Success => ControlFlow::Continue(()),
                            HttpMessage::Failure(err) => {
                                log_tx
                                    .quit(Some(err))
                                    .warn_on_err_once("Failed to send quit marker");
                                ControlFlow::Break(())
                            }
                        }
                    }
                }),
            );
        }
    }

    #[wasm_bindgen]
    pub fn send_table_to_channel(&self, id: &str, data: &[u8]) {
        let Some(app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        if let Some(log_tx) = self.log_senders.get(id) {
            let log_tx = log_tx.clone();

            let cursor = std::io::Cursor::new(data);
            let stream_reader = match arrow::ipc::reader::StreamReader::try_new(cursor, None) {
                Ok(stream_reader) => stream_reader,
                Err(err) => {
                    re_log::error_once!("Failed to interpret data as IPC-encoded arrow: {err}");
                    return;
                }
            };

            let mut batches = match stream_reader.collect::<Result<Vec<_>, _>>() {
                Ok(batches) => batches,
                Err(err) => {
                    re_log::error_once!("Could not read from IPC stream: {err}");
                    return;
                }
            };

            if batches.len() != 1 {
                re_log::warn_once!("Expected exactly one record batch, got {}", batches.len());
                return;
            }

            let record_batch = batches.remove(0);

            let msg = match table_msg_from_record_batch(record_batch) {
                Ok(msg) => msg,
                Err(err) => {
                    re_log::error_once!("Failed to decode Arrow message: {err}");
                    return;
                }
            };

            let egui_ctx = app.egui_ctx.clone();

            match log_tx.send(msg.into()) {
                Ok(_) => egui_ctx.request_repaint_after(std::time::Duration::from_millis(10)),
                Err(err) => {
                    re_log::info_once!("Failed to dispatch log message to viewer: {err}");
                }
            }
        }
    }

    #[wasm_bindgen]
    pub fn get_active_recording_id(&self) -> Option<String> {
        let app = self.runner.app_mut::<crate::App>()?;
        let hub = app.store_hub.as_ref()?;
        let recording = hub.active_recording()?;

        Some(recording.store_id().recording_id().to_string())
    }

    //TODO(#10737): we should refer to logical recordings using store id (recording id is ambibuous)
    #[wasm_bindgen]
    pub fn set_active_recording_id(&self, recording_id: &str) {
        let Some(mut app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        let Some(hub) = app.store_hub.as_mut() else {
            return;
        };

        let Some(store_id) = store_id_from_recording_id(hub, recording_id) else {
            return;
        };

        hub.set_active_recording(store_id);

        app.egui_ctx.request_repaint();
    }

    //TODO(#10737): we should refer to logical recordings using store id (recording id is ambibuous)
    #[wasm_bindgen]
    pub fn get_active_timeline(&self, recording_id: &str) -> Option<String> {
        let mut app = self.runner.app_mut::<crate::App>()?;
        let crate::App {
            store_hub: Some(hub),
            state,
            ..
        } = &mut *app
        else {
            return None;
        };

        let store_id = store_id_from_recording_id(hub, recording_id)?;
        let time_ctrl = state.time_control(&store_id)?;
        Some(time_ctrl.timeline_name().as_str().to_owned())
    }

    /// Set the active timeline.
    ///
    /// This does nothing if the timeline can't be found.
    //TODO(#10737): we should refer to logical recordings using store id (recording id is ambibuous)
    #[wasm_bindgen]
    pub fn set_active_timeline(&self, recording_id: &str, timeline_name: &str) {
        let Some(app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        let Some(hub) = &app.store_hub else {
            return;
        };

        let Some(recording_id) = store_id_from_recording_id(hub, recording_id) else {
            return;
        };

        app.command_sender
            .send_system(SystemCommand::TimeControlCommands {
                store_id: recording_id,
                time_commands: vec![TimeControlCommand::SetActiveTimeline(timeline_name.into())],
            });

        app.egui_ctx.request_repaint();
    }

    //TODO(#10737): we should refer to logical recordings using store id (recording id is ambibuous)
    #[wasm_bindgen]
    pub fn get_time_for_timeline(&self, recording_id: &str, timeline_name: &str) -> Option<f64> {
        let app = self.runner.app_mut::<crate::App>()?;

        let store_id = store_id_from_recording_id(app.store_hub.as_ref()?, recording_id)?;
        let time_ctrl = app.state.time_control(&store_id)?;

        time_ctrl
            .time_for_timeline(timeline_name.into())
            .map(|v| v.as_f64())
    }

    //TODO(#10737): we should refer to logical recordings using store id (recording id is ambibuous)
    #[wasm_bindgen]
    pub fn set_time_for_timeline(&self, recording_id: &str, timeline_name: &str, time: f64) {
        let Some(app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        let Some(hub) = &app.store_hub else {
            return;
        };

        let Some(recording_id) = store_id_from_recording_id(hub, recording_id) else {
            return;
        };

        app.command_sender
            .send_system(SystemCommand::TimeControlCommands {
                store_id: recording_id,
                time_commands: vec![
                    TimeControlCommand::SetActiveTimeline(timeline_name.into()),
                    TimeControlCommand::SetTime(time.into()),
                ],
            });

        app.egui_ctx.request_repaint();
    }

    //TODO(#10737): we should refer to logical recordings using store id (recording id is ambibuous)
    #[wasm_bindgen]
    pub fn get_timeline_time_range(&self, recording_id: &str, timeline_name: &str) -> JsValue {
        let Some(app) = self.runner.app_mut::<crate::App>() else {
            return JsValue::null();
        };
        let crate::App {
            store_hub: Some(hub),
            ..
        } = &*app
        else {
            return JsValue::null();
        };

        let Some(store_id) = store_id_from_recording_id(hub, recording_id) else {
            return JsValue::null();
        };
        let Some(recording) = hub.store_bundle().get(&store_id) else {
            return JsValue::null();
        };

        let Some(time_range) = recording.time_range_for(&timeline_name.into()) else {
            return JsValue::null();
        };

        let min = time_range.min().as_f64();
        let max = time_range.max().as_f64();

        let obj = js_sys::Object::new();
        js_sys::Reflect::set(&obj, &"min".into(), &min.into()).ok_or_log_js_error();
        js_sys::Reflect::set(&obj, &"max".into(), &max.into()).ok_or_log_js_error();

        JsValue::from(obj)
    }

    //TODO(#10737): we should refer to logical recordings using store id (recording id is ambibuous)
    #[wasm_bindgen]
    pub fn get_playing(&self, recording_id: &str) -> Option<bool> {
        let app = self.runner.app_mut::<crate::App>()?;
        let crate::App {
            store_hub: Some(hub),
            state,
            ..
        } = &*app
        else {
            return None;
        };

        let store_id = store_id_from_recording_id(hub, recording_id)?;
        if !hub.store_bundle().contains(&store_id) {
            return None;
        }
        let time_ctrl = state.time_control(&store_id)?;

        Some(time_ctrl.play_state() == PlayState::Playing)
    }

    //TODO(#10737): we should refer to logical recordings using store id (recording id is ambibuous)
    #[wasm_bindgen]
    pub fn set_playing(&self, recording_id: &str, value: bool) {
        let Some(mut app) = self.runner.app_mut::<crate::App>() else {
            return;
        };
        let crate::App {
            store_hub,
            egui_ctx,
            command_sender,
            ..
        } = &mut *app;

        let Some(hub) = store_hub.as_ref() else {
            return;
        };
        let Some(store_id) = store_id_from_recording_id(hub, recording_id) else {
            return;
        };

        let play_state = if value {
            PlayState::Playing
        } else {
            PlayState::Paused
        };

        command_sender.send_system(SystemCommand::TimeControlCommands {
            store_id: store_id.clone(),
            time_commands: vec![TimeControlCommand::SetPlayState(play_state)],
        });
        egui_ctx.request_repaint();
    }

    #[wasm_bindgen]
    pub fn set_credentials(&self, access_token: &str, email: &str) {
        let Some(mut app) = self.runner.app_mut::<crate::App>() else {
            return;
        };
        let crate::App {
            command_sender,
            egui_ctx,
            ..
        } = &mut *app;

        command_sender.send_system(SystemCommand::SetAuthCredentials {
            access_token: access_token.to_owned(),
            email: email.to_owned(),
        });
        egui_ctx.request_repaint();
    }
}

/// Best effort attempt at finding a store id based on the recording id.
fn store_id_from_recording_id(
    store_hub: &re_viewer_context::StoreHub,
    recording_id: &str,
) -> Option<re_log_types::StoreId> {
    store_hub
        .store_bundle()
        .recordings()
        .map(|entity_db| entity_db.store_id())
        .find(|store_id| store_id.recording_id().as_str() == recording_id)
        .cloned()
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

impl From<PanelState> for re_sdk_types::blueprint::components::PanelState {
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
    manifest_url: Option<String>,
    render_backend: Option<String>,
    video_decoder: Option<String>,
    hide_welcome_screen: Option<bool>,
    // allow_fullscreen: Option<bool>, // Not serialized from js as it governs how the `fullscreen` option is used.
    enable_history: Option<bool>,
    // width: Option<String>, // Width & height aren't serialized and only used to configure the canvas.
    // height: Option<String>,
    fallback_token: Option<String>,

    // Hidden `WebViewerOptions`
    // ------------
    viewer_base_url: Option<String>,
    notebook: Option<bool>,
    url: Option<StringOrStringArray>,
    panel_state_overrides: Option<PanelStateOverrides>,
    on_viewer_event: Option<Callback>,
    fullscreen: Option<FullscreenOptions>,
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
    connection_registry: re_redap_client::ConnectionRegistryHandle,
    app_options: AppOptions,
) -> Result<crate::App, re_renderer::RenderContextError> {
    let build_info = re_build_info::build_info!();

    let app_env = crate::AppEnvironment::Web {
        url: cc.integration_info.web_info.location.url.clone(),
    };

    let AppOptions {
        viewer_base_url,
        url,
        manifest_url,
        render_backend,
        video_decoder,
        hide_welcome_screen,
        panel_state_overrides,
        on_viewer_event,
        fullscreen,
        enable_history,

        notebook,

        fallback_token,
    } = app_options;

    if let Some(fallback_token) = fallback_token {
        match re_auth::Jwt::try_from(fallback_token) {
            Ok(token) => connection_registry.set_fallback_token(token),
            Err(err) => {
                re_log::warn!("Failed to parse JWT token: {err}");
            }
        }
    }

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
        persist_state: true,
        is_in_notebook: notebook.unwrap_or(false),
        expect_data_soon: None,
        force_wgpu_backend: render_backend.clone(),
        video_decoder_hw_acceleration,
        hide_welcome_screen: hide_welcome_screen.unwrap_or(false),

        on_event: on_viewer_event.clone().map(|on_event| {
            Rc::new(move |event: crate::ViewerEvent| {
                let Some(event) = serde_json::to_string(&event).ok_or_log_error() else {
                    return;
                };
                on_event
                    .call1(&JsValue::from_str(&event))
                    .ok_or_log_js_error();
            }) as crate::event::ViewerEventCallback
        }),

        fullscreen_options: fullscreen.clone(),
        panel_state_overrides: panel_state_overrides.unwrap_or_default().into(),

        enable_history,
        viewer_base_url,
    };
    crate::customize_eframe_and_setup_renderer(cc)?;

    let mut app = crate::App::new(
        main_thread_token,
        build_info,
        app_env,
        startup_options,
        cc,
        Some(connection_registry),
        AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().expect("Infallible on web"),
    );

    if enable_history {
        install_popstate_listener(&mut app).ok_or_log_js_error();
    }

    if let Some(manifest_url) = manifest_url {
        app.set_examples_manifest_url(manifest_url);
    }

    if let Some(urls) = url {
        for url in urls.into_inner() {
            match url.parse::<open_url::ViewerOpenUrl>() {
                Ok(url) => {
                    url.open(
                        &app.egui_ctx,
                        &open_url::OpenUrlOptions {
                            follow_if_http: false,
                            select_redap_source_when_loaded: true,
                            show_loader: true,
                        },
                        &app.command_sender,
                    );
                }
                Err(err) => {
                    re_log::warn!("Failed to open URL {url:?}: {err}");
                }
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
#[allow(clippy::allow_attributes, clippy::unwrap_used)] // This is only run by rerun employees, so it's fine to panic
#[wasm_bindgen]
pub fn set_email(email: String) {
    let mut config = re_analytics::Config::load().unwrap().unwrap_or_default();
    config.opt_in_metadata.insert("email".into(), email.into());

    config.save().unwrap();
}

/// Returns the [`TableMsg`] back from a encoded record batch.
// This is required to send bytes around in the notebook.
// If you ever change this, you also need to adapt `notebook.py` too.
fn table_msg_from_record_batch(
    mut data: RecordBatch,
) -> Result<TableMsg, Box<dyn std::error::Error>> {
    let id = data
        .schema_metadata_mut()
        .remove("__table_id")
        .ok_or("encoded record batch is missing `__table_id` metadata.")?;

    Ok(TableMsg {
        id: TableId::new(id),
        data,
    })
}

#[cfg(test)]
mod tests {
    use arrow::ArrowError;
    use arrow::array::{RecordBatch, RecordBatchOptions};

    use super::*;

    /// Returns the [`TableMsg`] encoded as a record batch.
    // This is required to send bytes to a viewer running in a notebook.
    // If you ever change this, you also need to adapt `notebook.py` too.
    pub fn to_arrow_encoded(table: &TableMsg) -> Result<RecordBatch, ArrowError> {
        let current_schema = table.data.schema();
        let mut metadata = current_schema.metadata().clone();
        metadata.insert("__table_id".to_owned(), table.id.as_str().to_owned());

        // Create a new schema with the updated metadata
        let new_schema = Arc::new(arrow::datatypes::Schema::new_with_metadata(
            current_schema.fields().clone(),
            metadata,
        ));

        // Create a new record batch with the same data but updated schema
        RecordBatch::try_new_with_options(
            new_schema,
            table.data.columns().to_vec(),
            &RecordBatchOptions::default(),
        )
    }

    #[test]
    fn table_msg_encoded_roundtrip() {
        use arrow::array::{ArrayRef, StringArray, UInt64Array};
        use arrow::datatypes::{DataType, Field, Schema};

        let data = {
            let schema = Arc::new(Schema::new_with_metadata(
                vec![
                    Field::new("id", DataType::UInt64, false),
                    Field::new("name", DataType::Utf8, false),
                ],
                Default::default(),
            ));

            // Create a UInt64 array
            let id_array = UInt64Array::from(vec![1, 2, 3, 4, 5]);

            // Create a String array
            let name_array = StringArray::from(vec![
                "Alice",
                "Bob",
                "Charlie",
                "Dave",
                "http://www.rerun.io",
            ]);

            // Convert arrays to ArrayRef (trait objects)
            let arrays: Vec<ArrayRef> = vec![
                Arc::new(id_array) as ArrayRef,
                Arc::new(name_array) as ArrayRef,
            ];

            // Create a RecordBatch
            ArrowRecordBatch::try_new_with_options(schema, arrays, &RecordBatchOptions::default())
                .unwrap()
        };

        let msg = TableMsg {
            id: TableId::new("test123".to_owned()),
            data,
        };

        let encoded = to_arrow_encoded(&msg).expect("to encoded failed");
        let decoded = table_msg_from_record_batch(encoded).expect("from concatenated failed");

        assert_eq!(msg, decoded);
    }
}
