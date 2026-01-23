use std::time::Duration;

use re_chunk::ChunkBatcherConfig;
use re_log_types::LogMsg;

use crate::sink::SinkFlushError;

/// A [`crate::sink::LogSink`] tied to a hosted Rerun gRPC server.
///
/// The hosted gRPC server may be connected to by any SDK or Viewer.
///
/// All data sent through this sink is immediately redirected to the gRPC server.
///
/// NOTE: When the `GrpcServerSink` is dropped, it will shut down the gRPC server.
/// If this sink has been passed to a `RecordingStream`, dropping, or disconnecting
/// the `RecordingStream` will indirectly drop this sink and shut down the server.
pub struct GrpcServerSink {
    uri: re_uri::ProxyUri,

    /// Sender to send messages to the gRPC server.
    sender: re_log_channel::LogSender,

    /// The gRPC server thread.
    _server_handle: std::thread::JoinHandle<()>,

    /// Rerun websocket server.
    server_shutdown_signal: re_grpc_server::shutdown::Signal,
}

impl GrpcServerSink {
    /// A `bind_ip` of `"0.0.0.0"` is a good default.
    pub fn new(
        bind_ip: &str,
        grpc_port: u16,
        server_options: re_grpc_server::ServerOptions,
    ) -> Result<Self, std::net::AddrParseError> {
        let (server_shutdown_signal, shutdown) = re_grpc_server::shutdown::shutdown();

        let grpc_server_addr = format!("{bind_ip}:{grpc_port}").parse()?;

        let uri = re_uri::ProxyUri::new(re_uri::Origin::from_scheme_and_socket_addr(
            re_uri::Scheme::RerunHttp,
            grpc_server_addr,
        ));
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

        Ok(Self {
            uri,
            sender: channel_tx,
            _server_handle: server_handle,
            server_shutdown_signal,
        })
    }

    /// What to connect the viewer to
    pub fn uri(&self) -> re_uri::ProxyUri {
        self.uri.clone()
    }
}

impl crate::sink::LogSink for GrpcServerSink {
    fn send(&self, msg: LogMsg) {
        if let Err(err) = self.sender.send(msg.into()) {
            re_log::error_once!("Failed to send log message to gRPC server: {err}");
        }
    }

    #[inline]
    fn flush_blocking(&self, timeout: Duration) -> Result<(), SinkFlushError> {
        self.sender
            .flush_blocking(timeout)
            .map_err(|err| match err {
                re_log_channel::FlushError::Closed => {
                    SinkFlushError::failed("gRPC server thread shut down")
                }
                re_log_channel::FlushError::Timeout => SinkFlushError::Timeout,
            })
    }

    fn default_batcher_config(&self) -> ChunkBatcherConfig {
        // The GRPC sink is typically used for live streams.
        ChunkBatcherConfig::LOW_LATENCY
    }
}

impl Drop for GrpcServerSink {
    fn drop(&mut self) {
        if let Err(err) = self.sender.flush_blocking(Duration::MAX) {
            re_log::error!("Failed to flush gRPC queue: {err}");
        }
        self.server_shutdown_signal.stop();
    }
}
