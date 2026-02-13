//! An in-memory channel of Rerun data messages

use std::sync::Arc;

pub use crossbeam::channel::{RecvError, RecvTimeoutError, SendError, TryRecvError};
use parking_lot::RwLock;
use re_uri::RedapUri;

mod data_source_message;
mod receiver;
mod receiver_set;
mod sender;

pub use self::data_source_message::{DataSourceMessage, DataSourceUiCommand};
pub use self::receiver::LogReceiver;
pub use self::receiver_set::LogReceiverSet;
pub use self::sender::LogSender;

// --- Source ---

/// An error that can occur when flushing.
#[derive(Debug, thiserror::Error)]
pub enum FlushError {
    #[error("Received closed before flushing completed")]
    Closed,

    #[error("Flush timed out - not all messages were sent.")]
    Timeout,
}

/// Identifies in what context this smart channel was created,
/// and what is holding the [`LogSender`].
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub enum LogSource {
    /// The sender is a background thread reading data from a file on disk
    /// (could be `.rrd` files, or `.glb`, `.png`, …).
    File(std::path::PathBuf),

    /// The sender is a background thread fetching data from an HTTP file server.
    RrdHttpStream {
        /// Should include `http(s)://` prefix.
        url: String,

        /// Indicates whether the viewer should open the stream in `Following` mode rather than `Playing` mode.
        // TODO(andreas): having follow in here is a bit weird. This should be part of the link fragments instead.
        follow: bool,
    },

    /// The channel was created in the context of loading an `.rrd` file from a `postMessage`
    /// javascript event.
    ///
    /// Only applicable to web browser iframes.
    /// Used for the inline web viewer in a notebook.
    RrdWebEvent,

    /// The channel was created in the context of a javascript client submitting an RRD directly as bytes.
    JsChannel {
        /// The name of the channel reported by the javascript client.
        channel_name: String,
    },

    /// The sender is a Rerun SDK running from another thread in the same process.
    Sdk,

    /// The data is streaming in from standard input.
    Stdin,

    /// The data is streaming in directly from a Rerun Data Platform server,
    /// over `rerun://` gRPC interface.
    RedapGrpcStream {
        uri: re_uri::DatasetSegmentUri,

        /// Switch to this recording once it has been loaded?
        select_when_loaded: bool,
    },

    /// The data is streaming in via a message proxy.
    MessageProxy(re_uri::ProxyUri),
}

impl std::fmt::Display for LogSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File(path) => write!(f, "file://{}", path.to_string_lossy()),
            Self::RrdHttpStream { url, follow: _ } => url.fmt(f),
            Self::MessageProxy(uri) => uri.fmt(f),
            Self::RedapGrpcStream { uri, .. } => uri.fmt(f),
            Self::RrdWebEvent => "Web event listener".fmt(f),
            Self::JsChannel { channel_name } => write!(f, "Javascript channel: {channel_name}"),
            Self::Sdk => "SDK".fmt(f),
            Self::Stdin => "stdin".fmt(f),
        }
    }
}

impl LogSource {
    pub fn is_redap(&self) -> bool {
        matches!(self, Self::RedapGrpcStream { .. })
    }

    pub fn is_network(&self) -> bool {
        match self {
            Self::File(_) | Self::Sdk | Self::RrdWebEvent | Self::Stdin => false,
            Self::RrdHttpStream { .. }
            | Self::JsChannel { .. }
            | Self::RedapGrpcStream { .. }
            | Self::MessageProxy { .. } => true,
        }
    }

    pub fn select_when_loaded(&self) -> bool {
        match self {
            Self::File(_)
            | Self::Sdk
            | Self::RrdWebEvent
            | Self::Stdin
            | Self::RrdHttpStream { .. }
            | Self::JsChannel { .. }
            | Self::MessageProxy { .. } => true,

            Self::RedapGrpcStream {
                select_when_loaded, ..
            } => *select_when_loaded,
        }
    }

    pub fn redap_uri(&self) -> Option<RedapUri> {
        match self {
            Self::RedapGrpcStream { uri, .. } => Some(RedapUri::DatasetData(uri.clone())),
            Self::MessageProxy(uri) => Some(RedapUri::Proxy(uri.clone())),

            Self::File(_)
            | Self::Sdk
            | Self::RrdWebEvent
            | Self::Stdin
            | Self::RrdHttpStream { .. }
            | Self::JsChannel { .. } => None,
        }
    }

    /// Same as [`Self::redap_uri`], but strips any extra query or fragment from the uri.
    pub fn stripped_redap_uri(&self) -> Option<RedapUri> {
        self.redap_uri().map(|uri| match uri {
            RedapUri::Catalog(_) | RedapUri::Entry(_) | RedapUri::Proxy(_) => uri,
            RedapUri::DatasetData(uri) => RedapUri::DatasetData(uri.without_query_and_fragment()),
        })
    }

    /// Loading text for sources that load data from a specific source (e.g. a file or a URL).
    ///
    /// Returns `None` for any source that receives data dynamically through SDK calls or similar.
    /// For a status string that applies to all sources, see [`Self::status_string`].
    pub fn loading_name(&self) -> Option<String> {
        match self {
            // We only show things we know are very-soon-to-be recordings:
            Self::File(path) => Some(path.to_string_lossy().into_owned()),
            Self::RrdHttpStream { url, .. } => Some(url.clone()),
            Self::RedapGrpcStream { uri, .. } => Some(uri.segment_id.clone()),

            Self::RrdWebEvent
            | Self::JsChannel { .. }
            | Self::MessageProxy { .. }
            | Self::Sdk
            | Self::Stdin => {
                // For all of these sources we're not actively loading data, but rather waiting for data to be sent.
                // These show up in the top panel - see `top_panel.rs`.
                None
            }
        }
    }

    /// Status string describing waiting or loading status for a source.
    pub fn status_string(&self) -> String {
        match self {
            Self::File(path) => {
                format!("Loading {}…", path.display())
            }
            Self::Stdin => "Loading stdin…".to_owned(),
            Self::RrdHttpStream { url, .. } => {
                format!("Waiting for data on {url}…")
            }
            Self::MessageProxy(uri) => {
                format!("Waiting for data on {uri}…")
            }
            Self::RedapGrpcStream { uri, .. } => {
                format!(
                    "Waiting for data on {}…",
                    uri.clone().without_query_and_fragment()
                )
            }
            Self::RrdWebEvent | Self::JsChannel { .. } => "Waiting for logging data…".to_owned(),
            Self::Sdk => "Waiting for logging data from SDK".to_owned(),
        }
    }

    /// Compares two channel sources but ignores any URI fragments and other selection/state only guides
    /// that don't affect what data is loaded.
    pub fn is_same_ignoring_uri_fragments(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::RedapGrpcStream { uri: uri1, .. }, Self::RedapGrpcStream { uri: uri2, .. }) => {
                uri1.clone().without_fragment() == uri2.clone().without_fragment()
            }
            (Self::RrdHttpStream { url: url1, .. }, Self::RrdHttpStream { url: url2, .. }) => {
                url1 == url2
            }
            _ => self == other,
        }
    }
}

// -------------------------------------------------------------------------------------

/// Shared by all receivers and senders of a channel
#[derive(Default)]
pub(crate) struct Channel {
    /// The sender should call this every time a message is sent.
    ///
    /// This can be used to wake up the receiver thread.
    waker: RwLock<Option<Box<dyn Fn() + Send + Sync + 'static>>>,
}

/// Create a new communication channel for [`DataSourceMessage`].
pub fn log_channel(source: LogSource) -> (LogSender, LogReceiver) {
    let max_bytes_on_wire = 128 * 1024 * 1024; // TODO(emilk): make configurable

    let source = Arc::new(source);
    let channel = Arc::new(Channel::default());
    let (tx, rx) = re_quota_channel::channel(format!("log_channel({source})"), max_bytes_on_wire);
    let sender = LogSender::new(tx, source.clone(), channel.clone());
    let receiver = LogReceiver::new(rx, channel, source);
    (sender, receiver)
}

// ---

/// The payload of a [`SmartMessage`].
///
/// Either data or an end-of-stream marker.
pub enum SmartMessagePayload {
    /// A message sent down the channel.
    Msg(DataSourceMessage),

    /// When received, flush anything already received and then call the given callback.
    Flush {
        on_flush_done: Box<dyn FnOnce() + Send>,
    },

    /// The [`LogSender`] has quit.
    ///
    /// `None` indicates the sender left gracefully, an error indicates otherwise.
    Quit(Option<Box<dyn std::error::Error + Send>>),
}

impl std::fmt::Debug for SmartMessagePayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Msg(_) => f.write_str("Msg(_)"),
            Self::Flush { .. } => f.write_str("Flush"),
            Self::Quit(_) => f.write_str("Quit"),
        }
    }
}

#[derive(Debug)]
pub struct SmartMessage {
    pub source: Arc<LogSource>,
    pub payload: SmartMessagePayload,
}

impl SmartMessage {
    pub fn data(&self) -> Option<&DataSourceMessage> {
        match &self.payload {
            SmartMessagePayload::Msg(msg) => Some(msg),
            SmartMessagePayload::Flush { .. } | SmartMessagePayload::Quit(_) => None,
        }
    }

    pub fn into_data(self) -> Option<DataSourceMessage> {
        match self.payload {
            SmartMessagePayload::Msg(msg) => Some(msg),
            SmartMessagePayload::Flush { .. } | SmartMessagePayload::Quit(_) => None,
        }
    }
}

impl re_byte_size::SizeBytes for SmartMessage {
    fn heap_size_bytes(&self) -> u64 {
        let Self { source: _, payload } = self;
        match payload {
            SmartMessagePayload::Msg(msg) => msg.heap_size_bytes(),
            SmartMessagePayload::Flush { .. } | SmartMessagePayload::Quit(..) => 0,
        }
    }
}
