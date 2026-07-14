//! An in-memory channel of Rerun data messages

use std::sync::Arc;

pub use crossbeam::channel::{RecvError, RecvTimeoutError, SendError, TryRecvError};
use parking_lot::RwLock;
use re_log_types::{StoreId, TableId};
use re_uri::RedapUri;

mod data_source_message;
mod receiver;
mod receiver_set;
mod sender;

pub use self::data_source_message::{
    DataSourceMessage, DataSourceUiCommand, InspectError, SaveScreenshotError,
};
pub use self::receiver::LogReceiver;
pub use self::receiver_set::LogReceiverSet;
pub use self::sender::LogSender;

// --- Source ---

/// Controls how a newly loaded recording is treated by the viewer.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub enum RecordingOpenBehavior {
    /// Load without affecting the recording panel.
    ///
    /// Used for preview views.
    Background,

    /// Mark as opened in the recording panel, but don't navigate to it.
    Open,

    /// Mark as opened and make it the active recording.
    OpenAndSelect,
}

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
#[cfg_attr(not(target_arch = "wasm32"), expect(clippy::large_enum_variant))]
pub enum LogSource {
    /// The sender is a background thread reading data from a file on disk
    /// (could be `.rrd` files, or `.glb`, `.png`, …).
    File {
        path: std::path::PathBuf,

        /// If `true`, the viewer should start in `Following` mode.
        follow: bool,
    },

    /// The sender is a background thread fetching data from an HTTP file server.
    #[serde(alias = "RrdHttpStream")]
    HttpStream {
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

    /// The data is streaming in directly from a catalog server,
    /// over `rerun://` gRPC interface.
    RedapGrpcStream {
        uri: re_uri::DatasetSegmentUri,

        open_behavior: RecordingOpenBehavior,

        /// If set, this source is streaming a blueprint that should be associated with a table
        /// once the stream completes successfully.
        #[serde(default)]
        table_blueprint: Option<TableBlueprintSource>,
    },

    /// The data is streaming in via a message proxy.
    MessageProxy(re_uri::ProxyUri),
}

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub struct TableBlueprintSource {
    pub table_id: TableId,
    pub blueprint_id: StoreId,
}

impl std::fmt::Display for LogSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File { path, .. } => write!(f, "file://{}", path.to_string_lossy()),
            Self::HttpStream { url, follow: _ } => url_display_name(url).fmt(f),
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
            Self::File { .. } | Self::Sdk | Self::RrdWebEvent | Self::Stdin => false,
            Self::HttpStream { .. }
            | Self::JsChannel { .. }
            | Self::RedapGrpcStream { .. }
            | Self::MessageProxy { .. } => true,
        }
    }

    pub fn open_behavior(&self) -> RecordingOpenBehavior {
        match self {
            Self::File { .. }
            | Self::Sdk
            | Self::RrdWebEvent
            | Self::Stdin
            | Self::HttpStream { .. }
            | Self::JsChannel { .. }
            | Self::MessageProxy { .. } => RecordingOpenBehavior::OpenAndSelect,

            Self::RedapGrpcStream { open_behavior, .. } => *open_behavior,
        }
    }

    pub fn redap_uri(&self) -> Option<RedapUri> {
        match self {
            Self::RedapGrpcStream { uri, .. } => Some(RedapUri::DatasetData(uri.clone())),
            Self::MessageProxy(uri) => Some(RedapUri::Proxy(uri.clone())),

            Self::File { .. }
            | Self::Sdk
            | Self::RrdWebEvent
            | Self::Stdin
            | Self::HttpStream { .. }
            | Self::JsChannel { .. } => None,
        }
    }

    /// Same as [`Self::redap_uri`], but strips any extra query or fragment from the uri.
    pub fn stripped_redap_uri(&self) -> Option<RedapUri> {
        self.redap_uri().map(|uri| match uri {
            RedapUri::Catalog(_)
            | RedapUri::Entry(_)
            | RedapUri::Folder(_)
            | RedapUri::Proxy(_) => uri,
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
            Self::File { path, .. } => Some(path.to_string_lossy().into_owned()),
            Self::HttpStream { url, .. } => Some(url_display_name(url)),
            Self::RedapGrpcStream { uri, .. } => Some(uri.segment_id.to_string()),

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
            Self::File { path, .. } => {
                format!("Loading {}…", path.display())
            }
            Self::Stdin => "Loading stdin…".to_owned(),
            Self::HttpStream { url, .. } => {
                format!("Waiting for data on {}…", url_display_name(url))
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
            (Self::HttpStream { url: url1, .. }, Self::HttpStream { url: url2, .. }) => {
                url1 == url2
            }
            _ => self == other,
        }
    }
}

/// A human-readable name for a source URL, safe to render as a label or log line.
///
/// `data:` URLs embed their payload inline and can be many megabytes long.
pub fn url_display_name(url: &str) -> String {
    // The part of a `data:` URL before the first comma is the media type
    // (e.g. `data:application/octet-stream;base64`); the rest is the payload.
    if url.starts_with("data:")
        && let Some(comma) = url.find(',')
    {
        return format!("{}…", &url[..=comma]);
    }

    url.to_owned()
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
#[derive(re_byte_size::SizeBytes)]
pub enum SmartMessagePayload {
    /// A message sent down the channel.
    Msg(DataSourceMessage),

    /// When received, flush anything already received and then call the given callback.
    Flush {
        #[size_bytes(ignore)]
        on_flush_done: Box<dyn FnOnce() + Send>,
    },

    /// The [`LogSender`] has quit.
    ///
    /// `None` indicates the sender left gracefully, an error indicates otherwise.
    Quit(#[size_bytes(ignore)] Option<Box<dyn std::error::Error + Send>>),
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

#[derive(Debug, re_byte_size::SizeBytes)]
pub struct SmartMessage {
    #[size_bytes(ignore)]
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

#[cfg(test)]
mod tests {
    use super::url_display_name;

    #[test]
    fn url_display_name_keeps_short_urls() {
        let url = "https://example.com/data.rrd";
        assert_eq!(url_display_name(url), url);
    }

    #[test]
    fn url_display_name_keeps_long_real_urls() {
        // Presigned links and redap URIs are legitimately long — render them in full.
        let url = format!("https://example.com/data.rrd?token={}", "x".repeat(1000));
        assert_eq!(url_display_name(&url), url);
    }

    #[test]
    fn url_display_name_truncates_long_data_url() {
        // A multi-megabyte `data:` URL must not be rendered verbatim (it OOMs text layout).
        let payload = "A".repeat(5_000_000);
        let url = format!("data:application/octet-stream;base64,{payload}");

        let name = url_display_name(&url);

        assert_eq!(name, "data:application/octet-stream;base64,…");
        assert!(name.len() < 100);
    }
}
