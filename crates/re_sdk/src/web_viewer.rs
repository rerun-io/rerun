use re_log_types::LogMsg;
use re_web_viewer_server::{WebViewerServer, WebViewerServerError, WebViewerServerPort};
use re_ws_comms::{RerunServer, RerunServerPort};

// ----------------------------------------------------------------------------

/// Failure to host a web viewer and/or Rerun server.
#[derive(thiserror::Error, Debug)]
pub enum WebViewerSinkError {
    /// Failure to host the web viewer.
    #[error(transparent)]
    WebViewerServer(#[from] WebViewerServerError),

    /// Failure to host the Rerun WebSocket server.
    #[error(transparent)]
    RerunServer(#[from] re_ws_comms::RerunServerError),
}

/// A [`crate::sink::LogSink`] tied to a hosted Rerun web viewer. This internally stores two servers:
/// * A [`re_ws_comms::RerunServer`] to relay messages from the sink to a websocket connection
/// * A [`WebViewerServer`] to serve the Wasm+HTML
struct WebViewerSink {
    open_browser: bool,

    /// Sender to send messages to the [`re_ws_comms::RerunServer`]
    sender: re_smart_channel::Sender<LogMsg>,

    /// Rerun websocket server.
    rerun_server: RerunServer,

    /// The http server serving wasm & html.
    _webviewer_server: WebViewerServer,
}

impl WebViewerSink {
    /// A `bind_ip` of `"0.0.0.0"` is a good default.
    pub fn new(
        open_browser: bool,
        bind_ip: &str,
        web_port: WebViewerServerPort,
        ws_port: RerunServerPort,
        server_memory_limit: re_memory::MemoryLimit,
    ) -> Result<Self, WebViewerSinkError> {
        // TODO(cmc): the sources here probably don't make much sense…
        let (rerun_tx, rerun_rx) = re_smart_channel::smart_channel(
            re_smart_channel::SmartMessageSource::Sdk,
            re_smart_channel::SmartChannelSource::Sdk,
        );

        let rerun_server = RerunServer::new(
            re_smart_channel::ReceiveSet::new(vec![rerun_rx]),
            bind_ip,
            ws_port,
            server_memory_limit,
        )?;
        let webviewer_server = WebViewerServer::new(bind_ip, web_port)?;

        let http_web_viewer_url = webviewer_server.server_url();
        let ws_server_url = rerun_server.server_url();
        let viewer_url = format!("{http_web_viewer_url}?url={ws_server_url}");

        re_log::info!("Hosting a web-viewer at {viewer_url}");
        if open_browser {
            webbrowser::open(&viewer_url).ok();
        }

        Ok(Self {
            open_browser,
            sender: rerun_tx,
            rerun_server,
            _webviewer_server: webviewer_server,
        })
    }
}

impl crate::sink::LogSink for WebViewerSink {
    fn send(&self, msg: LogMsg) {
        if let Err(err) = self.sender.send(msg) {
            re_log::error_once!("Failed to send log message to web server: {err}");
        }
    }

    #[inline]
    fn flush_blocking(&self) {
        if let Err(err) = self.sender.flush_blocking() {
            re_log::error_once!("Failed to flush: {err}");
        }
    }
}

impl Drop for WebViewerSink {
    fn drop(&mut self) {
        if self.open_browser && self.rerun_server.num_accepted_clients() == 0 {
            // For small scripts that execute fast we run the risk of finishing
            // before the browser has a chance to connect.
            // Let's give it a little more time:
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }

        if self.rerun_server.num_accepted_clients() == 0 {
            re_log::info!("Shutting down without any clients ever having connected. Consider sleeping to give them more time to connect");
        }
    }
}

// ----------------------------------------------------------------------------

/// Helper to spawn an instance of the [`WebViewerServer`].
/// This serves the HTTP+Wasm+JS files that make up the web-viewer.
///
/// Optionally opens a browser with the web-viewer and connects to the provided `target_url`.
/// This url could be a hosted RRD file or a `ws://` url to a running [`re_ws_comms::RerunServer`].
///
/// Note: this does not include the websocket server.
#[cfg(feature = "web_viewer")]
pub fn host_web_viewer(
    bind_ip: &str,
    web_port: WebViewerServerPort,
    force_wgpu_backend: Option<String>,
    open_browser: bool,
    source_url: &str,
) -> anyhow::Result<WebViewerServer> {
    let web_server = WebViewerServer::new(bind_ip, web_port)?;
    let http_web_viewer_url = web_server.server_url();

    let mut viewer_url = format!("{http_web_viewer_url}?url={source_url}");
    if let Some(force_graphics) = force_wgpu_backend {
        viewer_url = format!("{viewer_url}&renderer={force_graphics}");
    }

    re_log::info!("Hosting a web-viewer at {viewer_url}");
    if open_browser {
        webbrowser::open(&viewer_url).ok();
    }

    Ok(web_server)
}

// ----------------------------------------------------------------------------

/// Serve log-data over WebSockets and serve a Rerun web viewer over HTTP.
///
/// If the `open_browser` argument is `true`, your default browser
/// will be opened with a connected web-viewer.
///
/// If not, you can connect to this server using the `rerun` binary (`cargo install rerun-cli`).
///
/// NOTE: you can not connect one `Session` to another.
///
/// This function returns immediately.
#[must_use = "the sink must be kept around to keep the servers running"]
pub fn new_sink(
    open_browser: bool,
    bind_ip: &str,
    web_port: WebViewerServerPort,
    ws_port: RerunServerPort,
    server_memory_limit: re_memory::MemoryLimit,
) -> Result<Box<dyn crate::sink::LogSink>, WebViewerSinkError> {
    Ok(Box::new(WebViewerSink::new(
        open_browser,
        bind_ip,
        web_port,
        ws_port,
        server_memory_limit,
    )?))
}
