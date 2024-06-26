//! Web-specific tools used by various parts of the application.

use anyhow::Context as _;
use re_log::ResultExt;
use re_viewer_context::StoreHub;
use re_viewer_context::{CommandSender, SystemCommand, SystemCommandSender as _};
use serde::Deserialize;
use std::{ops::ControlFlow, sync::Arc};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsError;
use wasm_bindgen::JsValue;
use web_sys::History;
use web_sys::UrlSearchParams;
use web_sys::Window;

pub trait JsResultExt<T> {
    /// Logs an error if the result is an error and returns the result.
    fn ok_or_log_js_error(self) -> Option<T>;

    /// Logs an error if the result is an error and returns the result, but only once.
    fn ok_or_log_js_error_once(self) -> Option<T>;

    /// Log a warning if there is an `Err`, but only log the exact same message once.
    fn warn_on_js_err_once(self, msg: impl std::fmt::Display) -> Option<T>;

    /// Unwraps in debug builds otherwise logs an error if the result is an error and returns the result.
    fn unwrap_debug_or_log_js_error(self) -> Option<T>;
}

impl<T> JsResultExt<T> for Result<T, JsValue> {
    fn ok_or_log_js_error(self) -> Option<T> {
        self.map_err(string_from_js_value).ok_or_log_error()
    }

    fn ok_or_log_js_error_once(self) -> Option<T> {
        self.map_err(string_from_js_value).ok_or_log_error_once()
    }

    fn warn_on_js_err_once(self, msg: impl std::fmt::Display) -> Option<T> {
        self.map_err(string_from_js_value).warn_on_err_once(msg)
    }

    fn unwrap_debug_or_log_js_error(self) -> Option<T> {
        self.map_err(string_from_js_value)
            .unwrap_debug_or_log_error()
    }
}

/// Useful in error handlers
#[allow(clippy::needless_pass_by_value)]
pub fn string_from_js_value(s: wasm_bindgen::JsValue) -> String {
    // it's already a string
    if let Some(s) = s.as_string() {
        return s;
    }

    // it's an Error, call `toString` instead
    if let Some(s) = s.dyn_ref::<js_sys::Error>() {
        return format!("{}", s.to_string());
    }

    format!("{s:#?}")
}

pub fn js_error(msg: impl std::fmt::Display) -> JsValue {
    JsError::new(&msg.to_string()).into()
}

pub fn set_url_parameter_and_refresh(key: &str, value: &str) -> Result<(), wasm_bindgen::JsValue> {
    let window = window()?;
    let location = window.location();

    let url = web_sys::Url::new(&location.href()?)?;
    url.search_params().set(key, value);

    location.assign(&url.href())
}

/// Listen for `popstate` event, which comes when the user hits the back/forward buttons.
///
/// <https://developer.mozilla.org/en-US/docs/Web/API/Window/popstate_event>
pub fn install_popstate_listener(
    egui_ctx: egui::Context,
    command_sender: CommandSender,
) -> Result<(), JsValue> {
    let closure = Closure::wrap(Box::new({
        let mut prev = history()?.current_entry()?;
        move |_: web_sys::Event| {
            handle_popstate(&mut prev, &egui_ctx, &command_sender).ok_or_log_js_error();
        }
    }) as Box<dyn FnMut(_)>);

    window()?
        .add_event_listener_with_callback("popstate", closure.as_ref().unchecked_ref())
        .ok_or_log_js_error();
    closure.forget();
    Ok(())
}

fn handle_popstate(
    prev: &mut Option<HistoryEntry>,
    egui_ctx: &egui::Context,
    command_sender: &CommandSender,
) -> Result<(), JsValue> {
    let current = history()?.current_entry()?;
    if &current == prev {
        return Ok(());
    }

    let Some(entry) = current else {
        // the user navigated back to the history entry where the viewer was initially opened
        // in that case they likely expect to land back at the welcome screen:
        command_sender.send_system(SystemCommand::ActivateApp(StoreHub::welcome_screen_app_id()));

        return Ok(());
    };

    let follow_if_http = false;
    for url in &entry.urls {
        // we continue in case of errors because some receivers may be valid
        let Some(receiver) =
            url_to_receiver(egui_ctx.clone(), follow_if_http, url.clone()).ok_or_log_error()
        else {
            continue;
        };

        // We may be here because the user clicked Back/Forward in the browser while trying
        // out examples. If we re-download the same file we should clear out the old data first.
        command_sender.send_system(SystemCommand::ClearSourceAndItsStores(
            receiver.source().clone(),
        ));
        command_sender.send_system(SystemCommand::AddReceiver(receiver));
    }

    *prev = Some(entry);
    egui_ctx.request_repaint();

    Ok(())
}

pub fn go_back() -> Option<()> {
    let history = history().ok_or_log_js_error()?;
    history.back().ok_or_log_js_error()
}

pub fn go_forward() -> Option<()> {
    let history = history().ok_or_log_js_error()?;
    history.forward().ok_or_log_js_error()
}

/// A history entry is actually stored in two places:
/// - State object
/// - URL
///
/// Ideally we wouldn't have to, but we want two things:
/// - Listen to `popstate` events and handle navigations client-side,
///   so that the forward/back buttons can be used to navigate between
///   examples and the welcome screen.
/// - Add a `?url` query param to the address bar when navigating to
///   an example, so that examples can be shared directly by just
///   copying the link.
#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HistoryEntry {
    /// Data source URL
    ///
    /// We support loading multiple URLs at the same time
    ///
    /// `?url=`
    pub urls: Vec<String>,
}

// Builder methods
impl HistoryEntry {
    pub const KEY: &'static str = "__rerun";

    pub fn new() -> Self {
        Self { urls: Vec::new() }
    }

    /// Set the URL of the RRD to load when using this entry.
    pub fn rrd_url(mut self, url: String) -> Self {
        self.urls.push(url);
        self
    }
}

// Serialization
impl HistoryEntry {
    pub fn to_query_string(&self) -> Result<String, JsValue> {
        use std::fmt::Write;

        let params = UrlSearchParams::new()?;
        for url in &self.urls {
            params.append("url", url);
        }
        let mut out = "?".to_owned();
        write!(&mut out, "{}", params.to_string()).ok();

        Ok(out)
    }
}

pub fn window() -> Result<Window, JsValue> {
    web_sys::window().ok_or_else(|| js_error("failed to get window object"))
}

pub fn history() -> Result<History, JsValue> {
    window()?.history()
}

pub trait HistoryExt {
    /// Push a history entry onto the stack, which becomes the latest entry.
    fn push_entry(&self, entry: HistoryEntry) -> Result<(), JsValue>;

    /// Replace the latest entry.
    fn replace_entry(&self, entry: HistoryEntry) -> Result<(), JsValue>;

    /// Get the latest entry.
    fn current_entry(&self) -> Result<Option<HistoryEntry>, JsValue>;
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["window"], js_name = structuredClone)]
    /// The `structuredClone()` method.
    ///
    /// [MDN Documentation](https://developer.mozilla.org/en-US/docs/Web/API/structuredClone)
    pub fn structured_clone(value: &JsValue) -> Result<JsValue, JsValue>;
}

/// Get the current raw history state.
///
/// The return value is an object which may contain properties
/// added by other JS code. We need to be careful about not
/// trampling over those.
///
/// The returned object has been deeply cloned, so it is safe
/// to add our own keys to the object, as it won't update the
/// current browser history.
fn get_raw_state(history: &History) -> Result<JsValue, JsValue> {
    let state = history.state().unwrap_or(JsValue::UNDEFINED);
    if !state.is_object() {
        return Err(JsError::new("history state is not an object").into());
    }

    structured_clone(&state)
}

/// Get the state from `history`, deeply-cloned, and return it with updated values from the given `entry`.
///
/// This does _not_ mutate the browser history.
fn get_updated_state(history: &History, entry: &HistoryEntry) -> Result<JsValue, JsValue> {
    let state = get_raw_state(history)?;
    let key = JsValue::from_str(HistoryEntry::KEY);
    let entry = serde_wasm_bindgen::to_value(entry)?;
    js_sys::Reflect::set(&state, &key, &entry)?;
    Ok(state)
}

impl HistoryExt for History {
    fn push_entry(&self, entry: HistoryEntry) -> Result<(), JsValue> {
        let state = get_updated_state(self, &entry)?;
        let url = entry.to_query_string()?;
        self.push_state_with_url(&state, "", Some(&url))
    }

    fn replace_entry(&self, entry: HistoryEntry) -> Result<(), JsValue> {
        let state = get_updated_state(self, &entry)?;
        let url = entry.to_query_string()?;
        self.replace_state_with_url(&state, "", Some(&url))
    }

    fn current_entry(&self) -> Result<Option<HistoryEntry>, JsValue> {
        let state = get_raw_state(self)?;
        let key = JsValue::from_str(HistoryEntry::KEY);
        let value = js_sys::Reflect::get(&state, &key)?;
        if value.is_undefined() || value.is_null() {
            return Ok(None);
        }

        Ok(Some(serde_wasm_bindgen::from_value(value)?))
    }
}

enum EndpointCategory {
    /// Could be a local path (`/foo.rrd`) or a remote url (`http://foo.com/bar.rrd`).
    ///
    /// Could be a link to either an `.rrd` recording or a `.rbl` blueprint.
    HttpRrd(String),

    /// A remote Rerun server.
    WebSocket(String),

    /// An eventListener for rrd posted from containing html
    WebEventListener(String),
}

impl EndpointCategory {
    fn categorize_uri(uri: String) -> Self {
        if uri.starts_with("http") || uri.ends_with(".rrd") || uri.ends_with(".rbl") {
            Self::HttpRrd(uri)
        } else if uri.starts_with("ws:") || uri.starts_with("wss:") {
            Self::WebSocket(uri)
        } else if uri.starts_with("web_event:") {
            Self::WebEventListener(uri)
        } else {
            // If this is something like `foo.com` we can't know what it is until we connect to it.
            // We could/should connect and see what it is, but for now we just take a wild guess instead:
            re_log::info!("Assuming WebSocket endpoint");
            if uri.contains("://") {
                Self::WebSocket(uri)
            } else {
                Self::WebSocket(format!("{}://{uri}", re_ws_comms::PROTOCOL))
            }
        }
    }
}

/// Start receiving from the given url.
pub fn url_to_receiver(
    egui_ctx: egui::Context,
    follow_if_http: bool,
    url: String,
) -> anyhow::Result<re_smart_channel::Receiver<re_log_types::LogMsg>> {
    let ui_waker = Box::new(move || {
        // Spend a few more milliseconds decoding incoming messages,
        // then trigger a repaint (https://github.com/rerun-io/rerun/issues/963):
        egui_ctx.request_repaint_after(std::time::Duration::from_millis(10));
    });
    match EndpointCategory::categorize_uri(url) {
        EndpointCategory::HttpRrd(url) => Ok(
            re_log_encoding::stream_rrd_from_http::stream_rrd_from_http_to_channel(
                url,
                follow_if_http,
                Some(ui_waker),
            ),
        ),
        EndpointCategory::WebEventListener(url) => {
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
                                re_log::info_once!("Failed to send log message to viewer - closing connection to {url}");
                                ControlFlow::Break(())
                            }
                        }
                        HttpMessage::Success => {
                            tx.quit(None).warn_on_err_once("Failed to send quit marker");
                            ControlFlow::Break(())
                        }
                        HttpMessage::Failure(err) => {
                            tx.quit(Some(err))
                                .warn_on_err_once("Failed to send quit marker");
                            ControlFlow::Break(())
                        }
                    }
                }
            }));
            Ok(rx)
        }
        EndpointCategory::WebSocket(url) => re_data_source::connect_to_ws_url(&url, Some(ui_waker))
            .with_context(|| format!("Failed to connect to WebSocket server at {url}.")),
    }
}

// Can't deserialize `Option<js_sys::Function>` directly, so newtype it is.
#[derive(Clone, Deserialize)]
#[repr(transparent)]
pub struct Callback(#[serde(with = "serde_wasm_bindgen::preserve")] js_sys::Function);

impl Callback {
    pub fn call(&self) -> Result<JsValue, JsValue> {
        let window: JsValue = window()?.into();
        self.0.call0(&window)
    }
}

// Deserializes from JS string or array of strings.
#[derive(Clone)]
pub struct StringOrStringArray(Vec<String>);

impl StringOrStringArray {
    pub fn into_inner(self) -> Vec<String> {
        self.0
    }
}

impl std::ops::Deref for StringOrStringArray {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de> Deserialize<'de> for StringOrStringArray {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        fn from_value(value: JsValue) -> Option<Vec<String>> {
            if let Some(value) = value.as_string() {
                return Some(vec![value]);
            }

            let array = value.dyn_into::<js_sys::Array>().ok()?;
            let mut out = Vec::with_capacity(array.length() as usize);
            for item in array.into_iter() {
                out.push(item.as_string()?);
            }
            Some(out)
        }

        let value = serde_wasm_bindgen::preserve::deserialize(deserializer)?;
        from_value(value)
            .map(Self)
            .ok_or_else(|| serde::de::Error::custom("value is not a string or array of strings"))
    }
}
