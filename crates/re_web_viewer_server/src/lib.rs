//! Serves the web viewer wasm/html.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#![forbid(unsafe_code)]
#![warn(clippy::all, rust_2018_idioms)]

use std::task::{Context, Poll};

use futures_util::future;
use hyper::{server::conn::AddrIncoming, service::Service, Body, Request, Response};

pub const DEFAULT_WEB_VIEWER_PORT: u16 = 9090;

#[cfg(not(feature = "__ci"))]
mod data {
    // If you add/remove/change the paths here, also update the include-list in `Cargo.toml`!
    pub const INDEX_HTML: &[u8] = include_bytes!("../web_viewer/index_bundled.html");
    pub const FAVICON: &[u8] = include_bytes!("../web_viewer/favicon.svg");
    pub const SW_JS: &[u8] = include_bytes!("../web_viewer/sw.js");

    #[cfg(debug_assertions)]
    pub const VIEWER_JS_DEBUG: &[u8] = include_bytes!("../web_viewer/re_viewer_debug.js");

    #[cfg(debug_assertions)]
    pub const VIEWER_WASM_DEBUG: &[u8] = include_bytes!("../web_viewer/re_viewer_debug_bg.wasm");

    #[cfg(not(debug_assertions))]
    pub const VIEWER_JS_RELEASE: &[u8] = include_bytes!("../web_viewer/re_viewer.js");

    #[cfg(not(debug_assertions))]
    pub const VIEWER_WASM_RELEASE: &[u8] = include_bytes!("../web_viewer/re_viewer_bg.wasm");
}

#[derive(thiserror::Error, Debug)]
pub enum WebViewerServerError {
    #[error("Could not parse address: {0}")]
    AddrParseFailed(#[from] std::net::AddrParseError),

    #[error("failed to bind to port {0}: {1}")]
    BindFailed(u16, hyper::Error),

    #[error("failed to join web viewer server task: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("failed to serve web viewer: {0}")]
    ServeFailed(hyper::Error),
}

struct Svc {
    // NOTE: Optional because it is possible to have the `analytics` feature flag enabled
    // while at the same time opting-out of analytics at run-time.
    #[cfg(feature = "analytics")]
    analytics: Option<re_analytics::Analytics>,
}

impl Svc {
    #[cfg(feature = "analytics")]
    fn new() -> Self {
        let analytics = match re_analytics::Analytics::new(std::time::Duration::from_secs(2)) {
            Ok(analytics) => Some(analytics),
            Err(err) => {
                re_log::error!(%err, "failed to initialize analytics SDK");
                None
            }
        };
        Self { analytics }
    }

    #[cfg(not(feature = "analytics"))]
    fn new() -> Self {
        Self {}
    }

    #[cfg(feature = "analytics")]
    fn on_serve_wasm(&self) {
        if let Some(analytics) = &self.analytics {
            analytics.record(re_analytics::Event::append("serve_wasm"));
        }
    }
}

impl Service<Request<Body>> for Svc {
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    #[cfg(feature = "__ci")]
    fn call(&mut self, _req: Request<Body>) -> Self::Future {
        if false {
            self.on_serve_wasm(); // to silence warning about the function being unused
        }

        // panic! is not enough in hyper (since it uses catch_unwind) - that only kills this thread. We want to quit.
        eprintln!("web_server compiled with '__ci' feature (or `--all-features`). DON'T DO THAT! It's only for the CI!");
        std::process::abort();
    }

    #[cfg(not(feature = "__ci"))]
    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let response = Response::builder();

        let (mime, bytes) = match req.uri().path() {
            "/" | "/index.html" => ("text/html", data::INDEX_HTML),
            "/favicon.svg" => ("image/svg+xml", data::FAVICON),
            "/sw.js" => ("text/javascript", data::SW_JS),

            #[cfg(debug_assertions)]
            "/re_viewer.js" => ("text/javascript", data::VIEWER_JS_DEBUG),
            #[cfg(not(debug_assertions))]
            "/re_viewer.js" => ("text/javascript", data::VIEWER_JS_RELEASE),

            "/re_viewer_bg.wasm" => {
                #[cfg(feature = "analytics")]
                self.on_serve_wasm();

                #[cfg(debug_assertions)]
                {
                    re_log::info_once!("Serving DEBUG web-viewer");
                    ("application/wasm", data::VIEWER_WASM_DEBUG)
                }
                #[cfg(not(debug_assertions))]
                {
                    ("application/wasm", data::VIEWER_WASM_RELEASE)
                }
            }
            _ => {
                re_log::warn!("404 path: {}", req.uri().path());
                let body = Body::from(Vec::new());
                let rsp = response.status(404).body(body).unwrap();
                return future::ok(rsp);
            }
        };

        let body = Body::from(Vec::from(bytes));
        let mut response = response.status(200).body(body).unwrap();
        response.headers_mut().insert(
            hyper::header::CONTENT_TYPE,
            hyper::header::HeaderValue::from_static(mime),
        );
        future::ok(response)
    }
}

struct MakeSvc;

impl<T> Service<T> for MakeSvc {
    type Response = Svc;
    type Error = std::io::Error;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, _: T) -> Self::Future {
        future::ok(Svc::new())
    }
}

// ----------------------------------------------------------------------------

/// HTTP host for the Rerun Web Viewer application
/// This serves the HTTP+Wasm+JS files that make up the web-viewer.
pub struct WebViewerServer {
    server: hyper::Server<AddrIncoming, MakeSvc>,
}

impl WebViewerServer {
    pub fn new(port: u16) -> Result<Self, WebViewerServerError> {
        let bind_addr = format!("0.0.0.0:{port}").parse()?;
        let server = hyper::Server::try_bind(&bind_addr)
            .map_err(|e| WebViewerServerError::BindFailed(port, e))?
            .serve(MakeSvc);
        Ok(Self { server })
    }

    pub async fn serve(
        self,
        mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<(), WebViewerServerError> {
        self.server
            .with_graceful_shutdown(async {
                shutdown_rx.recv().await.ok();
            })
            .await
            .map_err(WebViewerServerError::ServeFailed)?;
        Ok(())
    }

    pub fn port(&self) -> u16 {
        self.server.local_addr().port()
    }
}

/// Sync handle for the [`WebViewerServer`]
///
/// When dropped, the server will be shut down.
pub struct WebViewerServerHandle {
    port: u16,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

impl Drop for WebViewerServerHandle {
    fn drop(&mut self) {
        re_log::info!("Shutting down web server on port {}.", self.port);
        self.shutdown_tx.send(()).ok();
    }
}

impl WebViewerServerHandle {
    /// Create new [`WebViewerServer`] to host the Rerun Web Viewer on a specified port.
    ///
    /// A port of 0 will let the OS choose a free port.
    ///
    /// The caller needs to ensure that there is a `tokio` runtime running.
    pub fn new(requested_port: u16) -> Result<Self, WebViewerServerError> {
        let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

        let web_server = WebViewerServer::new(requested_port)?;

        let port = web_server.server.local_addr().port();

        tokio::spawn(async move { web_server.serve(shutdown_rx).await });

        re_log::info!("Started web server on port {}.", port);

        Ok(Self { port, shutdown_tx })
    }

    /// Get the port where the web assets are hosted
    pub fn port(&self) -> u16 {
        self.port
    }
}
