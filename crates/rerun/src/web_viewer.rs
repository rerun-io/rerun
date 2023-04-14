use re_log_types::LogMsg;
use re_web_viewer_server::WebViewerServerHandle;
use re_ws_comms::RerunServerHandle;

/// A [`crate::sink::LogSink`] tied to a hosted Rerun web viewer. This internally stores two servers:
/// * A [`RerunServer`] to relay messages from the sink to a websocket connection
/// * A [`WebViewerServer`] to serve the Wasm+HTML
struct WebViewerSink {
    /// Sender to send messages to the [`re_ws_comms::RerunServer`]
    sender: re_smart_channel::Sender<LogMsg>,

    /// Handle to keep the [`re_ws_comms::RerunServer`] alive
    _rerun_server: RerunServerHandle,

    /// Handle to keep the [`re_web_viewer_server::WebViewerServer`] alive
    _webviewer_server: WebViewerServerHandle,
}

impl WebViewerSink {
    pub fn new(open_browser: bool, web_port: u16, ws_port: u16) -> anyhow::Result<Self> {
        let (rerun_tx, rerun_rx) = re_smart_channel::smart_channel(re_smart_channel::Source::Sdk);

        let rerun_server = RerunServerHandle::new(rerun_rx, ws_port)?;
        let webviewer_server = WebViewerServerHandle::new(web_port)?;

        let web_port = webviewer_server.port();
        let server_url = rerun_server.server_url();
        let viewer_url = format!("http://127.0.0.1:{web_port}?url={server_url}");

        re_log::info!("Web server is running - view it at {viewer_url}");
        if open_browser {
            webbrowser::open(&viewer_url).ok();
        }

        Ok(Self {
            sender: rerun_tx,
            _rerun_server: rerun_server,
            _webviewer_server: webviewer_server,
        })
    }
}

/// Async helper to spawn an instance of the [`re_web_viewer_server::WebViewerServer`].
/// This serves the HTTP+Wasm+JS files that make up the web-viewer.
///
/// Optionally opens a browser with the web-viewer and connects to the provided `target_url`.
/// This url could be a hosted RRD file or a `ws://` url to a running [`re_ws_comms::RerunServer`].
///
/// Note: this does not include the websocket server.
#[cfg(feature = "web_viewer")]
pub async fn host_web_viewer(
    web_port: u16,
    open_browser: bool,
    source_url: String,
    shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let web_server = re_web_viewer_server::WebViewerServer::new(web_port)?;
    let port = web_server.port();
    let web_server_handle = web_server.serve(shutdown_rx);

    let viewer_url = format!("http://127.0.0.1:{port}?url={source_url}");

    re_log::info!("Web server is running - view it at {viewer_url}");
    if open_browser {
        webbrowser::open(&viewer_url).ok();
    }

    web_server_handle.await.map_err(anyhow::Error::msg)
}

impl crate::sink::LogSink for WebViewerSink {
    fn send(&self, msg: LogMsg) {
        if let Err(err) = self.sender.send(msg) {
            re_log::error_once!("Failed to send log message to web server: {err}");
        }
    }
}

// ----------------------------------------------------------------------------

/// Serve log-data over WebSockets and serve a Rerun web viewer over HTTP.
///
/// If the `open_browser` argument is `true`, your default browser
/// will be opened with a connected web-viewer.
///
/// If not, you can connect to this server using the `rerun` binary (`cargo install rerun`).
///
/// NOTE: you can not connect one `Session` to another.
///
/// This function returns immediately.
///
/// The caller needs to ensure that there is a `tokio` runtime running.
#[must_use = "the sink must be kept around to keep the servers running"]
pub fn new_sink(
    open_browser: bool,
    web_port: u16,
    ws_port: u16,
) -> anyhow::Result<Box<dyn crate::sink::LogSink>> {
    Ok(Box::new(WebViewerSink::new(
        open_browser,
        web_port,
        ws_port,
    )?))
}
