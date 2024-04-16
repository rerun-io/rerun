//! The server is a pub-sub architecture.
//!
//! Each incoming log message is stored, and sent to any connected client.
//! Each connecting client is first sent the history of stored log messages.
//!
//! In the future thing will be changed to a protocol where the clients can query
//! for specific data based on e.g. time.

use std::{
    collections::{HashMap, VecDeque},
    net::{TcpListener, TcpStream},
    sync::{atomic::AtomicUsize, Arc},
};

use parking_lot::Mutex;

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
    pub fn new(
        bind_ip: &str,
        port: RerunServerPort,
        server_memory_limit: MemoryLimit,
    ) -> Result<Self, RerunServerError> {
        let bind_addr = format!("{bind_ip}:{port}");

        let listener = match TcpListener::bind(bind_addr) {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
                let bind_addr = format!("{bind_ip}:0");

                TcpListener::bind(bind_addr)
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
            "Hosting a WebSocket server on {wsurl}. You can connect to this with a native viewer (`rerun {wsurl}`) or the web viewer (with `?url={wsurl}`).",
            wsurl=slf.server_url()
        );

        Ok(slf)
    }

    /// Starts a thread that accepts new connections.
    pub fn listen(self, rx: ReceiveSet<LogMsg>) -> Result<(), RerunServerError> {
        // TODO:
        //let (_shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
        self.listen_with_graceful_shutdown(rx) //, shutdown_rx)
    }

    /// Starts a thread that accepts new connections and shuts down when `shutdown_rx` receives a message.
    pub fn listen_with_graceful_shutdown(
        self,
        rx: ReceiveSet<LogMsg>,
        //mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<(), RerunServerError> {
        let message_broadcaster =
            Arc::new(ReceiveSetBroadcaster::new(rx, self.server_memory_limit));

        // TODO: handle shutdown. Maybe like so? https://stackoverflow.com/questions/56692961/graceful-exit-tcplistener-incoming

        std::thread::Builder::new()
            .name("rerun_ws_server: listener".to_owned())
            .spawn(move || {
                for tcp_stream in self.listener.incoming() {
                    match tcp_stream {
                        Ok(tcp_stream) => {
                            handle_connection(message_broadcaster.clone(), tcp_stream);
                        }
                        Err(err) => {
                            re_log::warn!("Error accepting WebSocket connection: {err}");
                            break;
                        }
                    }
                }
            })?;

        Ok(())
    }

    /// Contains the `ws://` or `wss://` prefix.
    pub fn server_url(&self) -> String {
        server_url(&self.local_addr)
    }
}

fn handle_connection(message_broadcaster: Arc<ReceiveSetBroadcaster>, tcp_stream: TcpStream) {
    if let Err(err) = std::thread::Builder::new()
        .name("rerun_ws_server: connection".to_owned())
        .spawn(move || {
            let address = tcp_stream.peer_addr();
            re_log::debug!("New WebSocket connection at {:?}", address);

            let mut ws_stream = match tungstenite::accept(tcp_stream) {
                Ok(ws_stream) => ws_stream,
                Err(err) => {
                    re_log::warn!("Error accepting WebSocket connection: {err}");
                    return;
                }
            };

            {
                let (client_id, log_stream) = message_broadcaster.add_client();

                while let Ok(msg) = log_stream.recv() {
                    if let Err(err) = ws_stream.send(tungstenite::Message::Binary(msg.to_vec())) {
                        re_log::warn!("Error sending message to WebSocket client: {err}");
                        break;
                    }
                }

                message_broadcaster.remove_client(client_id);
            }

            re_log::debug!("Closing WebSocket connection at {:?}", address);
        })
    {
        re_log::error!("Failed to spawn thread for handling incoming WebSocket connection: {err}");
    }
}

/// Sync handle for the [`RerunServer`]
///
/// When dropped, the server will be shut down.
pub struct RerunServerHandle {
    local_addr: std::net::SocketAddr,
    //shutdown_tx: tokio::sync::broadcast::Sender<()>, // TODO:
}

impl Drop for RerunServerHandle {
    fn drop(&mut self) {
        re_log::info!("Shutting down Rerun server on {}", self.server_url());
        //self.shutdown_tx.send(()).ok();
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
        bind_ip: &str,
        requested_port: RerunServerPort,
        server_memory_limit: MemoryLimit,
    ) -> Result<Self, RerunServerError> {
        let ws_server = RerunServer::new(bind_ip, requested_port, server_memory_limit)?;
        let local_addr = ws_server.local_addr;
        ws_server.listen_with_graceful_shutdown(rerun_rx)?;

        Ok(Self {
            local_addr,
            //shutdown_tx,
        })
    }

    /// Contains the `ws://` or `wss://` prefix.
    pub fn server_url(&self) -> String {
        server_url(&self.local_addr)
    }
}

/// Broadcasts messages to all connected clients and stores a history of messages to resend to new clients.
struct ReceiveSetBroadcaster {
    inner: Arc<Mutex<ReceiveSetBroadcasterInnerState>>,
    next_client_id: AtomicUsize,
}

/// Inner state of the [`ReceiveSetBroadcaster`], protected by a mutex.
struct ReceiveSetBroadcasterInnerState {
    /// Don't allow adding to the history while adding/removing clients.
    /// This way, no messages history is lost!
    history: MessageQueue,
    clients: HashMap<usize, std::sync::mpsc::Sender<Arc<[u8]>>>,
}

impl ReceiveSetBroadcaster {
    fn new(log_rx: ReceiveSet<LogMsg>, server_memory_limit: MemoryLimit) -> Self {
        let inner = Arc::new(Mutex::new(ReceiveSetBroadcasterInnerState {
            history: MessageQueue::new(server_memory_limit),
            clients: HashMap::new(),
        }));
        let inner_cpy = inner.clone();

        if let Err(err) = std::thread::Builder::new()
            .name("rerun_ws_client_broadcaster".to_owned())
            .spawn(move || {
                while let Ok(msg) = log_rx.recv() {
                    match msg.payload {
                        re_smart_channel::SmartMessagePayload::Msg(data) => {
                            let bytes = crate::encode_log_msg(&data);
                            let bytes: Arc<[u8]> = bytes.into();

                            {
                                let mut inner = inner.lock();
                                inner.history.push(bytes.clone());
                                for client in inner.clients.values() {
                                    if let Err(err) = client.send(bytes.clone()) {
                                        re_log::warn!(
                                            "Error sending message to web socket client: {err}"
                                        );
                                    }
                                }
                            }
                        }
                        re_smart_channel::SmartMessagePayload::Quit(err) => {
                            if let Some(err) = err {
                                re_log::warn!("Sender {} has left unexpectedly: {err}", msg.source);
                            } else {
                                re_log::debug!("Sender {} has left", msg.source);
                            }
                            return;
                        }
                    }
                }
            })
        {
            re_log::error!(
                "Failed to spawn thread for broadcasting messages to websocket connections: {err}"
            );
        }

        Self {
            inner: inner_cpy,
            next_client_id: AtomicUsize::new(0),
        }
    }

    /// Adds a client to the broadcaster and replies all message history so far to it.
    ///
    /// Returns a client id that can be used to remove the client and a receive channel.
    fn add_client(&self) -> (usize, std::sync::mpsc::Receiver<Arc<[u8]>>) {
        let client_id = self
            .next_client_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (tx, rx) = std::sync::mpsc::channel();

        {
            let mut inner = self.inner.lock();

            for msg in &inner.history.messages {
                if let Err(err) = tx.send(msg.clone()) {
                    re_log::warn!("Error sending message to web socket client: {err}");
                }
            }

            inner.clients.insert(client_id, tx);
        }
        (client_id, rx)
    }

    /// Removes a client from the broadcaster that was previously added with [`Self::add_client`].
    fn remove_client(&self, client_id: usize) {
        let mut inner = self.inner.lock();
        inner.clients.remove(&client_id);
    }
}
