use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use re_log_encoding::Compression;
use re_log_types::LogMsg;
use re_protos::sdk_comms::v1alpha1::message_proxy_service_client::MessageProxyServiceClient;
use re_protos::sdk_comms::v1alpha1::WriteMessagesRequest;
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

pub struct Client {
    thread: Option<JoinHandle<()>>,
    cmd_tx: UnboundedSender<Cmd>,
    shutdown_tx: Sender<()>,
    flush_timeout: Option<Duration>,
}

impl Client {
    #[expect(clippy::needless_pass_by_value)]
    pub fn new(uri: ProxyUri, options: Options) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        let thread = thread::Builder::new()
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
                    ));
            })
            .expect("Failed to spawn message proxy client thread");

        Self {
            thread: Some(thread),
            cmd_tx,
            shutdown_tx,
            flush_timeout: options.flush_timeout,
        }
    }

    pub fn send(&self, msg: LogMsg) {
        self.cmd_tx.send(Cmd::LogMsg(msg)).ok();
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
                            re_log::warn!("Flush timed out, not all messages were sent");
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
) {
    let endpoint = match Endpoint::from_shared(uri.origin.as_url()) {
        Ok(endpoint) => endpoint,
        Err(err) => {
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
                        re_log::debug!("shutting down client without flush");
                        return;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        continue;
                    }
                }
            }
        }
    };
    let mut client = MessageProxyServiceClient::new(channel);

    let stream = async_stream::stream! {
        loop {
            tokio::select! {
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(Cmd::LogMsg(msg)) => {
                            let msg = match re_log_encoding::protobuf_conversions::log_msg_to_proto(msg, compression) {
                                Ok(msg) => msg,
                                Err(err) => {
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
                                re_log::debug!("Failed to respond to flush: channel is closed");
                                return;
                            };
                        }

                        None => {
                            re_log::debug!("Channel closed");
                            break;
                        }
                    }
                }

                _ = shutdown_rx.recv() => {
                    re_log::debug!("Shutting down client without flush");
                    return;
                }
            }
        }
    };

    if let Err(err) = client.write_messages(stream).await {
        re_log::error!("Write messages call failed: {err}");
    };
}
