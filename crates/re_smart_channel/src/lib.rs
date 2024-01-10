//! A channel that keeps track of latency and queue length.

use std::sync::{atomic::AtomicU64, Arc};

use web_time::Instant;

pub use crossbeam::channel::{RecvError, RecvTimeoutError, SendError, TryRecvError};

mod receive_set;
mod receiver;
mod sender;

pub use receive_set::ReceiveSet;
pub use receiver::Receiver;
pub use sender::Sender;

// --- Source ---

/// Identifies in what context this smart channel was created, and who/what is holding its
/// receiving end.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SmartChannelSource {
    /// The channel was created in the context of loading a file from disk (could be
    /// `.rrd` files, or `.glb`, `.png`, …).
    File(std::path::PathBuf),

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

    /// The channel was created in the context of streaming in RRD data from standard input.
    Stdin,
}

impl std::fmt::Display for SmartChannelSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File(path) => path.display().fmt(f),
            Self::RrdHttpStream { url } => url.fmt(f),
            Self::RrdWebEventListener => "Web Event Listener".fmt(f),
            Self::Sdk => "SDK".fmt(f),
            Self::WsClient { ws_server_url } => ws_server_url.fmt(f),
            Self::TcpServer { port } => write!(f, "TCP Server, port {port}"),
            Self::Stdin => "Standard Input".fmt(f),
        }
    }
}

impl SmartChannelSource {
    pub fn is_network(&self) -> bool {
        match self {
            Self::File(_) | Self::Sdk | Self::RrdWebEventListener | Self::Stdin => false,
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

    /// The data is streaming in from standard input.
    Stdin,
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
            SmartMessageSource::Stdin => "stdin".into(),
        })
    }
}

// ---

/// Stats for a channel, possibly shared between chained channels.
#[derive(Default)]
pub(crate) struct SharedStats {
    /// Latest known latency from sending a message to receiving it, it nanoseconds.
    latency_ns: AtomicU64,
}

pub fn smart_channel<T: Send>(
    sender_source: SmartMessageSource,
    source: SmartChannelSource,
) -> (Sender<T>, Receiver<T>) {
    let stats = Arc::new(SharedStats::default());
    smart_channel_with_stats(sender_source, Arc::new(source), stats)
}

/// Create a new channel using the same stats as some other.
///
/// This is a very leaky abstraction, and it would be nice to refactor some day
pub(crate) fn smart_channel_with_stats<T: Send>(
    sender_source: SmartMessageSource,
    source: Arc<SmartChannelSource>,
    stats: Arc<SharedStats>,
) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = crossbeam::channel::unbounded();
    let sender_source = Arc::new(sender_source);
    let sender = Sender::new(tx, sender_source, stats.clone());
    let receiver = Receiver::new(rx, stats, source);
    (sender, receiver)
}

// ---

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
