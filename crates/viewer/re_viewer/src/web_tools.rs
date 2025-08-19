//! Web-specific tools used by various parts of the application.

use serde::Deserialize;
use wasm_bindgen::{JsCast as _, JsError, JsValue};
use web_sys::Window;

use re_log::ResultExt as _;

pub trait JsResultExt<T> {
    /// Logs an error if the result is an error and returns the result.
    fn ok_or_log_js_error(self) -> Option<T>;

    /// Logs an error if the result is an error and returns the result, but only once.
    #[allow(unused)]
    fn ok_or_log_js_error_once(self) -> Option<T>;

    /// Log a warning if there is an `Err`, but only log the exact same message once.
    #[allow(unused)]
    fn warn_on_js_err_once(self, msg: impl std::fmt::Display) -> Option<T>;
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

// Can't deserialize `Option<js_sys::Function>` directly, so newtype it is.
#[derive(Clone, Deserialize)]
#[repr(transparent)]
pub struct Callback(#[serde(with = "serde_wasm_bindgen::preserve")] js_sys::Function);

impl Callback {
    #[inline]
    pub fn call0(&self) -> Result<JsValue, JsValue> {
        let window: JsValue = window()?.into();
        self.0.call0(&window)
    }

    #[inline]
    pub fn call1(&self, arg0: &JsValue) -> Result<JsValue, JsValue> {
        let window: JsValue = window()?.into();
        self.0.call1(&window, arg0)
    }

    #[inline]
    pub fn call2(&self, arg0: &JsValue, arg1: &JsValue) -> Result<JsValue, JsValue> {
        let window: JsValue = window()?.into();
        self.0.call2(&window, arg0, arg1)
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
