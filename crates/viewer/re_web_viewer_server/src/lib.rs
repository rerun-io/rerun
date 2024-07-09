//! Serves the web viewer wasm/html.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#![forbid(unsafe_code)]
#![warn(clippy::all, rust_2018_idioms)]

use std::{
    fmt::Display,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};

pub const DEFAULT_WEB_VIEWER_SERVER_PORT: u16 = 9090;

// See `Cargo.toml` docs for the `__ci` feature for more information about the `disable_web_viewer_server` cfg:
#[cfg(not(any(disable_web_viewer_server, feature = "__ci")))]
mod data {
    #![allow(clippy::large_include_file)]

    // If you add/remove/change the paths here, also update the include-list in `Cargo.toml`!
    pub const INDEX_HTML: &[u8] = include_bytes!("../../../../web_viewer/index.html");
    pub const FAVICON: &[u8] = include_bytes!("../../../../web_viewer/favicon.svg");
    pub const SW_JS: &[u8] = include_bytes!("../../../../web_viewer/sw.js");
    pub const VIEWER_JS: &[u8] = include_bytes!("../../../../web_viewer/re_viewer.js");
    pub const VIEWER_WASM: &[u8] = include_bytes!("../../../../web_viewer/re_viewer_bg.wasm");
}

/// Failure to host the web viewer.
#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum WebViewerServerError {
    #[error("Could not parse address: {0}")]
    AddrParseFailed(#[from] std::net::AddrParseError),

    #[error("Failed to create server at address {0}: {1}")]
    CreateServerFailed(String, Box<dyn std::error::Error + Send + Sync + 'static>),
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Typed port for use with [`WebViewerServer`]
pub struct WebViewerServerPort(pub u16);

impl WebViewerServerPort {
    /// Port to use with [`WebViewerServer::new`] when you want the OS to pick a port for you.
    ///
    /// This is defined as `0`.
    pub const AUTO: Self = Self(0);
}

impl Default for WebViewerServerPort {
    fn default() -> Self {
        Self(DEFAULT_WEB_VIEWER_SERVER_PORT)
    }
}

impl Display for WebViewerServerPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Needed for clap
impl FromStr for WebViewerServerPort {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<u16>() {
            Ok(port) => Ok(Self(port)),
            Err(err) => Err(format!("Failed to parse port: {err}")),
        }
    }
}

/// HTTP host for the Rerun Web Viewer application
/// This serves the HTTP+Wasm+JS files that make up the web-viewer.
pub struct WebViewerServer {
    inner: Arc<WebViewerServerInner>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

struct WebViewerServerInner {
    server: tiny_http::Server,
    shutdown: AtomicBool,
    num_wasm_served: AtomicU64,

    // NOTE: Optional because it is possible to have the `analytics` feature flag enabled
    // while at the same time opting-out of analytics at run-time.
    #[cfg(feature = "analytics")]
    analytics: Option<re_analytics::Analytics>,
}

impl WebViewerServer {
    /// Create new [`WebViewerServer`] to host the Rerun Web Viewer on a specified port.
    ///
    /// [`WebViewerServerPort::AUTO`] will tell the OS choose any free port.
    ///
    /// The server will immediately start listening for incoming connections
    /// and stop doing so when the returned [`WebViewerServer`] is dropped.
    ///
    /// ## Example
    /// ``` no_run
    /// # use re_web_viewer_server::{WebViewerServer, WebViewerServerPort, WebViewerServerError};
    /// # async fn example() -> Result<(), WebViewerServerError> {
    /// let server = WebViewerServer::new("0.0.0.0", WebViewerServerPort::AUTO)?;
    /// let server_url = server.server_url();
    /// # Ok(()) }
    /// ```
    pub fn new(bind_ip: &str, port: WebViewerServerPort) -> Result<Self, WebViewerServerError> {
        let bind_addr: std::net::SocketAddr = format!("{bind_ip}:{port}").parse()?;

        let server = tiny_http::Server::http(bind_addr)
            .map_err(|err| WebViewerServerError::CreateServerFailed(bind_addr.to_string(), err))?;
        let shutdown = AtomicBool::new(false);

        let inner = Arc::new(WebViewerServerInner {
            server,
            shutdown,
            num_wasm_served: Default::default(),

            #[cfg(feature = "analytics")]
            analytics: match re_analytics::Analytics::new(std::time::Duration::from_secs(2)) {
                Ok(analytics) => Some(analytics),
                Err(err) => {
                    re_log::error!(%err, "failed to initialize analytics SDK");
                    None
                }
            },
        });

        let inner_copy = inner.clone();

        // TODO(andreas): Should we create a bunch of worker threads as proposed by https://docs.rs/tiny_http/latest/tiny_http/#creating-the-server ?
        // Not doing this right now since what we're serving out is so trivial (just a few files).
        let thread_handle = std::thread::Builder::new()
            .name("re_web_viewer_server".to_owned())
            .spawn(move || inner_copy.serve())
            .ok();

        Ok(Self {
            inner,
            thread_handle,
        })
    }

    /// Includes `http://` prefix
    pub fn server_url(&self) -> String {
        let local_addr = self.inner.server.server_addr();
        if let Some(local_addr) = local_addr.clone().to_ip() {
            if local_addr.ip().is_unspecified() {
                return format!("http://localhost:{}", local_addr.port());
            }
        }
        format!("http://{local_addr}")
    }

    /// Blocks execution as long as the server is running.
    ///
    /// There's no way of shutting the server down from the outside right now.
    pub fn block(mut self) {
        if let Some(thread_handle) = self.thread_handle.take() {
            thread_handle.join().ok();
        }
    }
}

impl Drop for WebViewerServer {
    fn drop(&mut self) {
        if let Some(thread_handle) = self.thread_handle.take() {
            let num_wasm_served = self.inner.num_wasm_served.load(Ordering::Relaxed);
            re_log::debug!(
                "Shutting down web server after serving the Wasm {num_wasm_served} time(s)"
            );

            self.inner.shutdown.store(true, Ordering::Release);
            self.inner.server.unblock();
            thread_handle.join().ok();
        }
    }
}

impl WebViewerServerInner {
    fn serve(&self) {
        loop {
            let request = self.server.recv();
            if self.shutdown.load(Ordering::Acquire) {
                return;
            }

            let request = match request {
                Ok(request) => request,
                Err(err) => {
                    re_log::error!("Failed to receive http request: {err}");
                    continue;
                }
            };

            if let Err(err) = self.send_response(request) {
                re_log::error!("Failed to send http response: {err}");
            }
        }
    }

    fn on_serve_wasm(&self) {
        self.num_wasm_served.fetch_add(1, Ordering::Relaxed);

        #[cfg(feature = "analytics")]
        if let Some(analytics) = &self.analytics {
            analytics.record(re_analytics::event::ServeWasm);
        }
    }

    #[cfg(feature = "__ci")]
    #[allow(clippy::needless_pass_by_value)]
    fn send_response(&self, _request: tiny_http::Request) -> Result<(), std::io::Error> {
        if false {
            self.on_serve_wasm(); // to silence warning about the function being unused
        }
        panic!("web_server compiled with '__ci' feature (or `--all-features`). DON'T DO THAT! It's only for the CI!");
    }

    #[cfg(not(feature = "__ci"))]
    fn send_response(&self, request: tiny_http::Request) -> Result<(), std::io::Error> {
        // Strip arguments from url so we get the actual path.
        let url = request.url();
        let path = url.split('?').next().unwrap_or(url);

        let (mime, bytes) = match path {
            "/" | "/index.html" => ("text/html", data::INDEX_HTML),
            "/favicon.svg" => ("image/svg+xml", data::FAVICON),
            "/favicon.ico" => ("image/x-icon", data::FAVICON),
            "/sw.js" => ("text/javascript", data::SW_JS),
            "/re_viewer.js" => ("text/javascript", data::VIEWER_JS),
            "/re_viewer_bg.wasm" => {
                self.on_serve_wasm();
                ("application/wasm", data::VIEWER_WASM)
            }
            _ => {
                re_log::warn!("404 path: {}", path);
                return request.respond(tiny_http::Response::empty(404));
            }
        };

        // TODO(#6061): Wasm should be compressed.

        let mut response = tiny_http::Response::from_data(bytes).with_header(
            tiny_http::Header::from_str(&format!("Content-Type: {mime}"))
                // Both `mime` and the header are hardcoded, so shouldn't be able to fail depending on user input.
                .expect("Invalid http header"),
        );

        // The wasm files are pretty large, so they'll be sent chunked (ideally we'd gzip them...).
        // (tiny_http will do so automatically if the data is above a certain threshold.
        // It is configurable, but we don't know all the implications of that.)
        // Unfortunately `Transfer-Encoding: chunked` means that no size is transmitted.
        // We work around this by adding a custom header with the size that web_viewer/index.html understands.
        if let Ok(header) =
            tiny_http::Header::from_str(&format!("rerun-final-length: {}", bytes.len()))
        {
            response.add_header(header);
        }

        request.respond(response)
    }
}
