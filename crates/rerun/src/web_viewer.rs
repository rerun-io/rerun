use re_log_types::LogMsg;
use re_web_viewer_server::{WebViewerServerHandle, WebViewerServerPort};
use re_ws_comms::{RerunServerHandle, RerunServerPort};

/// A [`crate::sink::LogSink`] tied to a hosted Rerun web viewer. This internally stores two servers:
/// * A [`re_ws_comms::RerunServer`] to relay messages from the sink to a websocket connection
/// * A [`re_web_viewer_server::WebViewerServer`] to serve the Wasm+HTML
struct WebViewerSink {
    /// Sender to send messages to the [`re_ws_comms::RerunServer`]
    sender: re_smart_channel::Sender<LogMsg>,

    /// Handle to keep the [`re_ws_comms::RerunServer`] alive
    _rerun_server: RerunServerHandle,

    /// Handle to keep the [`re_web_viewer_server::WebViewerServer`] alive
    _webviewer_server: WebViewerServerHandle,
}

impl WebViewerSink {
    /// A `bind_ip` of `"0.0.0.0"` is a good default.
    pub fn new(
        open_browser: bool,
        bind_ip: &str,
        web_port: WebViewerServerPort,
        ws_port: RerunServerPort,
    ) -> anyhow::Result<Self> {
        // TODO(cmc): the sources here probably don't make much sense...
        let (rerun_tx, rerun_rx) = re_smart_channel::smart_channel(
            re_smart_channel::SmartMessageSource::Sdk,
            re_smart_channel::SmartChannelSource::Sdk,
        );

        let rerun_server = RerunServerHandle::new(rerun_rx, bind_ip.to_owned(), ws_port)?;
        let webviewer_server = WebViewerServerHandle::new(bind_ip, web_port)?;

        let http_web_viewer_url = webviewer_server.server_url();
        let ws_server_url = rerun_server.server_url();
        let viewer_url = format!("{http_web_viewer_url}?url={ws_server_url}");

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
    bind_ip: String,
    web_port: WebViewerServerPort,
    open_browser: bool,
    source_url: String,
) -> anyhow::Result<()> {
    let web_server = re_web_viewer_server::WebViewerServer::new(&bind_ip, web_port)?;
    let http_web_viewer_url = web_server.server_url();
    let web_server_handle = web_server.serve();

    let viewer_url = format!("{http_web_viewer_url}?url={source_url}");

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

    #[inline]
    fn flush_blocking(&self) {}
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
///
/// The caller needs to ensure that there is a `tokio` runtime running.
#[must_use = "the sink must be kept around to keep the servers running"]
pub fn new_sink(
    open_browser: bool,
    bind_ip: &str,
    web_port: WebViewerServerPort,
    ws_port: RerunServerPort,
) -> anyhow::Result<Box<dyn crate::sink::LogSink>> {
    Ok(Box::new(WebViewerSink::new(
        open_browser,
        bind_ip,
        web_port,
        ws_port,
    )?))
}
