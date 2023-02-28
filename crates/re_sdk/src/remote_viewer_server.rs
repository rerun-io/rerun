use re_log_types::LogMsg;

/// Hosts two servers:
/// * A web-server, serving the web-viewer
/// * A `WebSocket` server, server [`LogMsg`]es to remote viewer(s).
pub struct RemoteViewerServer {
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

    pub fn send(&self, msg: LogMsg) {
        if let Err(err) = self.sender.send(msg) {
            re_log::error_once!("Failed to send log message to web server: {err}");
        }
    }
}
