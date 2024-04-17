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
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        Arc,
    },
};

use parking_lot::Mutex;
use polling::{Event, Poller};

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
///
/// When dropped, the server will be shut down.
pub struct RerunServer {
    local_addr: std::net::SocketAddr,

    listener_join_handle: Option<std::thread::JoinHandle<()>>,
    poller: Arc<Poller>,
    shutdown_flag: Arc<AtomicBool>,
}

impl RerunServer {
    /// Create new [`RerunServer`] to relay [`LogMsg`]s to a websocket.
    /// The websocket will be available at `port`.
    ///
    /// A `bind_ip` of `"0.0.0.0"` is a good default.
    /// A port of 0 will let the OS choose a free port.
    ///
    /// Once created, the server will immediately start listening for connections.
    pub fn new(
        rerun_rx: ReceiveSet<LogMsg>,
        bind_ip: &str,
        port: RerunServerPort,
        server_memory_limit: MemoryLimit,
    ) -> Result<Self, RerunServerError> {
        let bind_addr = format!("{bind_ip}:{port}");

        let listener_socket = match TcpListener::bind(bind_addr) {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
                let bind_addr = format!("{bind_ip}:0");

                TcpListener::bind(bind_addr)
                    .map_err(|err| RerunServerError::BindFailed(RerunServerPort(0), err))?
            }
            Err(err) => return Err(RerunServerError::BindFailed(port, err)),
        };

        // Blocking listener socket seems much easier at first glance:
        // No polling needed and as such no extra libraries!
        // However, there is no portable way of stopping an `accept` call on a blocking socket.
        // Therefore, we do the "correct thing" and use a non-blocking socket together with the `polling` library.
        listener_socket.set_nonblocking(true)?;

        let listener_poll_key = 1;
        let poller = Arc::new(Poller::new()?);
        poller.add(&listener_socket, Event::readable(listener_poll_key))?;

        let message_broadcaster =
            Arc::new(ReceiveSetBroadcaster::new(rerun_rx, server_memory_limit));
        let shutdown_flag = Arc::new(AtomicBool::new(false));

        let local_addr = listener_socket.local_addr()?;
        let poller_copy = poller.clone();
        let shutdown_flag_copy = shutdown_flag.clone();

        let listener_join_handle = std::thread::Builder::new()
            .name("rerun_ws_server: listener".to_owned())
            .spawn(move || {
                Self::listen_thread_func(
                    &poller,
                    &listener_socket,
                    listener_poll_key,
                    &message_broadcaster,
                    &shutdown_flag,
                );
            })?;

        let slf = Self {
            local_addr,
            poller: poller_copy,
            listener_join_handle: Some(listener_join_handle),
            shutdown_flag: shutdown_flag_copy,
        };

        re_log::info!(
            "Hosting a WebSocket server on {wsurl}. You can connect to this with a native viewer (`rerun {wsurl}`) or the web viewer (with `?url={wsurl}`).",
            wsurl=slf.server_url()
        );

        Ok(slf)
    }

    /// Contains the `ws://` or `wss://` prefix.
    pub fn server_url(&self) -> String {
        server_url(&self.local_addr)
    }

    fn listen_thread_func(
        poller: &Poller,
        listener_socket: &TcpListener,
        listener_poll_key: usize,
        message_broadcaster: &Arc<ReceiveSetBroadcaster>,
        shutdown_flag: &AtomicBool,
    ) {
        let mut events = Vec::new();
        loop {
            if let Err(err) = poller.wait(&mut events, None) {
                re_log::warn!("Error polling WebSocket server listener: {err}");
            }

            if shutdown_flag.load(std::sync::atomic::Ordering::Acquire) {
                re_log::debug!("Stopping WebSocket server listener.");
                break;
            }

            for event in events.drain(..) {
                if event.key == listener_poll_key {
                    let (tcp_stream, _) = match listener_socket.accept() {
                        Ok(connection) => connection,

                        Err(err) => {
                            re_log::warn!("Error accepting WebSocket connection: {err}");
                            continue;
                        }
                    };
                    Self::handle_connection(message_broadcaster.clone(), tcp_stream);
                }
            }
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
                        if let Err(err) = ws_stream.send(tungstenite::Message::Binary(msg.to_vec()))
                        {
                            re_log::warn!("Error sending message to WebSocket client: {err}");
                            break;
                        }
                    }

                    message_broadcaster.remove_client(client_id);
                }

                re_log::debug!("Closing WebSocket connection at {:?}", address);
            })
        {
            re_log::error!(
                "Failed to spawn thread for handling incoming WebSocket connection: {err}"
            );
        }
    }

    fn stop_listener(&mut self) {
        let Some(join_handle) = self.listener_join_handle.take() else {
            return;
        };

        self.shutdown_flag
            .store(true, std::sync::atomic::Ordering::Release);

        if let Err(err) = self.poller.notify() {
            re_log::warn!("Error notifying WebSocket server listener: {err}");
            return;
        }

        if let Err(err) = join_handle.join() {
            re_log::warn!("Error joining listener thread: {err:?}");
        }
    }
}

impl Drop for RerunServer {
    fn drop(&mut self) {
        re_log::info!("Shutting down Rerun server on {}", self.server_url());
        self.stop_listener();
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
