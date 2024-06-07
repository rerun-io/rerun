use std::{ops::ControlFlow, sync::Arc};

use anyhow::Context as _;
use serde::Deserialize;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;

use re_log::ResultExt as _;

/// Web-specific tools used by various parts of the application.

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

pub fn set_url_parameter_and_refresh(key: &str, value: &str) -> Result<(), wasm_bindgen::JsValue> {
    let Some(window) = web_sys::window() else {
        return Err("Failed to get window".into());
    };
    let location = window.location();

    let url = web_sys::Url::new(&location.href()?)?;
    url.search_params().set(key, value);

    location.assign(&url.href())
}

/// Percent-encode the given string so you can put it in a URL.
pub fn percent_encode(s: &str) -> String {
    format!("{}", js_sys::encode_uri_component(s))
}

pub fn go_back() -> Option<()> {
    let history = web_sys::window()?
        .history()
        .map_err(|err| format!("Failed to get History API: {}", string_from_js_value(err)))
        .ok_or_log_error()?;
    history
        .back()
        .map_err(|err| format!("Failed to go back: {}", string_from_js_value(err)))
        .ok_or_log_error()
}

pub fn go_forward() -> Option<()> {
    let history = web_sys::window()?
        .history()
        .map_err(|err| format!("Failed to get History API: {}", string_from_js_value(err)))
        .ok_or_log_error()?;
    history
        .forward()
        .map_err(|err| format!("Failed to go forward: {}", string_from_js_value(err)))
        .ok_or_log_error()
}

/// The current percent-encoded URL suffix, e.g. "?foo=bar#baz".
pub fn current_url_suffix() -> Option<String> {
    let location = web_sys::window()?.location();
    let search = location.search().unwrap_or_default();
    let hash = location.hash().unwrap_or_default();
    Some(format!("{search}{hash}"))
}

/// Push a relative url on the web `History`,
/// so that the user can use the back button to navigate to it.
///
/// If this is already the current url, nothing happens.
///
/// The url must be percent encoded.
///
/// Example:
/// ```
/// push_history("foo/bar?baz=qux#fragment");
/// ```
pub fn push_history(new_relative_url: &str) -> Option<()> {
    let current_relative_url = current_url_suffix().unwrap_or_default();

    if current_relative_url == new_relative_url {
        re_log::debug!("Ignoring navigation to {new_relative_url:?} as we're already there");
    } else {
        re_log::debug!(
            "Existing url is {current_relative_url:?}; navigating to {new_relative_url:?}"
        );

        let history = web_sys::window()?
            .history()
            .map_err(|err| format!("Failed to get History API: {}", string_from_js_value(err)))
            .ok_or_log_error()?;
        // Instead of setting state to `null`, try to preserve existing state.
        // This helps with ensuring JS frameworks can perform client-side routing.
        // If we ever need to store anything in `state`, we should rethink how
        // we handle this.
        let existing_state = history.state().unwrap_or(JsValue::NULL);
        history
            .push_state_with_url(&existing_state, "", Some(new_relative_url))
            .map_err(|err| {
                format!(
                    "Failed to push history state: {}",
                    string_from_js_value(err)
                )
            })
            .ok_or_log_error()?;
    }
    Some(())
}

/// Replace the current relative url with an new one.
pub fn replace_history(new_relative_url: &str) -> Option<()> {
    let history = web_sys::window()?
        .history()
        .map_err(|err| format!("Failed to get History API: {}", string_from_js_value(err)))
        .ok_or_log_error()?;
    // NOTE: See `existing_state` in `push_history` above for info on why this is here.
    let existing_state = history.state().unwrap_or(JsValue::NULL);
    history
        .replace_state_with_url(&existing_state, "", Some(new_relative_url))
        .map_err(|err| {
            format!(
                "Failed to push history state: {}",
                string_from_js_value(err)
            )
        })
        .ok_or_log_error()
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
        self.0.call0(&web_sys::window().unwrap())
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
