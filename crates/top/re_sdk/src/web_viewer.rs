use re_chunk::ChunkBatcherConfig;
use re_log_types::LogMsg;
use re_web_viewer_server::{WebViewerServer, WebViewerServerError, WebViewerServerPort};

use crate::log_sink::SinkFlushError;

// ----------------------------------------------------------------------------

/// Failure to host a web viewer and/or Rerun server.
#[derive(thiserror::Error, Debug)]
pub enum WebViewerSinkError {
    /// Failure to host the web viewer.
    #[error(transparent)]
    WebViewerServer(#[from] WebViewerServerError),

    /// Invalid host IP.
    #[error(transparent)]
    InvalidAddress(#[from] std::net::AddrParseError),
}

/// A [`crate::sink::LogSink`] tied to a hosted Rerun web viewer. This internally stores two servers:
/// * A gRPC server to relay messages from the sink to any connected web viewers
/// * A [`WebViewerServer`] to serve the Wasm+HTML
struct WebViewerSink {
    open_browser: bool,

    /// Sender to send messages to the gRPC server.
    sender: re_log_channel::LogSender,

    /// The gRPC server thread.
    _server_handle: std::thread::JoinHandle<()>,

    /// Signal used to gracefully shutdown the gRPC server.
    server_shutdown_signal: re_grpc_server::shutdown::Signal,

    /// The http server serving wasm & html.
    _webviewer_server: WebViewerServer,
}

impl WebViewerSink {
    /// A `bind_ip` of `"0.0.0.0"` is a good default.
    pub fn new(
        open_browser: bool,
        bind_ip: &str,
        web_port: WebViewerServerPort,
        grpc_port: u16,
        server_options: re_grpc_server::ServerOptions,
    ) -> Result<Self, WebViewerSinkError> {
        let (server_shutdown_signal, shutdown) = re_grpc_server::shutdown::shutdown();

        let grpc_server_addr = format!("{bind_ip}:{grpc_port}").parse()?;
        let (channel_tx, channel_rx) = re_log_channel::log_channel(re_log_channel::LogSource::Sdk);
        let server_handle = std::thread::Builder::new()
            .name("message_proxy_server".to_owned())
            .spawn(move || {
                let mut builder = tokio::runtime::Builder::new_current_thread();
                builder.enable_all();
                let rt = builder.build().expect("failed to build tokio runtime");

                rt.block_on(re_grpc_server::serve_from_channel(
                    grpc_server_addr,
                    server_options,
                    shutdown,
                    channel_rx,
                ));
            })
            .expect("failed to spawn thread for message proxy server");
        let webviewer_server = WebViewerServer::new(bind_ip, web_port)?;

        let http_web_viewer_url = webviewer_server.server_url();

        let viewer_url =
            if grpc_server_addr.ip().is_unspecified() || grpc_server_addr.ip().is_loopback() {
                format!("{http_web_viewer_url}?url=rerun%2Bhttp://localhost:{grpc_port}/proxy")
            } else {
                format!("{http_web_viewer_url}?url=rerun%2Bhttp://{grpc_server_addr}/proxy")
            };

        re_log::info!("Hosting a web-viewer at {viewer_url}");
        if open_browser {
            webbrowser::open(&viewer_url).ok();
        }

        Ok(Self {
            open_browser,
            sender: channel_tx,
            _server_handle: server_handle,
            server_shutdown_signal,
            _webviewer_server: webviewer_server,
        })
    }
}

impl crate::sink::LogSink for WebViewerSink {
    fn send(&self, msg: LogMsg) {
        if let Err(err) = self.sender.send(msg.into()) {
            re_log::error_once!("Failed to send log message to web server: {err}");
        }
    }

    #[inline]
    fn flush_blocking(&self, timeout: std::time::Duration) -> Result<(), SinkFlushError> {
        self.sender
            .flush_blocking(timeout)
            .map_err(|err| match err {
                re_log_channel::FlushError::Closed => {
                    SinkFlushError::failed("The viewer is no longer subscribed")
                }
                re_log_channel::FlushError::Timeout => SinkFlushError::Timeout,
            })
    }

    fn default_batcher_config(&self) -> ChunkBatcherConfig {
        // The GRPC sink is typically used for live streams.
        ChunkBatcherConfig::LOW_LATENCY
    }
}

impl Drop for WebViewerSink {
    fn drop(&mut self) {
        if self.open_browser {
            // For small scripts that execute fast we run the risk of finishing
            // before the browser has a chance to connect.
            // Let's give it a little more time:
            re_log::info!("Sleeping a short while to give the browser time to connect…");
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }

        self.server_shutdown_signal.stop();
    }
}

// ----------------------------------------------------------------------------

/// Helper to spawn an instance of the [`WebViewerServer`] and configure a webviewer url.
#[cfg(feature = "web_viewer")]
pub struct WebViewerConfig {
    /// Ip to which the http server is bound.
    ///
    /// Defaults to 0.0.0.0
    pub bind_ip: String,

    /// The port to which the webviewer should bind.
    ///
    /// Defaults to [`WebViewerServerPort::AUTO`].
    pub web_port: WebViewerServerPort,

    /// The urls to which any spawned webviewer should connect.
    ///
    /// This url is a redap uri or a hosted RRD file that we retrieve via the message proxy.
    /// Has no effect if [`Self::open_browser`] is false.
    pub connect_to: Vec<String>,

    /// If set, adjusts the browser url to force a specific backend, either `webgl` or `webgpu`.
    ///
    /// Has no effect if [`Self::open_browser`] is false.
    pub force_wgpu_backend: Option<String>,

    /// If set, adjusts the browser url to set the video decoder setting, either `auto`, `prefer_software` or `prefer_hardware`.
    ///
    /// Has no effect if [`Self::open_browser`] is false.
    pub video_decoder: Option<String>,

    /// If set to `true`, opens the default browser after hosting the webviewer.
    ///
    /// Defaults to `true`.
    pub open_browser: bool,
}

#[cfg(feature = "web_viewer")]
impl Default for WebViewerConfig {
    fn default() -> Self {
        Self {
            bind_ip: "0.0.0.0".to_owned(),
            web_port: WebViewerServerPort::AUTO,
            connect_to: Vec::new(),
            force_wgpu_backend: None,
            video_decoder: None,
            open_browser: true,
        }
    }
}

#[cfg(feature = "web_viewer")]
impl WebViewerConfig {
    /// Helper to spawn an instance of the [`WebViewerServer`].
    /// This serves the HTTP+Wasm+JS files that make up the web-viewer.
    ///
    /// The server will immediately start listening for incoming connections
    /// and stop doing so when the returned [`WebViewerServer`] is dropped.
    ///
    /// Note: this does not include the gRPC server.
    pub fn host_web_viewer(self) -> Result<WebViewerServer, WebViewerServerError> {
        let Self {
            bind_ip,
            connect_to,
            web_port,
            force_wgpu_backend,
            video_decoder,
            open_browser,
        } = self;

        let web_server = WebViewerServer::new(&bind_ip, web_port)?;
        let http_web_viewer_url = web_server.server_url();

        let mut viewer_url = http_web_viewer_url;

        let mut first_arg = true;
        let mut append_argument = |arg| {
            let arg_delimiter = if first_arg {
                first_arg = false;
                "?"
            } else {
                "&"
            };
            viewer_url = format!("{viewer_url}{arg_delimiter}{arg}");
        };

        for source_url in connect_to {
            // TODO(jan): remove after we change from `rerun+http` to `rerun-http`
            let source_url = percent_encoding::utf8_percent_encode(
                &source_url,
                percent_encoding::NON_ALPHANUMERIC,
            );
            append_argument(format!("url={source_url}"));
        }
        if let Some(force_graphics) = force_wgpu_backend {
            append_argument(format!("renderer={force_graphics}"));
        }
        if let Some(video_decoder) = video_decoder {
            append_argument(format!("video_decoder={video_decoder}"));
        }

        re_log::info!("Hosting a web-viewer at {viewer_url}");
        if open_browser {
            webbrowser::open(&viewer_url).ok();
        }

        Ok(web_server)
    }
}

// ----------------------------------------------------------------------------

/// Serve log-data over gRPC and serve a Rerun web viewer over HTTP.
///
/// If the `open_browser` argument is `true`, your default browser
/// will be opened with a connected web-viewer.
///
/// If not, you can connect to this server using the `rerun` binary (`cargo install rerun-cli --locked`).
///
/// NOTE: you can not connect one `Session` to another.
///
/// This function returns immediately.
#[must_use = "the sink must be kept around to keep the servers running"]
pub fn new_sink(
    open_browser: bool,
    bind_ip: &str,
    web_port: WebViewerServerPort,
    grpc_port: u16,
    server_options: re_grpc_server::ServerOptions,
) -> Result<Box<dyn crate::sink::LogSink>, WebViewerSinkError> {
    Ok(Box::new(WebViewerSink::new(
        open_browser,
        bind_ip,
        web_port,
        grpc_port,
        server_options,
    )?))
}

/// Serves the Rerun Web Viewer (HTML+JS+Wasm) over http.
///
/// The server will immediately start listening for incoming connections
/// and stop doing so when the returned [`WebViewerServer`] is dropped.
///
/// Note: this does NOT start a gRPC server.
/// To start a gRPC server, use [`crate::RecordingStreamBuilder::serve_grpc`] and connect to it
/// by setting [`WebViewerConfig::connect_to`] to `rerun+http://localhost/proxy`.
///
/// Note: this function just calls [`WebViewerConfig::host_web_viewer`] and is here only
/// for convenience, visibility, and for symmetry with our Python SDK.
pub fn serve_web_viewer(config: WebViewerConfig) -> Result<WebViewerServer, WebViewerServerError> {
    config.host_web_viewer()
}
