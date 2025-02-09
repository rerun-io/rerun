use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use re_log_encoding::Compression;
use re_log_types::LogMsg;
use re_protos::sdk_comms::v0::message_proxy_client::MessageProxyClient;
use tokio::runtime;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;
use tonic::transport::Endpoint;

use super::MessageProxyUrl;

enum Cmd {
    LogMsg(LogMsg),
    Flush(oneshot::Sender<()>),
}

#[derive(Clone)]
pub struct Options {
    compression: Compression,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            compression: Compression::LZ4,
        }
    }
}

pub struct Client {
    thread: Option<JoinHandle<()>>,
    cmd_tx: UnboundedSender<Cmd>,
    shutdown_tx: Sender<()>,
}

impl Client {
    #[expect(clippy::needless_pass_by_value)]
    pub fn new(url: MessageProxyUrl, options: Options) -> Self {
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
                        url.to_http(),
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
        }
    }

    pub fn send(&self, msg: LogMsg) {
        self.cmd_tx.send(Cmd::LogMsg(msg)).ok();
    }

    pub fn flush(&self) {
        let (tx, rx) = oneshot::channel();
        if self.cmd_tx.send(Cmd::Flush(tx)).is_err() {
            re_log::debug!("Flush failed: already shut down.");
            return;
        };

        match rx.blocking_recv() {
            Ok(_) => {
                re_log::debug!("Flush complete");
            }
            Err(_) => {
                re_log::debug!("Flush failed, not all messages were sent");
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
            re_log::error!("failed to gracefully shut down message proxy client: {err}");
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
    url: String,
    mut cmd_rx: UnboundedReceiver<Cmd>,
    mut shutdown_rx: Receiver<()>,
    compression: Compression,
) {
    let endpoint = match Endpoint::from_shared(url) {
        Ok(endpoint) => endpoint,
        Err(err) => {
            re_log::error!("Invalid message proxy server endpoint: {err}");
            return;
        }
    };

    // Temporarily buffer messages while we're connecting:
    let mut buffered_messages = vec![];
    let channel = loop {
        match endpoint.connect().await {
            Ok(channel) => break channel,
            Err(err) => {
                re_log::debug!("failed to connect to message proxy server: {err}");
                tokio::select! {
                    cmd = cmd_rx.recv() => {
                        match cmd {
                            Some(Cmd::LogMsg(msg)) => {
                                buffered_messages.push(msg);
                            }
                            Some(Cmd::Flush(tx)) => {
                                re_log::warn_once!(
                                    "Attempted to flush while gRPC client was connecting."
                                );
                                if tx.send(()).is_err() {
                                    re_log::debug!("Failed to respond to flush: channel is closed");
                                    return;
                                };
                            }
                            None => {
                                re_log::debug!("Channel closed");
                                return;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        re_log::debug!("shutting down client without flush");
                        return;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(200)) => {
                        continue;
                    }
                }
            }
        }
    };
    let mut client = MessageProxyClient::new(channel);

    let stream = async_stream::stream! {
        for msg in buffered_messages {
            let msg = match re_log_encoding::protobuf_conversions::log_msg_to_proto(msg, compression) {
                Ok(msg) => msg,
                Err(err) => {
                    re_log::error!("Failed to encode message: {err}");
                    break;
                }
            };

            yield msg;
        }

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
