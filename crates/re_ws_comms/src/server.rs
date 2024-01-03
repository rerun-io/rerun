//! The server is a pub-sub architecture.
//!
//! Each incoming log message is stored, and sent to any connected client.
//! Each connecting client is first sent the history of stored log messages.
//!
//! In the future thing will be changed to a protocol where the clients can query
//! for specific data based on e.g. time.

use std::{collections::VecDeque, net::SocketAddr, sync::Arc};

use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Error};

use re_log_types::LogMsg;
use re_memory::MemoryLimit;
use re_smart_channel::ReceiveSet;

use crate::{server_url, RerunServerError, RerunServerPort};

#[derive(Clone)]
struct MessageQueue {
    server_memory_limit: MemoryLimit,
    messages: VecDeque<Arc<[u8]>>,
}

impl MessageQueue {
    pub fn new(server_memory_limit: MemoryLimit) -> Self {
        Self {
            server_memory_limit,
            messages: Default::default(),
        }
    }

    pub fn push(&mut self, msg: Arc<[u8]>) {
        self.gc_if_using_too_much_ram();
        self.messages.push_back(msg);
    }

    fn gc_if_using_too_much_ram(&mut self) {
        re_tracing::profile_function!();

        if let Some(max_bytes) = self.server_memory_limit.max_bytes {
            let max_bytes = max_bytes as u64;
            let bytes_used = self.messages.iter().map(|m| m.len() as u64).sum::<u64>();

            if max_bytes < bytes_used {
                re_tracing::profile_scope!("Drop messages");
                re_log::info_once!(
                    "Memory limit ({}) exceeded. Dropping old log messages from the server. Clients connecting after this will not see the full history.",
                    re_format::format_bytes(max_bytes as _)
                );

                let bytes_to_free = bytes_used - max_bytes;

                let mut bytes_dropped = 0;
                let mut messages_dropped = 0;

                while bytes_dropped < bytes_to_free {
                    if let Some(msg) = self.messages.pop_front() {
                        bytes_dropped += msg.len() as u64;
                        messages_dropped += 1;
                    } else {
                        break;
                    }
                }

                re_log::trace!(
                    "Dropped {} bytes in {messages_dropped} message(s)",
                    re_format::format_bytes(bytes_dropped as _)
                );
            }
        }
    }
}

/// Websocket host for relaying [`LogMsg`]s to a web viewer.
pub struct RerunServer {
    server_memory_limit: MemoryLimit,
    listener: TcpListener,
    local_addr: std::net::SocketAddr,
}

impl RerunServer {
    /// Create new [`RerunServer`] to relay [`LogMsg`]s to a websocket.
    /// The websocket will be available at `port`.
    ///
    /// A `bind_ip` of `"0.0.0.0"` is a good default.
    /// A port of 0 will let the OS choose a free port.
    pub async fn new(
        bind_ip: String,
        port: RerunServerPort,
        server_memory_limit: MemoryLimit,
    ) -> Result<Self, RerunServerError> {
        let bind_addr = format!("{bind_ip}:{port}");

        let listener = match TcpListener::bind(&bind_addr).await {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
                let bind_addr = format!("{bind_ip}:0");

                TcpListener::bind(&bind_addr)
                    .await
                    .map_err(|err| RerunServerError::BindFailed(RerunServerPort(0), err))?
            }
            Err(err) => return Err(RerunServerError::BindFailed(port, err)),
        };

        let slf = Self {
            server_memory_limit,
            local_addr: listener.local_addr()?,
            listener,
        };

        re_log::info!(
            "Listening for WebSocket traffic on {}. Connect with a Rerun Web Viewer.",
            slf.server_url()
        );

        Ok(slf)
    }

    /// Accept new connections
    pub async fn listen(self, rx: ReceiveSet<LogMsg>) -> Result<(), RerunServerError> {
        let (_shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
        self.listen_with_graceful_shutdown(rx, shutdown_rx).await
    }

    /// Accept new connections until we get a message on `shutdown_rx`
    pub async fn listen_with_graceful_shutdown(
        self,
        rx: ReceiveSet<LogMsg>,
        mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<(), RerunServerError> {
        let history = Arc::new(Mutex::new(MessageQueue::new(self.server_memory_limit)));

        let log_stream = to_broadcast_stream(rx, history.clone());

        loop {
            let (tcp_stream, _) = tokio::select! {
                res = self.listener.accept() => res?,
                _ = shutdown_rx.recv() => {
                    re_log::debug!("Shutting down WebSocket server");
                    return Ok(());
                }
            };

            let peer = tcp_stream.peer_addr()?;
            tokio::spawn(accept_connection(
                log_stream.clone(),
                peer,
                tcp_stream,
                history.clone(),
            ));
        }
    }

    /// Contains the `ws://` or `wss://` prefix.
    pub fn server_url(&self) -> String {
        server_url(&self.local_addr)
    }
}

/// Sync handle for the [`RerunServer`]
///
/// When dropped, the server will be shut down.
pub struct RerunServerHandle {
    local_addr: std::net::SocketAddr,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

impl Drop for RerunServerHandle {
    fn drop(&mut self) {
        re_log::info!("Shutting down Rerun server on {}", self.server_url());
        self.shutdown_tx.send(()).ok();
    }
}

impl RerunServerHandle {
    /// Create new [`RerunServer`] to relay [`LogMsg`]s to a websocket.
    /// Returns a [`RerunServerHandle`] that will shutdown the server when dropped.
    ///
    /// A `bind_ip` of `"0.0.0.0"` is a good default.
    /// A port of 0 will let the OS choose a free port.
    ///
    /// The caller needs to ensure that there is a `tokio` runtime running.
    pub fn new(
        rerun_rx: ReceiveSet<LogMsg>,
        bind_ip: String,
        requested_port: RerunServerPort,
        server_memory_limit: MemoryLimit,
    ) -> Result<Self, RerunServerError> {
        let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

        let rt = tokio::runtime::Handle::current();

        let ws_server = rt.block_on(tokio::spawn(async move {
            RerunServer::new(bind_ip, requested_port, server_memory_limit).await
        }))??;

        let local_addr = ws_server.local_addr;

        tokio::spawn(async move {
            ws_server
                .listen_with_graceful_shutdown(rerun_rx, shutdown_rx)
                .await
        });

        Ok(Self {
            local_addr,
            shutdown_tx,
        })
    }

    /// Contains the `ws://` or `wss://` prefix.
    pub fn server_url(&self) -> String {
        server_url(&self.local_addr)
    }
}

fn to_broadcast_stream(
    log_rx: ReceiveSet<LogMsg>,
    history: Arc<Mutex<MessageQueue>>,
) -> tokio::sync::broadcast::Sender<Arc<[u8]>> {
    let (tx, _) = tokio::sync::broadcast::channel(1024 * 1024);
    let tx1 = tx.clone();
    tokio::task::spawn_blocking(move || {
        while let Ok(msg) = log_rx.recv() {
            match msg.payload {
                re_smart_channel::SmartMessagePayload::Msg(data) => {
                    let bytes = crate::encode_log_msg(&data);
                    let bytes: Arc<[u8]> = bytes.into();
                    history.lock().push(bytes.clone());
                    if let Err(tokio::sync::broadcast::error::SendError(_bytes)) = tx1.send(bytes) {
                        // no receivers currently - that's fine!
                    }
                }
                re_smart_channel::SmartMessagePayload::Quit(err) => {
                    if let Some(err) = err {
                        re_log::warn!("Sender {} has left unexpectedly: {err}", msg.source);
                    } else {
                        re_log::debug!("Sender {} has left", msg.source);
                    }
                }
            }
        }
    });
    tx
}

async fn accept_connection(
    log_stream: tokio::sync::broadcast::Sender<Arc<[u8]>>,
    _peer: SocketAddr,
    tcp_stream: TcpStream,
    history: Arc<Mutex<MessageQueue>>,
) {
    // let span = re_log::span!(
    //     re_log::Level::INFO,
    //     "Connection",
    //     peer = _peer.to_string().as_str()
    // );
    // let _enter = span.enter();

    re_log::debug!("New WebSocket connection");

    if let Err(err) = handle_connection(log_stream, tcp_stream, history).await {
        match err {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => re_log::error!("Error processing connection: {err}"),
        }
    }
}

async fn handle_connection(
    log_stream: tokio::sync::broadcast::Sender<Arc<[u8]>>,
    tcp_stream: TcpStream,
    history: Arc<Mutex<MessageQueue>>,
) -> tungstenite::Result<()> {
    let ws_stream = accept_async(tcp_stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    {
        // Re-sending packet history - this is not water tight, but better than nothing.
        // TODO(emilk): water-proof resending of history + streaming of new stuff, without anything missed.
        let history: MessageQueue = history.lock().clone();
        for packet in history.messages {
            ws_sender
                .send(tungstenite::Message::Binary(packet.to_vec()))
                .await?;
        }
    }

    let mut log_rx = log_stream.subscribe();

    loop {
        tokio::select! {
            ws_msg = ws_receiver.next() => {
                match ws_msg {
                    Some(Ok(msg)) => {
                        re_log::debug!("Received message: {:?}", msg);
                    }
                    Some(Err(err)) => {
                        re_log::warn!("Error message: {err}");
                        break;
                    }
                    None => {
                        break;
                    }
                }
            }
            data_msg = log_rx.recv() => {
                let data_msg = data_msg.unwrap();

                ws_sender.send(tungstenite::Message::Binary(data_msg.to_vec())).await?;
            }
        }
    }

    Ok(())
}
