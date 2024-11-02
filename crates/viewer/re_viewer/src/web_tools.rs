//! Web-specific tools used by various parts of the application.

use anyhow::Context as _;
use re_log::ResultExt;
use serde::Deserialize;
use std::{ops::ControlFlow, sync::Arc};
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsError;
use wasm_bindgen::JsValue;
use web_sys::Window;

pub trait JsResultExt<T> {
    /// Logs an error if the result is an error and returns the result.
    fn ok_or_log_js_error(self) -> Option<T>;

    /// Logs an error if the result is an error and returns the result, but only once.
    #[allow(unused)]
    fn ok_or_log_js_error_once(self) -> Option<T>;

    /// Log a warning if there is an `Err`, but only log the exact same message once.
    #[allow(unused)]
    fn warn_on_js_err_once(self, msg: impl std::fmt::Display) -> Option<T>;

    /// Unwraps in debug builds otherwise logs an error if the result is an error and returns the result.
    #[allow(unused)]
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

pub fn window() -> Result<Window, JsValue> {
    web_sys::window().ok_or_else(|| js_error("failed to get window object"))
}

enum EndpointCategory {
    /// Could be a local path (`/foo.rrd`) or a remote url (`http://foo.com/bar.rrd`).
    ///
    /// Could be a link to either an `.rrd` recording or a `.rbl` blueprint.
    HttpRrd(String),

    /// gRPC Rerun Data Platform URL, e.g. `rrdp://ip:port/recording/1234`
    DataPlatform(String),

    /// A remote Rerun server.
    WebSocket(String),

    /// An eventListener for rrd posted from containing html
    WebEventListener(String),
}

impl EndpointCategory {
    fn categorize_uri(uri: String) -> Self {
        if uri.starts_with("http") || uri.ends_with(".rrd") || uri.ends_with(".rbl") {
            Self::HttpRrd(uri)
        } else if uri.starts_with("rrdp://") {
            Self::DataPlatform(uri)
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

        #[cfg(feature = "rrdp")]
        EndpointCategory::DataPlatform(url) => {
            re_rrdp_comms::stream_recording(url, Some(ui_waker)).map_err(|err| err.into())
        }
        #[cfg(not(feature = "rrdp"))]
        EndpointCategory::DataPlatform(_url) => {
            anyhow::bail!("Missing 'rrdp' feature flag");
        }
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
    #[inline]
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

    #[inline]
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
            for item in array {
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
