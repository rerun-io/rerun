use crate::{server_url, WebViewerServer, WebViewerServerError, WebViewerServerPort};

/// Sync handle for the [`WebViewerServer`]
///
/// When dropped, the server will be shut down.
pub struct WebViewerServerHandle {
    local_addr: std::net::SocketAddr,
    shutdown: Option<(
        futures_channel::oneshot::Sender<()>,
        std::thread::JoinHandle<()>,
    )>,
}

impl Drop for WebViewerServerHandle {
    fn drop(&mut self) {
        re_log::info!("Shutting down web server on {}", self.server_url());

        if let Some((shutdown_tx, thread_handle)) = self.shutdown.take() {
            if shutdown_tx.send(()).is_err() {
                re_log::error!("Failed to send shutdown signal to web server thread.");
            }
            thread_handle.join().ok();
        }
    }
}

impl WebViewerServerHandle {
    /// Create new [`WebViewerServer`] to host the Rerun Web Viewer on a specified port.
    /// Returns a [`WebViewerServerHandle`] that will shutdown the server when dropped.
    ///
    /// A port of 0 will let the OS choose a free port.
    ///
    /// Internally spawns a thread to run the server.
    /// If you instead want to use the server in your async runtime (e.g. [`tokio`](https://docs.rs/tokio/latest/tokio/) or [`smol`](https://docs.rs/smol/latest/smol/)), use [`WebViewerServer`].
    pub fn new(
        bind_ip: &str,
        requested_port: WebViewerServerPort,
    ) -> Result<Self, WebViewerServerError> {
        let (shutdown_tx, shutdown_rx) = futures_channel::oneshot::channel();

        let web_server = WebViewerServer::new(bind_ip, requested_port)?;

        let local_addr = web_server.server.local_addr();

        let server_serve_future = async {
            if let Err(err) = web_server.serve_with_graceful_shutdown(shutdown_rx).await {
                re_log::error!("Web server failed: {err}");
            }
        };

        let thread_handle = std::thread::Builder::new()
            .name("WebViewerServerHandle".to_owned())
            .spawn(move || pollster::block_on(server_serve_future))
            .map_err(WebViewerServerError::ThreadSpawnFailed)?;

        let slf = Self {
            local_addr,
            shutdown: Some((shutdown_tx, thread_handle)),
        };

        re_log::info!("Started web server on {}", slf.server_url());

        Ok(slf)
    }

    /// Includes `http://` prefix
    pub fn server_url(&self) -> String {
        server_url(&self.local_addr)
    }
}
