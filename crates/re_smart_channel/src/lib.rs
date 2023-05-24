//! A channel that keeps track of latency and queue length.

use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering::Relaxed},
    Arc,
};

use web_time::Instant;

pub use crossbeam::channel::{RecvError, RecvTimeoutError, SendError, TryRecvError};

// --- Source ---

/// Identifies in what context this smart channel was created, and who/what is holding its
/// receiving end.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SmartChannelSource {
    /// The channel was created in the context of loading a bunch of files from disk (could be
    /// `.rrd` files, or `.glb`, `.png`, …).
    // TODO(#2121): Remove this
    Files { paths: Vec<std::path::PathBuf> },

    /// The channel was created in the context of loading an `.rrd` file over http.
    RrdHttpStream { url: String },

    /// The channel was created in the context of loading an `.rrd` file from a `postMessage`
    /// js event.
    ///
    /// Only applicable to web browser iframes.
    RrdWebEventListener,

    /// The channel was created in the context of loading data using a Rerun SDK sharing the same
    /// process.
    Sdk,

    /// The channel was created in the context of fetching data from a Rerun WebSocket server.
    ///
    /// We are likely running in a web browser.
    WsClient {
        /// The server we are connected to (or are trying to connect to)
        ws_server_url: String,
    },

    /// The channel was created in the context of receiving data from one or more Rerun SDKs
    /// over TCP.
    ///
    /// We are a TCP server listening on this port.
    TcpServer { port: u16 },
}

impl SmartChannelSource {
    pub fn is_network(&self) -> bool {
        match self {
            Self::Files { .. } | Self::Sdk | Self::RrdWebEventListener => false,
            Self::RrdHttpStream { .. } | Self::WsClient { .. } | Self::TcpServer { .. } => true,
        }
    }
}

/// Identifies who/what sent a particular message in a smart channel.
///
/// Due to the multiplexed nature of the smart channel, every message coming in can originate
/// from a different source.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SmartMessageSource {
    /// The source is unknown.
    ///
    /// This is only used when we need to allocate a sender but cannot yet know what that the
    /// source is.
    /// This should never be used to send a message; use [`Sender::clone_as`] to specify the source
    /// of a [`Sender`] after its creation.
    Unknown,

    /// The sender is a background thread reading data from a file on disk.
    File(std::path::PathBuf),

    /// The sender is a background thread fetching data from an HTTP file server.
    RrdHttpStream { url: String },

    /// The sender is a javascript callback triggered by a `postMessage` event.
    ///
    /// Only applicable to web browser iframes.
    RrdWebEventCallback,

    /// The sender is a Rerun SDK running from another thread in the same process.
    Sdk,

    /// The sender is a WebSocket client fetching data from a Rerun WebSocket server.
    ///
    /// We are likely running in a web browser.
    WsClient {
        /// The server we are connected to (or are trying to connect to)
        ws_server_url: String,
    },

    /// The sender is a TCP client.
    TcpClient {
        // NOTE: Optional as we might not be able to retrieve the peer's address for some obscure
        // reason.
        addr: Option<std::net::SocketAddr>,
    },
}

impl std::fmt::Display for SmartMessageSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&match self {
            SmartMessageSource::Unknown => "unknown".into(),
            SmartMessageSource::File(path) => format!("file://{}", path.to_string_lossy()),
            SmartMessageSource::RrdHttpStream { url } => format!("http://{url}"),
            SmartMessageSource::RrdWebEventCallback => "web_callback".into(),
            SmartMessageSource::Sdk => "sdk".into(),
            SmartMessageSource::WsClient { ws_server_url } => ws_server_url.clone(),
            SmartMessageSource::TcpClient { addr } => format!(
                "tcp://{}",
                addr.map_or_else(|| "(unknown ip)".to_owned(), |addr| addr.to_string())
            ),
        })
    }
}

// ---

/// Stats for a channel, possibly shared between chained channels.
#[derive(Default)]
struct SharedStats {
    /// Latest known latency from sending a message to receiving it, it nanoseconds.
    latency_ns: AtomicU64,
}

pub fn smart_channel<T: Send>(
    sender_source: SmartMessageSource,
    source: SmartChannelSource,
) -> (Sender<T>, Receiver<T>) {
    let stats = Arc::new(SharedStats::default());
    smart_channel_with_stats(sender_source, source, stats)
}

/// Create a new channel using the same stats as some other.
///
/// This is a very leaky abstraction, and it would be nice to refactor some day
fn smart_channel_with_stats<T: Send>(
    sender_source: SmartMessageSource,
    source: SmartChannelSource,
    stats: Arc<SharedStats>,
) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = crossbeam::channel::unbounded();
    let sender_source = Arc::new(sender_source);
    let sender = Sender {
        tx,
        source: sender_source,
        stats: stats.clone(),
    };
    let receiver = Receiver {
        rx,
        stats,
        source,
        connected: AtomicBool::new(true),
    };
    (sender, receiver)
}

// ---

#[derive(Clone)]
pub struct Sender<T: Send> {
    tx: crossbeam::channel::Sender<SmartMessage<T>>,
    source: Arc<SmartMessageSource>,
    stats: Arc<SharedStats>,
}

impl<T: Send> Sender<T> {
    /// Clones the sender with an updated source.
    pub fn clone_as(&self, source: SmartMessageSource) -> Self {
        Self {
            tx: self.tx.clone(),
            source: Arc::new(source),
            stats: Arc::clone(&self.stats),
        }
    }

    pub fn send(&self, msg: T) -> Result<(), SendError<T>> {
        self.send_at(
            Instant::now(),
            Arc::clone(&self.source),
            SmartMessagePayload::Msg(msg),
        )
    }

    /// Forwards a message as-is.
    pub fn send_at(
        &self,
        time: Instant,
        source: Arc<SmartMessageSource>,
        payload: SmartMessagePayload<T>,
    ) -> Result<(), SendError<T>> {
        // NOTE: We should never be sending a message with an unknown source.
        debug_assert!(!matches!(*source, SmartMessageSource::Unknown));

        self.tx
            .send(SmartMessage {
                time,
                source,
                payload,
            })
            .map_err(|SendError(msg)| SendError(msg.into_data().unwrap()))
    }

    /// Used to indicate that a sender has left.
    ///
    /// This sends a message down the channel allowing the receiving end to know whether one of the
    /// sender has left, and if so why (if applicable).
    ///
    /// Using a [`Sender`] after calling `quit` is undefined behaviour: the receiving end is free
    /// to silently drop those messages (or worse).
    pub fn quit(
        &self,
        err: Option<Box<dyn std::error::Error + Send>>,
    ) -> Result<(), SendError<SmartMessage<T>>> {
        // NOTE: We should never be sending a message with an unknown source.
        debug_assert!(!matches!(*self.source, SmartMessageSource::Unknown));

        self.tx.send(SmartMessage {
            time: Instant::now(),
            source: Arc::clone(&self.source),
            payload: SmartMessagePayload::Quit(err),
        })
    }

    /// Is the channel currently empty of messages?
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tx.is_empty()
    }

    /// Number of messages in the channel right now.
    #[inline]
    pub fn len(&self) -> usize {
        self.tx.len()
    }

    /// Latest known latency from sending a message to receiving it, it nanoseconds.
    pub fn latency_ns(&self) -> u64 {
        self.stats.latency_ns.load(Relaxed)
    }

    /// Latest known latency from sending a message to receiving it,
    /// in seconds
    pub fn latency_sec(&self) -> f32 {
        self.latency_ns() as f32 / 1e9
    }
}

/// The payload of a [`SmartMessage`].
///
/// Either data or an end-of-stream marker.
#[derive(Debug)]
pub enum SmartMessagePayload<T: Send> {
    /// A message sent down the channel.
    Msg(T),

    /// The [`Sender`] has quit.
    ///
    /// `None` indicates the sender left gracefully, an error indicates otherwise.
    Quit(Option<Box<dyn std::error::Error + Send>>),
}

impl<T: Send + PartialEq> PartialEq for SmartMessagePayload<T> {
    fn eq(&self, rhs: &Self) -> bool {
        match (self, rhs) {
            (SmartMessagePayload::Msg(msg1), SmartMessagePayload::Msg(msg2)) => msg1.eq(msg2),
            _ => false,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct SmartMessage<T: Send> {
    pub time: Instant,
    pub source: Arc<SmartMessageSource>,
    pub payload: SmartMessagePayload<T>,
}

impl<T: Send> SmartMessage<T> {
    pub fn data(&self) -> Option<&T> {
        use SmartMessagePayload::{Msg, Quit};
        match &self.payload {
            Msg(msg) => Some(msg),
            Quit(_) => None,
        }
    }

    pub fn into_data(self) -> Option<T> {
        use SmartMessagePayload::{Msg, Quit};
        match self.payload {
            Msg(msg) => Some(msg),
            Quit(_) => None,
        }
    }
}

pub struct Receiver<T: Send> {
    rx: crossbeam::channel::Receiver<SmartMessage<T>>,
    stats: Arc<SharedStats>,
    source: SmartChannelSource,
    connected: AtomicBool,
}

impl<T: Send> Receiver<T> {
    /// Are we still connected?
    ///
    /// Once false, we will never be connected again: the source has run dry.
    ///
    /// This is only updated once one of the receive methods fails.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Relaxed)
    }

    pub fn recv(&self) -> Result<SmartMessage<T>, RecvError> {
        let msg = match self.rx.recv() {
            Ok(x) => x,
            Err(RecvError) => {
                self.connected.store(false, Relaxed);
                return Err(RecvError);
            }
        };

        let latency_ns = msg.time.elapsed().as_nanos() as u64;
        self.stats.latency_ns.store(latency_ns, Relaxed);

        Ok(msg)
    }

    pub fn try_recv(&self) -> Result<SmartMessage<T>, TryRecvError> {
        let msg = match self.rx.try_recv() {
            Ok(x) => x,
            Err(err) => {
                if err == TryRecvError::Disconnected {
                    self.connected.store(false, Relaxed);
                }
                return Err(err);
            }
        };

        let latency_ns = msg.time.elapsed().as_nanos() as u64;
        self.stats.latency_ns.store(latency_ns, Relaxed);

        Ok(msg)
    }

    pub fn recv_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<SmartMessage<T>, RecvTimeoutError> {
        let msg = match self.rx.recv_timeout(timeout) {
            Ok(x) => x,
            Err(err) => {
                if err == RecvTimeoutError::Disconnected {
                    self.connected.store(false, Relaxed);
                }
                return Err(err);
            }
        };

        let latency_ns = msg.time.elapsed().as_nanos() as u64;
        self.stats.latency_ns.store(latency_ns, Relaxed);

        Ok(msg)
    }

    /// Receives without registering the latency.
    ///
    /// This is for use with [`Sender::send_at`] when chaining to another channel
    /// created with [`Self::chained_channel`].
    pub fn recv_with_send_time(&self) -> Result<SmartMessage<T>, RecvError> {
        self.rx.recv()
    }

    /// Where is the data coming from?
    #[inline]
    pub fn source(&self) -> &SmartChannelSource {
        &self.source
    }

    /// Is the channel currently empty of messages?
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rx.is_empty()
    }

    /// Number of messages in the channel right now.
    #[inline]
    pub fn len(&self) -> usize {
        self.rx.len()
    }

    /// Latest known latency from sending a message to receiving it, it nanoseconds.
    pub fn latency_ns(&self) -> u64 {
        self.stats.latency_ns.load(Relaxed)
    }

    /// Latest known latency from sending a message to receiving it,
    /// in seconds
    pub fn latency_sec(&self) -> f32 {
        self.latency_ns() as f32 / 1e9
    }

    /// Create a new channel that use the same stats as this one.
    ///
    /// This means both channels will see the same latency numbers.
    ///
    /// Care must be taken to use [`Self::recv_with_send_time`] and [`Sender::send_at`].
    /// This is a very leaky abstraction, and it would be nice with a refactor.
    pub fn chained_channel(&self) -> (Sender<T>, Receiver<T>) {
        smart_channel_with_stats(
            // NOTE: We cannot know yet, and it doesn't matter as the new sender will only be used
            // to forward existing messages.
            SmartMessageSource::Unknown,
            self.source.clone(),
            self.stats.clone(),
        )
    }
}

// ---

#[test]
fn test_smart_channel() {
    let (tx, rx) = smart_channel(SmartMessageSource::Sdk, SmartChannelSource::Sdk); // whatever source

    assert_eq!(tx.len(), 0);
    assert_eq!(rx.len(), 0);
    assert_eq!(tx.latency_ns(), 0);

    tx.send(42).unwrap();

    assert_eq!(tx.len(), 1);
    assert_eq!(rx.len(), 1);
    assert_eq!(tx.latency_ns(), 0);

    std::thread::sleep(std::time::Duration::from_millis(10));

    assert_eq!(rx.recv().map(|msg| msg.into_data()), Ok(Some(42)));

    assert_eq!(tx.len(), 0);
    assert_eq!(rx.len(), 0);
    assert!(tx.latency_ns() > 1_000_000);
}

#[test]
fn test_smart_channel_connected() {
    let (tx1, rx) = smart_channel(SmartMessageSource::Sdk, SmartChannelSource::Sdk); // whatever source
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    assert!(rx.is_connected());

    let tx2 = tx1.clone();
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    assert!(rx.is_connected());

    tx2.send(42).unwrap();
    assert_eq!(rx.try_recv().map(|msg| msg.into_data()), Ok(Some(42)));
    assert!(rx.is_connected());

    drop(tx1);
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    assert!(rx.is_connected());

    drop(tx2);
    assert_eq!(rx.try_recv(), Err(TryRecvError::Disconnected));
    assert!(!rx.is_connected());
}
