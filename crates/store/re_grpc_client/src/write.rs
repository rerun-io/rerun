use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use re_chunk::external::crossbeam::atomic::AtomicCell;
use re_log_encoding::ToTransport as _;
use re_log_encoding::rrd::Compression;
use re_log_types::LogMsg;
use re_protos::sdk_comms::v1alpha1::WriteMessagesRequest;
use re_protos::sdk_comms::v1alpha1::message_proxy_service_client::MessageProxyServiceClient;
use re_uri::ProxyUri;
use tokio::runtime;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tonic::transport::Endpoint;
use web_time::Instant;

use crate::TonicStatusError;

/// An error that can occur when flushing.
#[derive(Debug, thiserror::Error)]
pub enum GrpcFlushError {
    #[error("gRPC flush timed out after {num_sec:.0}s - not all messages were sent")]
    Timeout { num_sec: f32 },

    #[error("gRPC has been unable to connect for {duration_sec:.0}s, uri: {uri}")]
    FailedToConnect { uri: ProxyUri, duration_sec: f32 },

    #[error("gRPC connection gracefully disconnected, uri: {uri}")]
    GracefulDisconnect { uri: ProxyUri },

    #[error("{0}")]
    InternalError(String),

    #[error("gRPC connection severed: {err}, uri: {uri}")]
    ErrorDisconnect {
        uri: ProxyUri,
        err: ClientConnectionFailure,
    },
}

impl GrpcFlushError {
    pub fn from_status(uri: ProxyUri, status: ClientConnectionState) -> Self {
        match status {
            ClientConnectionState::Connecting { started } => Self::FailedToConnect {
                uri,
                duration_sec: started.elapsed().as_secs_f32(),
            },
            ClientConnectionState::Connected => Self::InternalError(
                "gRPC connection is open, but flush still failed. Probably a bug in the Rerun SDK"
                    .to_owned(),
            ),
            ClientConnectionState::Disconnected(Ok(())) => Self::GracefulDisconnect { uri },
            ClientConnectionState::Disconnected(Err(err)) => Self::ErrorDisconnect { uri, err },
        }
    }
}

enum Cmd {
    LogMsg(LogMsg),
    Flush {
        on_done: crossbeam::channel::Sender<()>,
    },
}

#[derive(Clone)]
pub struct Options {
    pub compression: Compression,

    /// If we have not yet connected to the client, then
    /// do not block [`Client::flush_blocking`] for longer than this.
    ///
    /// We will still retry connecting for however long it takes.
    /// But blocking [`Client::flush_blocking`] forever when the
    /// server just isn't there is not a good idea.
    pub connect_timeout_on_flush: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            compression: Compression::LZ4,
            connect_timeout_on_flush: Duration::from_secs(5),
        }
    }
}

/// Why a client was unintentionally disconnected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ClientConnectionFailure {
    #[error("Invalid message proxy server endpoint")]
    InvalidEndpoint,

    #[error("Failed to encode message")]
    FailedToEncodeMessage,

    #[error("Failed to send messages: {0}")]
    FailedToSendMessages(tonic::Code),
}

/// The connection state of a client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientConnectionState {
    /// The client is connecting to the remote server.
    Connecting { started: Instant },

    /// The client is connected to the remote server.
    Connected,

    /// The client is disconnected from the remote server.
    ///
    /// No new connection attempts will be made.
    Disconnected(Result<(), ClientConnectionFailure>),
}

/// This is the gRPC client used for the SDK-side log-sink.
pub struct Client {
    uri: ProxyUri,
    options: Options,
    thread: Option<JoinHandle<()>>,
    cmd_tx: Sender<Cmd>,
    shutdown_tx: Sender<()>,
    status: Arc<AtomicCell<ClientConnectionState>>,
}

impl Client {
    pub fn new(uri: ProxyUri, options: Options) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(100); // TODO(#11024): specify size in bytes instead of number of messages
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        let status = Arc::new(AtomicCell::new(ClientConnectionState::Connecting {
            started: Instant::now(),
        }));
        let thread = {
            let uri = uri.clone();
            let status = status.clone();
            thread::Builder::new()
                .name("message_proxy_client".to_owned())
                .spawn(move || {
                    let mut runtime = runtime::Builder::new_current_thread();
                    runtime.enable_all();
                    runtime
                        .build()
                        .expect("Failed to build tokio runtime")
                        .block_on(message_proxy_client(
                            uri.clone(),
                            cmd_rx,
                            shutdown_rx,
                            options.compression,
                            status,
                        ));
                })
                .expect("Failed to spawn message proxy client thread")
        };

        Self {
            uri,
            options,
            thread: Some(thread),
            cmd_tx,
            shutdown_tx,
            status,
        }
    }

    /// Send a message asynchronously with backpressure.
    ///
    /// This will block (async) if the channel is full.
    pub async fn send_async(&self, msg: LogMsg) {
        self.cmd_tx.send(Cmd::LogMsg(msg)).await.ok();
    }

    /// Send a message with blocking backpressure.
    ///
    /// This will block the current thread if the channel is full.
    pub fn send_blocking(&self, msg: LogMsg) {
        self.send_cmd_blocking(Cmd::LogMsg(msg)).ok();
    }

    fn send_cmd_blocking(&self, cmd: Cmd) -> Result<(), ()> {
        re_tracing::profile_function!();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            if handle.runtime_flavor() == runtime::RuntimeFlavor::MultiThread {
                tokio::task::block_in_place(|| self.cmd_tx.blocking_send(cmd))
            } else {
                re_log::warn_once!(
                    "Single-threaded tokio runtime detected - please use a multi-threaded runtime for best performance with Rerun's gRPC client. Falling back to async send."
                );
                self.cmd_tx.blocking_send(cmd)
            }
        } else {
            self.cmd_tx.blocking_send(cmd)
        }.map_err(|_ignored_details| ())
    }

    /// Whether the client is connected to a remote server.
    pub fn status(&self) -> ClientConnectionState {
        self.status.load()
    }

    /// Block until all messages are sent, or there is a failure.
    ///
    /// If the gRPC connection has not yet been established,
    /// this call will block for _at most_ [`Options::connect_timeout_on_flush`].
    /// This means this function will only block all the way to the given `timeout` argument
    /// IF there is some hope of progress being made - i.e. the connection open.
    ///
    /// If the gRPC connection was severed before all messages were sent,
    /// this function will return an error.
    ///
    /// If a timeout is provided, we will break when that timeout is received,
    /// returning an error.
    pub fn flush_blocking(&self, timeout: Duration) -> Result<(), GrpcFlushError> {
        re_tracing::profile_function!();

        let (flush_done_tx, flush_done_rx) = crossbeam::channel::bounded(1); // oneshot
        if self
            .send_cmd_blocking(Cmd::Flush {
                on_done: flush_done_tx,
            })
            .is_err()
        {
            return Err(GrpcFlushError::from_status(self.uri.clone(), self.status()));
        }

        let start = std::time::Instant::now();

        let very_slow = std::time::Duration::from_secs(10);
        let mut has_emitted_slow_warning = false;

        loop {
            // Check in if the connection status has changed every now and then.
            let interval = Duration::from_secs(1).min(timeout); // This could be better, but is good enough.
            match flush_done_rx.recv_timeout(interval) {
                Ok(()) => {
                    let elapsed = start.elapsed();
                    if has_emitted_slow_warning {
                        re_log::info!(
                            "gRPC flush completed in {:.1} seconds",
                            elapsed.as_secs_f32()
                        );
                    } else {
                        re_log::trace!(
                            "gRPC flush completed in {:.1} seconds",
                            elapsed.as_secs_f32()
                        );
                    }
                    return Ok(());
                }
                Err(crossbeam::channel::RecvTimeoutError::Timeout) => {
                    let elapsed = start.elapsed();

                    if timeout < elapsed {
                        return Err(GrpcFlushError::Timeout {
                            num_sec: elapsed.as_secs_f32(),
                        });
                    }

                    if !has_emitted_slow_warning && very_slow <= start.elapsed() {
                        if timeout < Duration::from_secs(10_000) {
                            re_log::info!(
                                "Flushing the gRPC stream has taken over {:.1}s seconds (timeout: {:.0}s); will keep waiting…",
                                elapsed.as_secs_f32(),
                                timeout.as_secs_f32(),
                            );
                        } else {
                            re_log::info!(
                                "Flushing the gRPC stream has taken over {:.1}s seconds; will keep waiting…",
                                elapsed.as_secs_f32()
                            );
                        }
                        has_emitted_slow_warning = true;
                    }

                    match self.status() {
                        ClientConnectionState::Connecting { started } => {
                            // We check the time from when the connection initially started.
                            // This means the flush can return a failure quicker than its timeout.
                            // Otherwise a bad URL would always lead to a flush call blocking for `connect_timeout_on_flush`.
                            // That would also be fine 🤷
                            if self.options.connect_timeout_on_flush < started.elapsed() {
                                return Err(GrpcFlushError::FailedToConnect {
                                    uri: self.uri.clone(),
                                    duration_sec: started.elapsed().as_secs_f32(),
                                });
                            }
                        }
                        ClientConnectionState::Connected => {
                            // Keep waiting
                        }
                        ClientConnectionState::Disconnected(_) => {
                            return Err(GrpcFlushError::from_status(
                                self.uri.clone(),
                                self.status(),
                            ));
                        }
                    }
                }
                Err(crossbeam::channel::RecvTimeoutError::Disconnected) => {
                    return Err(GrpcFlushError::from_status(self.uri.clone(), self.status()));
                }
            }
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        re_log::debug!("Shutting down message proxy client");

        // Wait for flush, blocking forever if needed.
        if let Err(err) = self.flush_blocking(Duration::MAX) {
            re_log::error!("Failed to flush gRPC messages during shutdown: {err}");
        }

        // Quit immediately - no more messages left in the queue
        if let Err(err) = self.shutdown_tx.try_send(()) {
            re_log::error!("Failed to gracefully shut down message proxy client: {err}");
            return;
        }

        // Wait for the shutdown
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }

        re_log::debug!("Message proxy client has shut down");
    }
}

async fn message_proxy_client(
    uri: ProxyUri,
    mut cmd_rx: Receiver<Cmd>,
    mut shutdown_rx: Receiver<()>,
    compression: Compression,
    status: Arc<AtomicCell<ClientConnectionState>>,
) {
    let endpoint = match Endpoint::from_shared(uri.origin.as_url()) {
        Ok(endpoint) => endpoint,
        Err(err) => {
            status.store(ClientConnectionState::Disconnected(Err(
                ClientConnectionFailure::InvalidEndpoint,
            )));
            re_log::error!("Invalid message proxy server endpoint: {err}");
            return;
        }
    };

    let mut last_connect_failure_log_time: Option<Instant> = None;
    let channel = loop {
        match endpoint.connect().await {
            Ok(channel) => break channel,
            Err(err) => {
                let log_interval = Duration::from_secs(5);
                if last_connect_failure_log_time
                    .is_none_or(|last_log_time| log_interval < last_log_time.elapsed())
                {
                    re_log::debug!(?uri, "Failed to connect: {err}, retrying…");
                    last_connect_failure_log_time = Some(Instant::now());
                }

                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        status.store(ClientConnectionState::Disconnected(Ok(())));
                        re_log::debug!("Shutting down client without flush");
                        return;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    }
                }
            }
        }
    };

    re_log::debug!(?uri, "Connected");
    status.store(ClientConnectionState::Connected);

    let mut client = MessageProxyServiceClient::new(channel)
        .max_decoding_message_size(crate::MAX_DECODING_MESSAGE_SIZE);

    let stream_status = status.clone();
    let stream = async_stream::stream! {
        loop {
            tokio::select! {
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(Cmd::LogMsg(mut log_msg)) => {
                            if let Some(metadata_key) = re_sorbet::TimestampLocation::IPCEncode.metadata_key() {
                                // Insert the timestamp metadata into the Arrow message for accurate e2e latency measurements:
                                log_msg.insert_arrow_record_batch_metadata(
                                    metadata_key.to_owned(),
                                    re_sorbet::timestamp_metadata::now_timestamp(),
                                );
                            }

                            let msg = match log_msg.to_transport(compression) {
                                Ok(msg) => msg,
                                Err(err) => {
                                    stream_status.store(ClientConnectionState::Disconnected(
                                        Err(ClientConnectionFailure::FailedToEncodeMessage),
                                    ));
                                    re_log::error!("Failed to encode message: {err}");
                                    break;
                                }
                            };

                            let msg = WriteMessagesRequest {
                                log_msg: Some(msg.into()),
                            };

                            yield msg;
                        }

                        Some(Cmd::Flush { on_done }) => {
                            // Messages are received in order, so once we receive a `flush`
                            // we know we've sent all messages before that flush through already.
                            re_log::trace!("Flush requested");
                            if re_quota_channel::send_crossbeam(&on_done, ()).is_err() {
                                // Flush channel may already be closed for non-blocking flush, so this isn't an error.
                                re_log::debug!("Failed to respond to flush: flush report channel was closed");
                                break;
                            }
                        }

                        None => {
                            // Assume channel closing is intentional, so don't report as error.
                            re_log::debug!("Shutdown channel closed");
                            break;
                        }
                    }
                }

                _ = shutdown_rx.recv() => {
                    re_log::debug!("Shutting down client without flush");
                    break;
                }
            }
        }
    };

    let disconnect_result = if let Err(status) = client.write_messages(stream).await {
        re_log::error!(
            "Write messages call failed: {}",
            TonicStatusError::from(status.clone())
        );

        // Ignore status code "Unknown" since this was observed to happen on regular Viewer shutdowns.
        if status.code() != tonic::Code::Ok && status.code() != tonic::Code::Unknown {
            Err(ClientConnectionFailure::FailedToSendMessages(status.code()))
        } else {
            Ok(())
        }
    } else {
        Ok(())
    };

    // Don't set error status if we already did so in the stream.
    if !matches!(status.load(), ClientConnectionState::Disconnected(_)) {
        status.store(ClientConnectionState::Disconnected(disconnect_result));
    }
}
