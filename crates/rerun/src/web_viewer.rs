use re_log_types::LogMsg;

/// Hosts two servers:
/// * A web-server, serving the web-viewer
/// * A `WebSocket` server, server [`LogMsg`]es to remote viewer(s).
struct RemoteViewerServer {
    sender: re_smart_channel::Sender<LogMsg>,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

impl Drop for RemoteViewerServer {
    fn drop(&mut self) {
        re_log::info!("Shutting down web server.");
        self.shutdown_tx.send(()).ok();
    }
}

impl RemoteViewerServer {
    pub fn new(open_browser: bool) -> Self {
        let (rerun_tx, rerun_rx) = re_smart_channel::smart_channel(re_smart_channel::Source::Sdk);
        let (shutdown_tx, shutdown_rx_ws_server) = tokio::sync::broadcast::channel(1);
        let shutdown_rx_web_server = shutdown_tx.subscribe();

        tokio::spawn(async move {
            // This is the server which the web viewer will talk to:
            let ws_server = re_ws_comms::Server::new(re_ws_comms::DEFAULT_WS_SERVER_PORT)
                .await
                .unwrap();
            let ws_server_handle = tokio::spawn(ws_server.listen(rerun_rx, shutdown_rx_ws_server));
            let ws_server_url = re_ws_comms::default_server_url("127.0.0.1");

            // This is the server that serves the Wasm+HTML:
            let web_server_handle = tokio::spawn(host_web_viewer(
                open_browser,
                ws_server_url,
                shutdown_rx_web_server,
            ));

            ws_server_handle.await.unwrap().unwrap();
            web_server_handle.await.unwrap().unwrap();
        });

        Self {
            sender: rerun_tx,
            shutdown_tx,
        }
    }
}

/// Hosts two servers:
/// * A web-server, serving the web-viewer
/// * A `WebSocket` server, server [`LogMsg`]es to remote viewer(s).
///
/// Optionally opens a browser with the web-viewer.
#[cfg(feature = "web_viewer")]
pub async fn host_web_viewer(
    open_browser: bool,
    ws_server_url: String,
    shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let web_port = 9090;
    let viewer_url = format!("http://127.0.0.1:{web_port}?url={ws_server_url}");

    let web_server = re_web_viewer_server::WebViewerServer::new(web_port);
    let web_server_handle = tokio::spawn(web_server.serve(shutdown_rx));

    re_log::info!("Web server is running - view it at {viewer_url}");
    if open_browser {
        webbrowser::open(&viewer_url).ok();
    } else {
        re_log::info!("Web server is running - view it at {viewer_url}");
    }

    web_server_handle.await?
}

impl crate::sink::LogSink for RemoteViewerServer {
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
#[must_use]
pub fn new_sink(open_browser: bool) -> Box<dyn crate::sink::LogSink> {
    Box::new(RemoteViewerServer::new(open_browser))
}
