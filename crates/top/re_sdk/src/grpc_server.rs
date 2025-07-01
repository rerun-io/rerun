use re_log_types::LogMsg;

/// A [`crate::sink::LogSink`] tied to a hosted Rerun gRPC server.
///
/// The hosted gRPC server may be connected to by any SDK or Viewer.
///
/// All data sent through this sink is immediately redirected to the gRPC server.
pub struct GrpcServerSink {
    uri: re_uri::ProxyUri,

    /// Sender to send messages to the gRPC server.
    sender: re_smart_channel::Sender<LogMsg>,

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
        server_memory_limit: re_memory::MemoryLimit,
    ) -> Result<Self, std::net::AddrParseError> {
        let (server_shutdown_signal, shutdown) = re_grpc_server::shutdown::shutdown();

        let grpc_server_addr = format!("{bind_ip}:{grpc_port}").parse()?;

        let uri = re_uri::ProxyUri::new(re_uri::Origin::from_scheme_and_socket_addr(
            re_uri::Scheme::RerunHttp,
            grpc_server_addr,
        ));
        let (channel_tx, channel_rx) = re_smart_channel::smart_channel::<re_log_types::LogMsg>(
            re_smart_channel::SmartMessageSource::MessageProxy(uri.clone()),
            re_smart_channel::SmartChannelSource::Sdk,
        );
        let server_handle = std::thread::Builder::new()
            .name("message_proxy_server".to_owned())
            .spawn(move || {
                let mut builder = tokio::runtime::Builder::new_current_thread();
                builder.enable_all();
                let rt = builder.build().expect("failed to build tokio runtime");

                rt.block_on(re_grpc_server::serve_from_channel(
                    grpc_server_addr,
                    server_memory_limit,
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
        if let Err(err) = self.sender.send(msg) {
            re_log::error_once!("Failed to send log message to gRPC server: {err}");
        }
    }

    #[inline]
    fn flush_blocking(&self) {
        if let Err(err) = self.sender.flush_blocking() {
            re_log::error_once!("Failed to flush: {err}");
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Drop for GrpcServerSink {
    fn drop(&mut self) {
        self.sender.flush_blocking().ok();
        self.server_shutdown_signal.stop();
    }
}
