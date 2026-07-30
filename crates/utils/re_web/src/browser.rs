//! Browser environment access and detection.

/// Returns the current browser window.
#[cfg(target_arch = "wasm32")]
pub fn window() -> Result<web_sys::Window, crate::Error> {
    web_sys::window().ok_or_else(|| crate::Error::new("browser window is unavailable"))
}

/// Returns the current browser location.
#[cfg(target_arch = "wasm32")]
pub fn location() -> Result<web_sys::Location, crate::Error> {
    Ok(window()?.location())
}

/// Returns the current browser history.
#[cfg(target_arch = "wasm32")]
pub fn history() -> Result<web_sys::History, crate::Error> {
    window()?
        .history()
        .map_err(|err| crate::Error::from_js_value("failed to access browser history", &err))
}

/// Returns the URL of the current page.
#[cfg(target_arch = "wasm32")]
pub fn current_page_url() -> Result<String, crate::Error> {
    location()?
        .href()
        .map_err(|err| crate::Error::from_js_value("failed to read the current page URL", &err))
}

/// Sets a query parameter and navigates to the resulting URL.
#[cfg(target_arch = "wasm32")]
pub fn set_url_parameter_and_refresh(key: &str, value: &str) -> Result<(), crate::Error> {
    let location = location()?;
    let href = location
        .href()
        .map_err(|err| crate::Error::from_js_value("failed to read the current page URL", &err))?;
    let url = web_sys::Url::new(&href)
        .map_err(|err| crate::Error::from_js_value("failed to parse the current page URL", &err))?;
    url.search_params().set(key, value);
    location
        .assign(&url.href())
        .map_err(|err| crate::Error::from_js_value("failed to navigate to the updated URL", &err))
}

/// Whether the current browser is Safari.
pub fn is_safari() -> bool {
    cfg_select! {
        target_arch = "wasm32" => {
            use wasm_bindgen::{JsCast as _, JsValue};

            let Ok(window) = window() else {
                return false;
            };

            js_sys::Object::has_own(
                window.unchecked_ref::<js_sys::Object>(),
                &JsValue::from("safari"),
            )
        }
        _ => {
            false
        }
    }
}

/// Whether the current browser is Firefox.
pub fn is_firefox() -> bool {
    cfg_select! {
        target_arch = "wasm32" => {
            window()
                .ok()
                .and_then(|window| window.navigator().user_agent().ok())
                .is_some_and(|user_agent| user_agent.to_lowercase().contains("firefox"))
        }
        _ => {
            false
        }
    }
}
