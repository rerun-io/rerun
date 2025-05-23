//! A channel that keeps track of latency and queue length.

use std::sync::{Arc, atomic::AtomicU64};

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
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), expect(clippy::large_enum_variant))]
pub enum SmartChannelSource {
    /// The channel was created in the context of loading a file from disk (could be
    /// `.rrd` files, or `.glb`, `.png`, …).
    File(std::path::PathBuf),

    /// The channel was created in the context of loading an `.rrd` file over http.
    ///
    /// The `follow` flag indicates whether the viewer should open the stream in `Following` mode rather than `Playing` mode.
    RrdHttpStream { url: String, follow: bool },

    /// The channel was created in the context of loading an `.rrd` file from a `postMessage`
    /// js event.
    ///
    /// Only applicable to web browser iframes.
    /// Used for the inline web viewer in a notebook.
    RrdWebEventListener,

    /// The channel was created in the context of a javascript client submitting an RRD directly as bytes.
    JsChannel {
        /// The name of the channel reported by the javascript client.
        channel_name: String,
    },

    /// The channel was created in the context of loading data using a Rerun SDK sharing the same
    /// process.
    Sdk,

    /// The channel was created in the context of streaming in RRD data from standard input.
    Stdin,

    /// The data is streaming in directly from a Rerun Data Platform server, over gRPC.
    RedapGrpcStream {
        uri: re_uri::DatasetDataUri,
        token: Option<re_auth::Jwt>,
    },

    /// The data is streaming in via a message proxy.
    MessageProxy(re_uri::ProxyUri),
}

impl std::fmt::Display for SmartChannelSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File(path) => path.display().fmt(f),
            Self::RrdHttpStream { url, follow: _ } => url.fmt(f),
            Self::MessageProxy(uri) => uri.fmt(f),
            Self::RedapGrpcStream { uri, token: _ } => uri.fmt(f),
            Self::RrdWebEventListener => "Web event listener".fmt(f),
            Self::JsChannel { channel_name } => write!(f, "Javascript channel: {channel_name}"),
            Self::Sdk => "SDK".fmt(f),
            Self::Stdin => "Standard input".fmt(f),
        }
    }
}

impl SmartChannelSource {
    pub fn is_network(&self) -> bool {
        match self {
            Self::File(_) | Self::Sdk | Self::RrdWebEventListener | Self::Stdin => false,
            Self::RrdHttpStream { .. }
            | Self::JsChannel { .. }
            | Self::RedapGrpcStream { .. }
            | Self::MessageProxy { .. } => true,
        }
    }
}

/// Identifies who/what sent a particular message in a smart channel.
///
/// Due to the multiplexed nature of the smart channel, every message coming in can originate
/// from a different source.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(not(target_arch = "wasm32"), expect(clippy::large_enum_variant))]
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
    RrdHttpStream {
        /// Should include `http(s)://` prefix.
        url: String,
    },

    /// The sender is a javascript callback triggered by a `postMessage` event.
    ///
    /// Only applicable to web browser iframes.
    RrdWebEventCallback,

    /// The sender is a javascript client submitting an RRD directly as bytes.
    JsChannelPush,

    /// The sender is a Rerun SDK running from another thread in the same process.
    Sdk,

    /// The data is streaming in from standard input.
    Stdin,

    /// A file on a Rerun Data Platform server, over `rerun://` gRPC interface.
    RedapGrpcStream {
        uri: re_uri::DatasetDataUri,
        token: Option<re_auth::Jwt>,
    },

    /// A stream of messages over message proxy gRPC interface.
    MessageProxy(re_uri::ProxyUri),
}

impl std::fmt::Display for SmartMessageSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&match self {
            Self::Unknown => "unknown".into(),
            Self::File(path) => format!("file://{}", path.to_string_lossy()),
            Self::RrdHttpStream { url } => url.clone(),
            Self::MessageProxy(uri) => uri.to_string(),
            Self::RedapGrpcStream { uri, token: _ } => uri.to_string(),
            Self::RrdWebEventCallback => "web_callback".into(),
            Self::JsChannelPush => "javascript".into(),
            Self::Sdk => "sdk".into(),
            Self::Stdin => "stdin".into(),
        })
    }
}

// ---

/// Stats for a channel, possibly shared between chained channels.
#[derive(Default)]
pub(crate) struct SharedStats {
    /// Latest known latency from sending a message to receiving it, it nanoseconds.
    latency_nanos: AtomicU64,
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
pub enum SmartMessagePayload<T: Send> {
    /// A message sent down the channel.
    Msg(T),

    /// When received, flush anything already received and then call the given callback.
    Flush {
        on_flush_done: Box<dyn FnOnce() + Send>,
    },

    /// The [`Sender`] has quit.
    ///
    /// `None` indicates the sender left gracefully, an error indicates otherwise.
    Quit(Option<Box<dyn std::error::Error + Send>>),
}

impl<T: Send> std::fmt::Debug for SmartMessagePayload<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Msg(_) => f.write_str("Msg(_)"),
            Self::Flush { .. } => f.write_str("Flush"),
            Self::Quit(_) => f.write_str("Quit"),
        }
    }
}

impl<T: Send + PartialEq> PartialEq for SmartMessagePayload<T> {
    fn eq(&self, rhs: &Self) -> bool {
        match (self, rhs) {
            (Self::Msg(msg1), Self::Msg(msg2)) => msg1.eq(msg2),
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
        match &self.payload {
            SmartMessagePayload::Msg(msg) => Some(msg),
            SmartMessagePayload::Flush { .. } | SmartMessagePayload::Quit(_) => None,
        }
    }

    pub fn into_data(self) -> Option<T> {
        match self.payload {
            SmartMessagePayload::Msg(msg) => Some(msg),
            SmartMessagePayload::Flush { .. } | SmartMessagePayload::Quit(_) => None,
        }
    }
}

// ---

#[test]
fn test_smart_channel() {
    let (tx, rx) = smart_channel(SmartMessageSource::Sdk, SmartChannelSource::Sdk); // whatever source

    assert_eq!(tx.len(), 0);
    assert_eq!(rx.len(), 0);
    assert_eq!(tx.latency_nanos(), 0);

    tx.send(42).unwrap();

    assert_eq!(tx.len(), 1);
    assert_eq!(rx.len(), 1);
    assert_eq!(tx.latency_nanos(), 0);

    std::thread::sleep(std::time::Duration::from_millis(10));

    assert_eq!(rx.recv().map(|msg| msg.into_data()), Ok(Some(42)));

    assert_eq!(tx.len(), 0);
    assert_eq!(rx.len(), 0);
    assert!(tx.latency_nanos() > 1_000_000);
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
