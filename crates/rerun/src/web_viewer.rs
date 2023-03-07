use re_log_types::LogMsg;

/// Hosts two servers:
/// * A web-server, serving the web-viewer
/// * A `WebSocket` server, server [`LogMsg`]es to remote viewer(s).
struct RemoteViewerServer {
    web_server_join_handle: tokio::task::JoinHandle<()>,
    sender: re_smart_channel::Sender<LogMsg>,
}

impl Drop for RemoteViewerServer {
    fn drop(&mut self) {
        re_log::info!("Shutting down web server.");
        self.web_server_join_handle.abort();
    }
}

impl RemoteViewerServer {
    pub fn new(tokio_rt: &tokio::runtime::Runtime, open_browser: bool) -> Self {
        let (rerun_tx, rerun_rx) = re_smart_channel::smart_channel(re_smart_channel::Source::Sdk);

        let web_server_join_handle = tokio_rt.spawn(async move {
            // This is the server which the web viewer will talk to:
            let ws_server = re_ws_comms::Server::new(re_ws_comms::DEFAULT_WS_SERVER_PORT)
                .await
                .unwrap();
            let ws_server_handle = tokio::spawn(ws_server.listen(rerun_rx)); // TODO(emilk): use tokio_rt ?

            // This is the server that serves the Wasm+HTML:
            let web_port = 9090;
            let web_server = re_web_viewer_server::WebViewerServer::new(web_port);
            let web_server_handle = tokio::spawn(async move {
                web_server.serve().await.unwrap();
            });

            let ws_server_url = re_ws_comms::default_server_url();
            let viewer_url = format!("http://127.0.0.1:{web_port}?url={ws_server_url}");
            if open_browser {
                webbrowser::open(&viewer_url).ok();
            } else {
                re_log::info!("Web server is running - view it at {viewer_url}");
            }

            ws_server_handle.await.unwrap().unwrap();
            web_server_handle.await.unwrap();
        });

        Self {
            web_server_join_handle,
            sender: rerun_tx,
        }
    }
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
pub fn serve(session: &mut crate::Session, open_browser: bool) {
    if !session.is_enabled() {
        re_log::debug!("Rerun disabled - call to serve() ignored");
        return;
    }

    session.set_sink(new_sink(open_browser));
}

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
#[must_use]
pub fn new_sink(open_browser: bool) -> Box<dyn crate::sink::LogSink> {
    // TODO(emilk): creating a tokio runtime on-demand like this is not great. Not sure how this interacts with `#[tokio::main]`, for instance.
    use once_cell::sync::Lazy;
    use parking_lot::Mutex;
    static TOKIO_RUNTIME: Lazy<Mutex<tokio::runtime::Runtime>> = Lazy::new(|| {
        Mutex::new(tokio::runtime::Runtime::new().expect("Failed to create tokio runtime"))
    });

    Box::new(RemoteViewerServer::new(&TOKIO_RUNTIME.lock(), open_browser))
}
