//! Rerun's usage of the history API on web lives here.
//!
//! A history entry is stored in two places:
//! - State object
//! - URL
//!
//! Ideally we wouldn't have to, but we want two things:
//! - Listen to `popstate` events and handle navigations client-side,
//!   so that the forward/back buttons can be used to navigate between
//!   examples and the welcome screen.
//! - Add a `?url` query param to the address bar when navigating to
//!   an example, so that examples can be shared directly by just
//!   copying the link.

use crate::web_tools::{url_to_receiver, window, JsResultExt as _};
use js_sys::wasm_bindgen;
use parking_lot::Mutex;
use re_viewer_context::{CommandSender, SystemCommand, SystemCommandSender as _};
use std::sync::Arc;
use std::sync::OnceLock;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsError;
use wasm_bindgen::JsValue;
use web_sys::History;
use web_sys::UrlSearchParams;

#[derive(Clone, Default, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

    /// Set the URL of the RRD to load when using this entry.
    #[inline]
    pub fn rrd_url(mut self, url: String) -> Self {
        self.urls.push(url);
        self
    }
}

// Serialization
impl HistoryEntry {
    pub fn to_query_string(&self) -> Result<String, JsValue> {
        use std::fmt::Write as _;

        let params = UrlSearchParams::new()?;
        for url in &self.urls {
            params.append("url", url);
        }
        let mut out = "?".to_owned();
        write!(&mut out, "{}", params.to_string()).ok();

        Ok(out)
    }
}

fn stored_history_entry() -> &'static Arc<Mutex<Option<HistoryEntry>>> {
    static STORED_HISTORY_ENTRY: OnceLock<Arc<Mutex<Option<HistoryEntry>>>> = OnceLock::new();
    STORED_HISTORY_ENTRY.get_or_init(|| Arc::new(Mutex::new(None)))
}

fn get_stored_history_entry() -> Option<HistoryEntry> {
    stored_history_entry().lock().clone()
}

fn set_stored_history_entry(entry: Option<HistoryEntry>) {
    *stored_history_entry().lock() = entry;
}

type EventListener<Event> = dyn FnMut(Event) -> Result<(), JsValue>;

/// Listen for `popstate` event, which comes when the user hits the back/forward buttons.
///
/// <https://developer.mozilla.org/en-US/docs/Web/API/Window/popstate_event>
pub fn install_popstate_listener(app: &mut crate::App) -> Result<(), JsValue> {
    let egui_ctx = app.egui_ctx.clone();
    let command_sender = app.command_sender.clone();

    let closure = Closure::wrap(Box::new({
        move |event: web_sys::PopStateEvent| {
            let new_state = deserialize_from_state(&event.state())?;
            handle_popstate(&egui_ctx, &command_sender, new_state);
            Ok(())
        }
    }) as Box<EventListener<_>>);

    set_stored_history_entry(history()?.current_entry()?);

    window()?
        .add_event_listener_with_callback("popstate", closure.as_ref().unchecked_ref())
        .ok_or_log_js_error();

    app.popstate_listener = Some(PopstateListener::new(closure));

    Ok(())
}

pub struct PopstateListener(Option<Closure<EventListener<web_sys::PopStateEvent>>>);

impl PopstateListener {
    fn new(closure: Closure<EventListener<web_sys::PopStateEvent>>) -> Self {
        Self(Some(closure))
    }
}

impl Drop for PopstateListener {
    fn drop(&mut self) {
        let Some(window) = window().ok_or_log_js_error() else {
            return;
        };

        // The closure is guaranteed to be `Some`, because the field isn't
        // accessed outside of the constructor.
        let Some(closure) = self.0.take() else {
            unreachable!();
        };
        window
            .remove_event_listener_with_callback("popstate", closure.as_ref().unchecked_ref())
            .ok_or_log_js_error();
        drop(closure);
    }
}

fn handle_popstate(
    egui_ctx: &egui::Context,
    command_sender: &CommandSender,
    new_state: Option<HistoryEntry>,
) {
    let prev_state = get_stored_history_entry();

    re_log::debug!("popstate: prev={prev_state:?} new={new_state:?}");

    if prev_state == new_state {
        re_log::debug!("popstate: no change");

        return;
    }

    if new_state.is_none() || new_state.as_ref().is_some_and(|v| v.urls.is_empty()) {
        re_log::debug!("popstate: clear recordings + go to welcome screen");

        // the user navigated back to the history entry where the viewer was initially opened
        // in that case they likely expect to land back at the welcome screen.
        // We just close all recordings, which should automatically show the welcome screen or the redap browser.
        command_sender.send_system(SystemCommand::CloseAllRecordings);

        set_stored_history_entry(new_state);
        egui_ctx.request_repaint();

        return;
    }

    let Some(entry) = new_state else {
        unreachable!();
    };

    let follow_if_http = false;
    for url in &entry.urls {
        // we continue in case of errors because some receivers may be valid
        let Some(receiver) = url_to_receiver(
            egui_ctx.clone(),
            follow_if_http,
            url.clone(),
            command_sender.clone(),
        ) else {
            continue;
        };

        command_sender.send_system(SystemCommand::ClearSourceAndItsStores(
            receiver.source().clone(),
        ));
        command_sender.send_system(SystemCommand::AddReceiver(receiver));

        re_log::debug!("popstate: add receiver {url:?}");
    }

    set_stored_history_entry(Some(entry));
    egui_ctx.request_repaint();
}

pub fn go_back() -> Option<()> {
    let history = history().ok_or_log_js_error()?;
    history.back().ok_or_log_js_error()
}

pub fn go_forward() -> Option<()> {
    let history = history().ok_or_log_js_error()?;
    history.forward().ok_or_log_js_error()
}

pub fn history() -> Result<History, JsValue> {
    window()?.history()
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

    if state.is_undefined() || state.is_null() {
        // no state - return empty object
        return Ok(js_sys::Object::new().into());
    }

    if !state.is_object() {
        // invalid state
        return Err(JsError::new("history state is not an object").into());
    }

    // deeply clone state
    structured_clone(&state)
}

fn deserialize_from_state(state: &JsValue) -> Result<Option<HistoryEntry>, JsValue> {
    let key = JsValue::from_str(HistoryEntry::KEY);
    let value = js_sys::Reflect::get(state, &key)?;
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    let entry = serde_wasm_bindgen::from_value(value)?;
    Ok(Some(entry))
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

pub trait HistoryExt: private::Sealed {
    /// Push a history entry onto the stack, which becomes the latest entry.
    fn push_entry(&self, entry: HistoryEntry) -> Result<(), JsValue>;

    /// Replace the latest entry.
    #[allow(unused)]
    fn replace_entry(&self, entry: HistoryEntry) -> Result<(), JsValue>;

    /// Get the latest entry.
    fn current_entry(&self) -> Result<Option<HistoryEntry>, JsValue>;
}

impl private::Sealed for History {}

impl HistoryExt for History {
    fn push_entry(&self, entry: HistoryEntry) -> Result<(), JsValue> {
        let state = get_updated_state(self, &entry)?;
        let url = entry.to_query_string()?;
        self.push_state_with_url(&state, "", Some(&url))?;
        set_stored_history_entry(Some(entry));

        Ok(())
    }

    fn replace_entry(&self, entry: HistoryEntry) -> Result<(), JsValue> {
        let state = get_updated_state(self, &entry)?;
        let url = entry.to_query_string()?;
        self.replace_state_with_url(&state, "", Some(&url))?;
        set_stored_history_entry(Some(entry));

        Ok(())
    }

    fn current_entry(&self) -> Result<Option<HistoryEntry>, JsValue> {
        let state = get_raw_state(self)?;
        deserialize_from_state(&state)
    }
}

mod private {
    pub trait Sealed {}
}
