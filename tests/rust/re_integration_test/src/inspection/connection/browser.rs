//! Drive the real wasm web viewer in a browser over the Chrome `DevTools` Protocol.

use std::sync::Arc;
use std::time::{Duration, Instant};

use egui_inspection::protocol::{self, Request, Response};
use re_web_viewer_server::{WebViewerData, WebViewerServer, WebViewerServerPort};

use super::{CONNECT_TIMEOUT, TestEnv};

/// The real wasm web viewer running in a browser, driven by calling its `inspect` method over a
/// `headless_chrome` (CDP) JS eval.
pub(in crate::inspection) struct BrowserConnection {
    tab: Arc<headless_chrome::Tab>,

    // Keep these alive for the duration of the test
    _browser: headless_chrome::Browser,
    _server: WebViewerServer,
}

impl BrowserConnection {
    pub(super) fn new(size: egui::Vec2, startup_url: Option<&str>) -> Self {
        // A real `http://127.0.0.1:{port}` origin is required so the viewer's gRPC-web calls to the
        // redap server pass CORS. We load the assets from disk rather than letting
        // `re_web_viewer_server` embed them, so this crate doesn't need the wasm at compile time.
        let data = WebViewerData::from_dir(&TestEnv::get().resolve_web_viewer_dir())
            .expect("Failed to load the built web viewer assets");
        let server = WebViewerServer::with_data("127.0.0.1", WebViewerServerPort::AUTO, data)
            .expect("Failed to start the web viewer server");

        let mut page_url = url::Url::parse(&server.server_url()).expect("valid base URL");
        {
            let mut query = page_url.query_pairs_mut();
            query.append_pair("integration_test", "");
            query.append_pair("hide_welcome_screen", "");
            if let Some(url) = &startup_url {
                query.append_pair("url", url);
            }
        }

        let browser = launch_browser(size);
        let tab = browser.new_tab().expect("Failed to open a browser tab");

        // Forward the browser's console and network logs for debugging
        install_browser_log_forwarding(&tab);

        let connection = Self {
            tab,
            _browser: browser,
            _server: server,
        };

        connection.set_viewport(size.x as u32, size.y as u32);

        connection
            .tab
            .navigate_to(page_url.as_str())
            .expect("Failed to navigate to the viewer page")
            .wait_until_navigated()
            .expect("The viewer page did not finish navigating");

        connection.wait_until_ready();
        connection
    }

    /// We need to set the size via `DeviceMetrics` since the regular size call sets the outer size.
    pub(super) fn set_viewport(&self, width: u32, height: u32) {
        use headless_chrome::protocol::cdp::Emulation;

        self.tab
            .call_method(Emulation::SetDeviceMetricsOverride {
                width,
                height,
                device_scale_factor: 1.0,
                mobile: false,
                scale: None,
                screen_width: None,
                screen_height: None,
                position_x: None,
                position_y: None,
                dont_set_visible_size: None,
                screen_orientation: None,
                viewport: None,
                display_feature: None,
                device_posture: None,
            })
            .expect("Failed to set the browser viewport size");
    }

    /// Wait until the viewer has booted and exposed `window._handle` (set in `on_app_started`), so
    /// the first `inspect` eval doesn't hit an undefined handle.
    fn wait_until_ready(&self) {
        let deadline = Instant::now() + CONNECT_TIMEOUT;
        loop {
            let ready = self
                .tab
                .evaluate("typeof window._handle !== 'undefined'", false)
                .ok()
                .and_then(|object| object.value)
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            if ready {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "The web viewer did not finish booting within {CONNECT_TIMEOUT:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Send one request and return its response, panicking on transport failure or a
    /// [`Response::Error`] reply.
    pub(super) fn request(&self, request: &Request) -> Response {
        use base64::Engine as _;

        let encoded = protocol::encode_body(&request).expect("Failed to encode inspection request");
        let request_base64 = base64::engine::general_purpose::STANDARD.encode(encoded);

        // `window._handle.inspect` is an `async` wasm method (returns a Promise); `await_promise`
        // (the second `evaluate` argument) resolves it to the base64 response string.
        // Decode the base64 request to raw bytes using Uint8Array.fromBase64, call inspect,
        // and encode the response back to base64 using toBase64.
        let script = format!(
            r#"(async () => {{
                const bytes = Uint8Array.fromBase64("{request_base64}");
                const response = await window._handle.inspect(bytes);
                return response.toBase64();
            }})()"#
        );
        let response_base64 = self.evaluate_js(&script);

        let response = base64::engine::general_purpose::STANDARD
            .decode(response_base64)
            .expect("Failed to decode base64 inspection response");
        match protocol::decode_body(&response).expect("Failed to decode inspection response") {
            Response::Error { message } => panic!("Viewer returned an error: {message}"),
            response => response,
        }
    }

    /// Evaluate an async `JavaScript` expression in the browser and return its string result.
    pub(super) fn evaluate_js(&self, script: &str) -> String {
        self.tab
            .evaluate(script, true)
            .unwrap_or_else(|err| panic!("Browser evaluation failed: {err}"))
            .value
            .and_then(|value| value.as_str().map(str::to_owned))
            .expect("Browser evaluation did not return a string")
    }

    /// Capture the current frame as PNG bytes via the browser's native screenshot (the composited
    /// canvas), rather than routing a large PNG through the eval boundary.
    pub(super) fn screenshot(&self) -> Vec<u8> {
        use headless_chrome::protocol::cdp::Page;

        self.tab
            .capture_screenshot(Page::CaptureScreenshotFormatOption::Png, None, None, true)
            .expect("Failed to capture a browser screenshot")
    }
}

/// Launch Chrome (headless by default; windowed when [`TestEnv::windowed`] is set) at the given
/// size.
///
/// `headless_chrome` gives each launch its own temporary user-data directory, so many browsers can
/// run in parallel without colliding on a shared profile's `SingletonLock`.
fn launch_browser(size: egui::Vec2) -> headless_chrome::Browser {
    use headless_chrome::{Browser, LaunchOptionsBuilder};

    let (width, height) = (size.x as u32, size.y as u32);
    let options = LaunchOptionsBuilder::default()
        .headless(!TestEnv::get().windowed)
        .window_size(Some((width, height)))
        // Disable Chrome's setuid sandbox: CI runners (and most containers) don't grant the
        // privileges it needs, so with it enabled Chrome never starts and `Browser::new` hangs
        // waiting for the DevTools endpoint. Safe here — this only ever loads our local test viewer.
        .sandbox(false)
        .args(vec![
            // Let wgpu get a (software) WebGL/WebGPU context in headless Chrome,
            std::ffi::OsStr::new("--enable-unsafe-swiftshader"),
            std::ffi::OsStr::new("--use-angle=swiftshader"),
            // Force scale factor of 1 so snapshots match
            std::ffi::OsStr::new("--force-device-scale-factor=1"),
        ])
        .build()
        .expect("Failed to build the browser launch options");

    Browser::new(options).expect("Failed to launch Chrome — is it installed?")
}

/// Forward the browser's console messages and log entries (failed fetches, CORS, exceptions) to the
/// test's stderr.
fn install_browser_log_forwarding(tab: &headless_chrome::Tab) {
    use headless_chrome::protocol::cdp::{Log, Runtime, types::Event};

    // Enable the Log + Runtime domains so browser-level entries and the viewer's own `console.*`
    // output are delivered as events.
    tab.enable_log().ok();
    tab.enable_runtime().ok();
    tab.add_event_listener(Arc::new(move |event: &Event| match event {
        Event::LogEntryAdded(entry) => {
            let entry = &entry.params.entry;
            match entry.level {
                Log::LogEntryLevel::Verbose | Log::LogEntryLevel::Info => {}
                Log::LogEntryLevel::Warning => {
                    re_log::warn!("[browser warning] {}", entry.text);
                }
                Log::LogEntryLevel::Error => {
                    re_log::error!("[browser error] {}", entry.text);
                }
            }
        }

        Event::RuntimeConsoleAPICalled(call) => {
            let call = &call.params;
            let level = match call.Type {
                Runtime::ConsoleAPICalledEventTypeOption::Error => "error",
                Runtime::ConsoleAPICalledEventTypeOption::Warning => "warning",
                _ => "log",
            };
            if matches!(
                call.Type,
                Runtime::ConsoleAPICalledEventTypeOption::Error
                    | Runtime::ConsoleAPICalledEventTypeOption::Warning
            ) {
                let args = call
                    .args
                    .iter()
                    .map(format_remote_object)
                    .collect::<Vec<_>>()
                    .join(" ");
                eprintln!("[browser {level}] {args}");
            }
        }

        Event::RuntimeExceptionThrown(exception) => {
            let details = &exception.params.exception_details;
            let message = details
                .exception
                .as_ref()
                .map(format_remote_object)
                .unwrap_or_default();
            panic!("browser exception: {} {message}", details.text);
        }

        _ => {}
    }))
    .ok();
}

/// Render a single `console.*` argument as the string a developer would see in the browser console.
fn format_remote_object(object: &headless_chrome::protocol::cdp::Runtime::RemoteObject) -> String {
    match object.value.as_ref() {
        // Strings come through as JSON, which would print them with their quotes.
        Some(value) => value
            .as_str()
            .map_or_else(|| value.to_string(), ToOwned::to_owned),
        None => object.description.clone().unwrap_or_default(),
    }
}
