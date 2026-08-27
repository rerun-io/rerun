//! JS error conversion.

use wasm_bindgen::{JsCast as _, JsValue};

/// An error reported by a JS API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error(String);

impl Error {
    /// Creates an error from a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    /// Creates an error from a JS value and operation context.
    pub(crate) fn from_js_value(context: &str, value: &JsValue) -> Self {
        Self(format!("{context}: {}", format_js_value(value)))
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<&JsValue> for Error {
    fn from(value: &JsValue) -> Self {
        Self(format_js_value(value))
    }
}

impl From<JsValue> for Error {
    fn from(value: JsValue) -> Self {
        Self::from(&value)
    }
}

fn format_js_value(value: &JsValue) -> String {
    if let Some(value) = value.as_string() {
        return value;
    }

    if let Some(exception) = value.dyn_ref::<web_sys::DomException>() {
        let name = exception.name();
        let message = exception.message();
        return if message.is_empty() {
            name
        } else {
            format!("{name}: {message}")
        };
    }

    if let Some(error) = value.dyn_ref::<js_sys::Error>() {
        return String::from(error.to_string());
    }

    format!("{value:#?}")
}

#[cfg(test)]
mod tests {
    use wasm_bindgen::JsValue;
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn formats_string() {
        assert_eq!(
            super::format_js_value(&JsValue::from_str("failed")),
            "failed"
        );
    }

    #[wasm_bindgen_test]
    fn formats_dom_exception() {
        let exception: JsValue =
            web_sys::DomException::new_with_message_and_name("denied", "SecurityError")
                .expect("DOMException should be constructible")
                .into();
        assert_eq!(super::format_js_value(&exception), "SecurityError: denied");
    }

    #[wasm_bindgen_test]
    fn formats_error() {
        let error: JsValue = js_sys::Error::new("failed").into();
        assert_eq!(super::format_js_value(&error), "Error: failed");
    }

    #[wasm_bindgen_test]
    fn error_implements_std_error() {
        let error = super::Error::new("failed");
        let error: &dyn std::error::Error = &error;
        assert_eq!(error.to_string(), "failed");
    }
}
