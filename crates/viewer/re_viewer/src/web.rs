//! Main entry-point of the web app.

#![allow(clippy::mem_forget)] // False positives from #[wasm_bindgen] macro

use ahash::HashMap;
use arrow::{array::RecordBatch, error::ArrowError};
use serde::Deserialize;
use std::rc::Rc;
use std::str::FromStr as _;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

use re_log::ResultExt as _;
use re_log_types::{TableId, TableMsg};
use re_memory::AccountingAllocator;
use re_viewer_context::{AsyncRuntimeHandle, SystemCommand, SystemCommandSender as _};

use crate::app_state::recording_config_entry;
use crate::history::install_popstate_listener;
use crate::web_tools::{
    string_from_js_value, url_to_receiver, Callback, JsResultExt as _, StringOrStringArray,
};

#[global_allocator]
static GLOBAL: AccountingAllocator<std::alloc::System> =
    AccountingAllocator::new(std::alloc::System);

struct Channel {
    log_tx: re_smart_channel::Sender<re_log_types::LogMsg>,
    table_tx: crossbeam::channel::Sender<re_log_types::TableMsg>,
}

#[wasm_bindgen]
pub struct WebHandle {
    runner: eframe::WebRunner,

    /// A dedicated smart channel used by the [`WebHandle::add_rrd_from_bytes`] API.
    ///
    /// This exists because the direct bytes API is expected to submit many small RRD chunks
    /// and allocating a new tx pair for each chunk doesn't make sense.
    tx_channels: HashMap<String, Channel>,

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
        if let Some(rx) = url_to_receiver(
            app.egui_ctx.clone(),
            follow_if_http,
            url.to_owned(),
            app.command_sender.clone(),
        ) {
            app.add_log_receiver(rx);
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

        let (log_tx, log_rx) = re_smart_channel::smart_channel(
            re_smart_channel::SmartMessageSource::JsChannelPush,
            re_smart_channel::SmartChannelSource::JsChannel {
                channel_name: channel_name.to_owned(),
            },
        );
        let (table_tx, table_rx) = crossbeam::channel::unbounded();

        app.add_log_receiver(log_rx);
        app.add_table_receiver(table_rx);
        self.tx_channels
            .insert(id.to_owned(), Channel { log_tx, table_tx });
    }

    /// Close an existing channel for streaming data.
    ///
    /// No-op if the channel is already closed.
    #[wasm_bindgen]
    pub fn close_channel(&mut self, id: &str) {
        let Some(app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        if let Some(channel) = self.tx_channels.remove(id) {
            channel
                .log_tx
                .quit(None)
                .warn_on_err_once("Failed to send quit marker");
            drop(channel.table_tx);
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

        if let Some(channel) = self.tx_channels.get(id) {
            let tx = channel.log_tx.clone();
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

    #[wasm_bindgen]
    pub fn send_table_to_channel(&self, id: &str, data: &[u8]) {
        let Some(app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        if let Some(channel) = self.tx_channels.get(id) {
            let tx = channel.table_tx.clone();

            let cursor = std::io::Cursor::new(data);
            let stream_reader = arrow::ipc::reader::StreamReader::try_new(cursor, None) else {
                re_log::error_once!("Failed to create cursor");
                return;
            };

            let encoded = &stream_reader.collect::<Result<Vec<_>, _>>() else {
                re_log::error_once!("Could not read from IPC stream");
                return;
            };

            let msg = from_arrow_encoded(encoded[0]) else {
                re_log::error_once!("Failed to decode Arrow message");
                return;
            };

            let egui_ctx = app.egui_ctx.clone();

            if tx.send(msg).is_ok() {
                egui_ctx.request_repaint_after(std::time::Duration::from_millis(10));
            } else {
                re_log::info_once!("Failed to dispatch log message to viewer.");
            }
        }
    }

    #[wasm_bindgen]
    pub fn get_active_recording_id(&self) -> Option<String> {
        let app = self.runner.app_mut::<crate::App>()?;
        let hub = app.store_hub.as_ref()?;
        let recording = hub.active_recording()?;

        Some(recording.store_id().to_string())
    }

    #[wasm_bindgen]
    pub fn set_active_recording_id(&self, store_id: &str) {
        let Some(mut app) = self.runner.app_mut::<crate::App>() else {
            return;
        };

        let Some(hub) = app.store_hub.as_mut() else {
            return;
        };
        let store_id = re_log_types::StoreId::from_string(
            re_log_types::StoreKind::Recording,
            store_id.to_owned(),
        );
        if !hub.store_bundle().contains(&store_id) {
            return;
        };

        hub.set_activate_recording(store_id);

        app.egui_ctx.request_repaint();
    }

    #[wasm_bindgen]
    pub fn get_active_timeline(&self, store_id: &str) -> Option<String> {
        let mut app = self.runner.app_mut::<crate::App>()?;
        let crate::App {
            store_hub: Some(ref hub),
            state,
            ..
        } = &mut *app
        else {
            return None;
        };

        let store_id = re_log_types::StoreId::from_string(
            re_log_types::StoreKind::Recording,
            store_id.to_owned(),
        );
        if !hub.store_bundle().contains(&store_id) {
            return None;
        };

        let rec_cfg = state.recording_config_mut(&store_id)?;
        let time_ctrl = rec_cfg.time_ctrl.read();
        Some(time_ctrl.timeline().name().as_str().to_owned())
    }

    /// Set the active timeline.
    ///
    /// This does nothing if the timeline can't be found.
    #[wasm_bindgen]
    pub fn set_active_timeline(&self, store_id: &str, timeline_name: &str) {
        let Some(mut app) = self.runner.app_mut::<crate::App>() else {
            return;
        };
        let crate::App {
            store_hub: Some(ref hub),
            state,
            egui_ctx,
            ..
        } = &mut *app
        else {
            return;
        };

        let store_id = re_log_types::StoreId::from_string(
            re_log_types::StoreKind::Recording,
            store_id.to_owned(),
        );
        let Some(recording) = hub.store_bundle().get(&store_id) else {
            return;
        };
        let rec_cfg =
            recording_config_entry(&mut state.recording_configs, store_id.clone(), recording);

        let Some(timeline) = recording.timelines().get(&timeline_name.into()).copied() else {
            re_log::warn!("Failed to find timeline '{timeline_name}' in {store_id}");
            return;
        };

        rec_cfg.time_ctrl.write().set_timeline(timeline);

        egui_ctx.request_repaint();
    }

    #[wasm_bindgen]
    pub fn get_time_for_timeline(&self, store_id: &str, timeline_name: &str) -> Option<f64> {
        let app = self.runner.app_mut::<crate::App>()?;

        let store_id = re_log_types::StoreId::from_string(
            re_log_types::StoreKind::Recording,
            store_id.to_owned(),
        );
        let rec_cfg = app.state.recording_config(&store_id)?;

        let time_ctrl = rec_cfg.time_ctrl.read();
        time_ctrl
            .time_for_timeline(timeline_name.into())
            .map(|v| v.as_f64())
    }

    #[wasm_bindgen]
    pub fn set_time_for_timeline(&self, store_id: &str, timeline_name: &str, time: f64) {
        let Some(mut app) = self.runner.app_mut::<crate::App>() else {
            return;
        };
        let crate::App {
            store_hub: Some(ref hub),
            state,
            egui_ctx,
            ..
        } = &mut *app
        else {
            return;
        };

        let store_id = re_log_types::StoreId::from_string(
            re_log_types::StoreKind::Recording,
            store_id.to_owned(),
        );
        let Some(recording) = hub.store_bundle().get(&store_id) else {
            return;
        };
        let rec_cfg =
            recording_config_entry(&mut state.recording_configs, store_id.clone(), recording);
        let Some(timeline) = recording.timelines().get(&timeline_name.into()).copied() else {
            re_log::warn!("Failed to find timeline '{timeline_name}' in {store_id}");
            return;
        };

        rec_cfg
            .time_ctrl
            .write()
            .set_timeline_and_time(timeline, time);
        egui_ctx.request_repaint();
    }

    #[wasm_bindgen]
    pub fn get_timeline_time_range(&self, store_id: &str, timeline_name: &str) -> JsValue {
        let Some(app) = self.runner.app_mut::<crate::App>() else {
            return JsValue::null();
        };
        let crate::App {
            store_hub: Some(ref hub),
            ..
        } = &*app
        else {
            return JsValue::null();
        };

        let store_id = re_log_types::StoreId::from_string(
            re_log_types::StoreKind::Recording,
            store_id.to_owned(),
        );
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

    #[wasm_bindgen]
    pub fn get_playing(&self, store_id: &str) -> Option<bool> {
        let app = self.runner.app_mut::<crate::App>()?;
        let crate::App {
            store_hub: Some(ref hub),
            state,
            ..
        } = &*app
        else {
            return None;
        };

        let store_id = re_log_types::StoreId::from_string(
            re_log_types::StoreKind::Recording,
            store_id.to_owned(),
        );
        if !hub.store_bundle().contains(&store_id) {
            return None;
        };
        let rec_cfg = state.recording_config(&store_id)?;

        let time_ctrl = rec_cfg.time_ctrl.read();
        Some(time_ctrl.play_state() == re_viewer_context::PlayState::Playing)
    }

    #[wasm_bindgen]
    pub fn set_playing(&self, store_id: &str, value: bool) {
        let Some(mut app) = self.runner.app_mut::<crate::App>() else {
            return;
        };
        let crate::App {
            store_hub,
            state,
            egui_ctx,
            ..
        } = &mut *app;

        let Some(hub) = store_hub.as_ref() else {
            return;
        };
        let store_id = re_log_types::StoreId::from_string(
            re_log_types::StoreKind::Recording,
            store_id.to_owned(),
        );
        let Some(recording) = hub.store_bundle().get(&store_id) else {
            return;
        };
        let rec_cfg = recording_config_entry(&mut state.recording_configs, store_id, recording);

        let play_state = if value {
            re_viewer_context::PlayState::Playing
        } else {
            re_viewer_context::PlayState::Paused
        };

        rec_cfg
            .time_ctrl
            .write()
            .set_play_state(recording.times_per_timeline(), play_state);
        egui_ctx.request_repaint();
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
    callbacks: Option<Callbacks>,
    fullscreen: Option<FullscreenOptions>,
    enable_history: Option<bool>,

    notebook: Option<bool>,
    persist: Option<bool>,
}

// Keep in sync with `index.ts`.
#[derive(Clone, Deserialize)]
pub struct Callbacks {
    /// Fired when the selection changes.
    ///
    /// This event is fired each time any part of the event payload changes,
    /// this includes for example clicking on different parts of the same
    /// entity in a 2D or 3D view.
    pub on_selectionchange: Callback,

    /// Fired when the a different timeline is selected.
    pub on_timelinechange: Callback,

    /// Fired when the timepoint changes.
    ///
    /// Does not fire when `on_seek` is called.
    pub on_timeupdate: Callback,

    /// Fired when the timeline is paused.
    pub on_pause: Callback,

    /// Fired when the timeline is played.
    pub on_play: Callback,
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

/// Callback selection item meant for serialization into JS.
///
/// We do this because the selection item we expose from the Rust API
/// is not as nice to work with from JS when serialized into JSON.
///
/// One example of that is `EntityPath` being serialized as an array of
/// path parts, instead of a single string, and we don't want the joining
/// logic to live in multiple places.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum JsCallbackSelectionItem {
    Entity {
        entity_path: String,
        instance_id: Option<u64>,
        view_name: Option<String>,
        position: Option<glam::Vec3>,
    },

    View {
        view_id: String,
        view_name: String,
    },

    Container {
        container_id: String,
        container_name: String,
    },
}

impl From<crate::callback::CallbackSelectionItem> for JsCallbackSelectionItem {
    fn from(v: crate::callback::CallbackSelectionItem) -> Self {
        use crate::callback::CallbackSelectionItem as Item;
        match v {
            Item::Entity {
                entity_path,
                instance_id,
                view_name,
                position,
            } => Self::Entity {
                entity_path: entity_path.to_string(),
                instance_id: instance_id.specific_index().map(|id| id.get()),
                view_name,
                position,
            },
            Item::View { view_id, view_name } => Self::View {
                view_id: view_id.uuid().to_string(),
                view_name,
            },
            Item::Container {
                container_id,
                container_name,
            } => Self::Container {
                container_id: container_id.uuid().to_string(),
                container_name,
            },
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
        callbacks,
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

        callbacks: callbacks.clone().map(|opts| crate::Callbacks {
            on_selection_change: Rc::new(move |selection| {
                // Express the collection as a flat list of items.
                let array = js_sys::Array::new_with_length(selection.len() as u32);
                for (i, item) in selection.into_iter().enumerate() {
                    let Some(value) =
                        serde_wasm_bindgen::to_value(&JsCallbackSelectionItem::from(item))
                            .map_err(|v| v.into())
                            .ok_or_log_js_error()
                    else {
                        continue;
                    };
                    array.set(i as u32, value);
                }
                opts.on_selectionchange.call1(&array).ok_or_log_js_error();
            }),

            on_timeline_change: Rc::new(move |timeline, time| {
                if let Err(err) = opts.on_timelinechange.call2(
                    &JsValue::from_str(timeline.name().as_str()),
                    &JsValue::from_f64(time.as_f64()),
                ) {
                    re_log::error!("{}", string_from_js_value(err));
                };
            }),
            on_time_update: Rc::new(move |time| {
                if let Err(err) = opts.on_timeupdate.call1(&JsValue::from_f64(time.as_f64())) {
                    re_log::error!("{}", string_from_js_value(err));
                }
            }),
            on_play: Rc::new(move || {
                if let Err(err) = opts.on_play.call0() {
                    re_log::error!("{}", string_from_js_value(err));
                }
            }),
            on_pause: Rc::new(move || {
                if let Err(err) = opts.on_pause.call0() {
                    re_log::error!("{}", string_from_js_value(err));
                }
            }),
        }),

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
        cc,
        AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().expect("Infallible on web"),
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
            if let Some(receiver) = url_to_receiver(
                cc.egui_ctx.clone(),
                follow_if_http,
                url,
                app.command_sender.clone(),
            ) {
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
#[allow(clippy::unwrap_used)] // This is only run by rerun employees, so it's fine to panic
pub fn set_email(email: String) {
    let mut config = re_analytics::Config::load().unwrap().unwrap_or_default();
    config.opt_in_metadata.insert("email".into(), email.into());
    config.save().unwrap();
}

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
    RecordBatch::try_new(new_schema, table.data.columns().to_vec())
}

/// Returns the [`TableMsg`] back from a encoded record batch.
// This is required to send bytes around in the notebook.
// If you ever change this, you also need to adapt `notebook.py` too.
pub fn from_arrow_encoded(data: &RecordBatch) -> Option<TableMsg> {
    re_log::info!("{:?}", data);
    let mut metadata = data.schema().metadata().clone();
    let id = metadata.remove("__table_id").expect("this has to be here");

    let data = RecordBatch::try_new(
        Arc::new(arrow::datatypes::Schema::new_with_metadata(
            data.schema().fields().clone(),
            metadata,
        )),
        data.columns().to_vec(),
    )
    .ok()?;

    Some(TableMsg {
        id: TableId::new(id),
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_msg_encoded_roundtrip() {
        use arrow::{
            array::{ArrayRef, StringArray, UInt64Array},
            datatypes::{DataType, Field, Schema},
        };

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
            ArrowRecordBatch::try_new(schema, arrays).unwrap()
        };

        let msg = TableMsg {
            id: TableId::new("test123".to_owned()),
            data,
        };

        let encoded = to_arrow_encoded(&msg).expect("to encoded failed");
        let decoded = from_arrow_encoded(&encoded).expect("from concatenated failed");

        assert_eq!(msg, decoded);
    }
}
