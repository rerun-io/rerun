use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use re_chunk::external::crossbeam::atomic::AtomicCell;
use re_log_encoding::Compression;
use re_log_types::LogMsg;
use re_protos::sdk_comms::v1alpha1::WriteMessagesRequest;
use re_protos::sdk_comms::v1alpha1::message_proxy_service_client::MessageProxyServiceClient;
use re_uri::ProxyUri;
use tokio::runtime;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;
use tonic::transport::Endpoint;

enum Cmd {
    LogMsg(LogMsg),
    Flush(oneshot::Sender<()>),
}

#[derive(Clone)]
pub struct Options {
    pub compression: Compression,
    pub flush_timeout: Option<Duration>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            compression: Compression::LZ4,
            flush_timeout: Default::default(),
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

    #[error("Failed to send messages")]
    FailedToSendMessages(tonic::Code),
}

/// The connection state of a client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientConnectionState {
    /// The client is connecting to the remote server.
    Connecting,

    /// The client is connected to the remote server.
    Connected,

    /// The client is disconnected from the remote server.
    ///
    /// No new connection attempts will be made.
    Disconnected(Result<(), ClientConnectionFailure>),
}

/// This is the gRPC client used for the SDK-side log-sink.
pub struct Client {
    thread: Option<JoinHandle<()>>,
    cmd_tx: UnboundedSender<Cmd>,
    shutdown_tx: Sender<()>,
    flush_timeout: Option<Duration>,
    status: Arc<AtomicCell<ClientConnectionState>>,
}

impl Client {
    #[expect(clippy::needless_pass_by_value)]
    pub fn new(uri: ProxyUri, options: Options) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        let status = Arc::new(AtomicCell::new(ClientConnectionState::Connecting));
        let thread = {
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
                            uri,
                            cmd_rx,
                            shutdown_rx,
                            options.compression,
                            status,
                        ));
                })
                .expect("Failed to spawn message proxy client thread")
        };

        Self {
            thread: Some(thread),
            cmd_tx,
            shutdown_tx,
            flush_timeout: options.flush_timeout,
            status,
        }
    }

    pub fn send(&self, msg: LogMsg) {
        self.cmd_tx.send(Cmd::LogMsg(msg)).ok();
    }

    /// Whether the client is connected to a remote server.
    pub fn status(&self) -> ClientConnectionState {
        self.status.load()
    }

    pub fn flush(&self) {
        use tokio::sync::oneshot::error::TryRecvError;

        let (tx, mut rx) = oneshot::channel();
        if self.cmd_tx.send(Cmd::Flush(tx)).is_err() {
            re_log::debug!("Flush failed: already shut down.");
            return;
        };

        let start = std::time::Instant::now();

        loop {
            match rx.try_recv() {
                Ok(_) => {
                    re_log::trace!("Flush complete");
                    break;
                }
                Err(TryRecvError::Empty) => {
                    if let Some(timeout) = self.flush_timeout {
                        let elapsed = start.elapsed();
                        if elapsed >= timeout {
                            re_log::warn!(
                                "Flush timed out, not all messages were sent. The timeout can be adjusted when connecting via gRPC."
                            );
                            break;
                        }
                    }
                    std::thread::yield_now();
                }
                Err(TryRecvError::Closed) => {
                    re_log::warn!("Flush failed, not all messages were sent");
                    break;
                }
            }
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        re_log::debug!("Shutting down message proxy client");
        // Wait for flush
        self.flush();

        // Quit immediately - no more messages left in the queue
        if let Err(err) = self.shutdown_tx.try_send(()) {
            re_log::error!("Failed to gracefully shut down message proxy client: {err}");
            return;
        };

        // Wait for the shutdown
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        };

        re_log::debug!("Message proxy client has shut down");
    }
}

async fn message_proxy_client(
    uri: ProxyUri,
    mut cmd_rx: UnboundedReceiver<Cmd>,
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

    let channel = loop {
        match endpoint.connect().await {
            Ok(channel) => break channel,
            Err(err) => {
                re_log::debug!("failed to connect to message proxy server: {err}");
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        status.store(ClientConnectionState::Disconnected(Ok(())));
                        re_log::debug!("Shutting down client without flush");
                        return;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        continue;
                    }
                }
            }
        }
    };

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
                            // Insert the timestamp metadata into the Arrow message for accurate e2e latency measurements:
                             log_msg.insert_arrow_record_batch_metadata(
                                re_sorbet::timestamp_metadata::KEY_TIMESTAMP_SDK_IPC_ENCODE.to_owned(),
                                re_sorbet::timestamp_metadata::now_timestamp(),
                            );

                            let msg = match re_log_encoding::protobuf_conversions::log_msg_to_proto(log_msg, compression) {
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
                                log_msg: Some(msg),
                            };

                            yield msg;
                        }

                        Some(Cmd::Flush(tx)) => {
                            // Messages are received in order, so once we receive a `flush`
                            // we know we've sent all messages before that flush through already.
                            re_log::debug!("Flush requested");
                            if tx.send(()).is_err() {
                                // Flush channel may already be closed for non-blocking flush, so this isn't an error.
                                re_log::debug!("Failed to respond to flush: flush report channel was closed");
                                break;
                            };
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

    let disconnect_result = if let Err(err) = client.write_messages(stream).await {
        re_log::error!("Write messages call failed: {err}");

        // Ignore status code "Unknown" since this was observed to happen on regular Viewer shutdowns.
        if err.code() != tonic::Code::Ok && err.code() != tonic::Code::Unknown {
            Err(ClientConnectionFailure::FailedToSendMessages(err.code()))
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
