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

/// Whether the current browser runs on WebKit.
///
/// This is true for Safari, every browser on iOS, and apps that embed WebKit.
/// Use it for workarounds for WebKit bugs and limits.
pub fn is_webkit() -> bool {
    cfg_select! {
        target_arch = "wasm32" => user_agent().is_some_and(|user_agent| is_webkit_user_agent(&user_agent)),
        _ => false,
    }
}

/// Whether the given user agent string belongs to a WebKit browser.
///
/// Chromium-based browsers also say `AppleWebKit/` in their user agent, so we rule them out by `Chrome/`.
/// Chrome on iOS says `CriOS/` instead of `Chrome/`. It runs on WebKit, so it counts.
#[cfg(any(target_arch = "wasm32", test))]
fn is_webkit_user_agent(user_agent: &str) -> bool {
    user_agent.contains("AppleWebKit/") && !user_agent.contains("Chrome/")
}

/// Whether the current browser is Firefox.
pub fn is_firefox() -> bool {
    cfg_select! {
        target_arch = "wasm32" => user_agent()
            .is_some_and(|user_agent| user_agent.to_lowercase().contains("firefox")),
        _ => false,
    }
}

#[cfg(target_arch = "wasm32")]
fn user_agent() -> Option<String> {
    window().ok()?.navigator().user_agent().ok()
}

#[cfg(test)]
mod tests {
    use super::is_webkit_user_agent;

    #[test]
    fn webkit_user_agents() {
        let webkit = [
            // Safari on macOS
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.2 Safari/605.1.15",
            // Safari on iOS
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1",
            // Chrome on iOS
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/138.0.7204.156 Mobile/15E148 Safari/604.1",
            // Firefox on iOS
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/141.0 Mobile/15E148 Safari/605.1.15",
            // In-app browser on iOS, without a `Safari/` token
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148",
        ];
        for user_agent in webkit {
            assert!(
                is_webkit_user_agent(user_agent),
                "expected WebKit: {user_agent:?}"
            );
        }

        let not_webkit = [
            // Chrome on macOS
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36",
            // Edge on Windows
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36 Edg/138.0.0.0",
            // Opera on Windows
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36 OPR/120.0.0.0",
            // Android WebView
            "Mozilla/5.0 (Linux; Android 14; Pixel 8 Build/AP2A.240805.005; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/138.0.7204.157 Mobile Safari/537.36",
            // Headless Chrome, as in CI
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/138.0.0.0 Safari/537.36",
            // Firefox on macOS
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:141.0) Gecko/20100101 Firefox/141.0",
            "",
        ];
        for user_agent in not_webkit {
            assert!(
                !is_webkit_user_agent(user_agent),
                "expected not WebKit: {user_agent:?}"
            );
        }
    }
}
