//! Serves the web viewer wasm/html.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#![forbid(unsafe_code)]
#![warn(clippy::all, rust_2018_idioms)]

use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub const DEFAULT_WEB_VIEWER_SERVER_PORT: u16 = 9090;

// See `Cargo.toml` for docs about the `disable_web_viewer_server` and `trailing_web_viewer` cfgs:
#[cfg(all(not(disable_web_viewer_server), not(trailing_web_viewer)))]
mod data {
    #![expect(clippy::large_include_file)]

    // If you add/remove/change the paths here, also update the include-list in `Cargo.toml`!
    #[inline]
    pub fn index_html() -> &'static [u8] {
        include_bytes!("../web_viewer/index.html")
    }

    #[inline]
    pub fn favicon() -> &'static [u8] {
        include_bytes!("../web_viewer/favicon.svg")
    }

    #[inline]
    pub fn sw_js() -> &'static [u8] {
        include_bytes!("../web_viewer/sw.js")
    }

    #[inline]
    pub fn viewer_js() -> &'static [u8] {
        include_bytes!("../web_viewer/re_viewer.js")
    }

    #[inline]
    pub fn viewer_wasm() -> &'static [u8] {
        include_bytes!("../web_viewer/re_viewer_bg.wasm")
    }

    #[inline]
    pub fn signed_in_html() -> &'static [u8] {
        include_bytes!("./signed-in.html")
    }
}

#[cfg(all(not(disable_web_viewer_server), trailing_web_viewer))]
mod trailing_data;

#[cfg(all(not(disable_web_viewer_server), trailing_web_viewer))]
use trailing_data as data;

/// Failure to host the web viewer.
#[derive(thiserror::Error, Debug)]
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
#[must_use = "Dropping this means stopping the server"]
pub struct WebViewerServer {
    inner: Arc<WebViewerServerInner>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

struct WebViewerServerInner {
    server: tiny_http::Server,
    shutdown: AtomicBool,
    num_wasm_served: AtomicU64,
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
        let bind_addr = std::net::SocketAddr::new(bind_ip.parse()?, port.0);

        let server = tiny_http::Server::http(bind_addr)
            .map_err(|err| WebViewerServerError::CreateServerFailed(bind_addr.to_string(), err))?;
        let shutdown = AtomicBool::new(false);

        let inner = Arc::new(WebViewerServerInner {
            server,
            shutdown,
            num_wasm_served: Default::default(),
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
        if let Some(local_addr) = local_addr.clone().to_ip()
            && local_addr.ip().is_unspecified()
        {
            return format!("http://127.0.0.1:{}", local_addr.port());
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

    /// Keeps the web viewer running until the parent process shuts down.
    pub fn detach(mut self) {
        if let Some(thread_handle) = self.thread_handle.take() {
            // dropping the thread handle detaches the thread.
            drop(thread_handle);
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
        re_analytics::record(|| re_analytics::event::ServeWasm);
    }

    #[cfg(disable_web_viewer_server)]
    fn send_response(&self, _request: tiny_http::Request) -> Result<(), std::io::Error> {
        if false {
            self.on_serve_wasm(); // to silence warning about the function being unused
        }
        panic!(
            "re_web_viewer_server compiled without .wasm, because of '__disable_server' feature, `--all-features`, or 'RERUN_DISABLE_WEB_VIEWER_SERVER=1'. DON'T DO THAT! It's only meant for tests and docs!"
        );
    }

    #[cfg(not(disable_web_viewer_server))]
    fn send_response(&self, request: tiny_http::Request) -> Result<(), std::io::Error> {
        // Strip arguments from url so we get the actual path.
        let url = request.url();
        let path = url.split('?').next().unwrap_or(url);

        let (mime, bytes): (&str, &[u8]) = match path {
            "/" | "/index.html" => ("text/html", data::index_html()),
            "/favicon.svg" => ("image/svg+xml", data::favicon()),
            "/favicon.ico" => ("image/x-icon", data::favicon()),
            "/sw.js" => ("text/javascript", data::sw_js()),
            "/re_viewer.js" => ("text/javascript", data::viewer_js()),
            "/re_viewer_bg.wasm" => {
                self.on_serve_wasm();
                ("application/wasm", data::viewer_wasm())
            }
            "/signed-in" => ("text/html", data::signed_in_html()),
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

        // The wasm files are pretty large, so they'll be sent chunked (ideally we'd gzip them…).
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
