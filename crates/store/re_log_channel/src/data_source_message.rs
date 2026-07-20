// TODO(andreas): Conceptually these should go to `re_data_source`.
// However, `re_data_source` depends on everything that _implements_ a datasource, therefore we would get a circular dependency!

use std::sync::Arc;

use re_log_encoding::RrdManifest;
use re_log_types::{LogMsg, StoreId, TableMsg, impl_into_enum};
use re_protos::sdk_comms::v1alpha1::{GetViewerStateResponse, SetTimeCursorResponse};

/// Message from a data source.
///
/// May contain limited UI commands for instrumenting the state of the receiving end.
#[derive(Clone, Debug, re_byte_size::SizeBytes)]
pub enum DataSourceMessage {
    /// A piece of the index of all the chunks in a recording.
    ///
    /// Some sources may send this, others may not.
    /// There may be one or more of these, followed by [`Self::RrdManifestComplete`].
    RrdManifest(StoreId, Arc<RrdManifest>),

    /// All parts of the RRD manifest have been sent.
    RrdManifestComplete(StoreId),

    /// See [`LogMsg`].
    LogMsg(LogMsg),

    /// See [`TableMsg`].
    TableMsg(TableMsg),

    /// A UI command that has to be ordered relative to [`LogMsg`]s.
    ///
    /// Non-ui receivers can safely ignore these.
    // TODO(RR-5073): Remove ui commands from DataSourceMessage
    UiCommand(DataSourceUiCommand),
}

impl_into_enum!(LogMsg, DataSourceMessage, LogMsg);
impl_into_enum!(TableMsg, DataSourceMessage, TableMsg);
impl_into_enum!(DataSourceUiCommand, DataSourceMessage, UiCommand);

impl DataSourceMessage {
    /// The name of the variant, useful for error message etc
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::RrdManifest(..) => "RrdManifest",
            Self::RrdManifestComplete(_) => "RrdManifestComplete",
            Self::LogMsg(_) => "LogMsg",
            Self::TableMsg(_) => "TableMsg",
            Self::UiCommand(_) => "UiCommand",
        }
    }

    // We sometimes inject meta-data for latency tracking etc
    pub fn insert_arrow_record_batch_metadata(&mut self, key: String, value: String) {
        match self {
            Self::LogMsg(log_msg) => log_msg.insert_arrow_record_batch_metadata(key, value),
            Self::TableMsg(table_msg) => table_msg.insert_arrow_record_batch_metadata(key, value),
            Self::RrdManifest(..) | Self::RrdManifestComplete(_) | Self::UiCommand(_) => {
                // Not everything needs latency tracking
            }
        }
    }
}

/// UI commands issued when streaming in datasets.
///
/// If you're not in a ui context you can safely ignore these.
#[derive(Clone, Debug)]
pub enum DataSourceUiCommand {
    /// Navigate to time/entities/anchors/etc. that are set in a `re_uri::Fragment`.
    SetUrlFragment {
        store_id: StoreId,

        /// Uri fragment, see `re_uri::Fragment` on how to parse it.
        // Not using `re_uri::Fragment` to avoid further dependency entanglement.
        fragment: String, //re_uri::Fragment,
    },

    /// Save a screenshot to a file.
    SaveScreenshot {
        /// File path to save the screenshot to.
        // TODO(#12482): Returning the screenshot to the caller would be more flexible and useful.
        file_path: camino::Utf8PathBuf,

        /// Optional view id to screenshot a specific view.
        ///
        /// If none is provided, the entire viewer is screenshotted.
        view_id: Option<String>,

        /// Optional completion signal, sent once the screenshot has been written (or failed).
        on_done: Option<futures::channel::mpsc::UnboundedSender<Result<(), SaveScreenshotError>>>,
    },

    /// Run one `egui_inspection` request against the viewer and return its response.
    Inspect {
        /// MessagePack-encoded `egui_inspection::protocol::Request`.
        request: Vec<u8>,

        /// Channel the viewer sends the MessagePack-encoded `egui_inspection::protocol::Response`
        /// back on, or an [`InspectError`] if the request could not be decoded or the response
        /// could not be encoded.
        on_done: futures::channel::mpsc::UnboundedSender<Result<Vec<u8>, InspectError>>,
    },

    /// Snapshot the current viewer state (open recordings, route, timelines + ranges,
    /// the active recording's time cursor).
    ///
    /// Used by `re_viewer_mcp`'s `GetViewerState` gRPC method to give an agent context.
    GetViewerState {
        /// Channel the viewer sends the state back on.
        on_done: futures::channel::mpsc::UnboundedSender<GetViewerStateResponse>,
    },

    /// Open a URL in the viewer (a recording/blueprint file, a `rerun://` dataset URI, a redap
    /// server/catalog URL, or an intra-recording link).
    ///
    /// Used by `re_viewer_mcp`'s `OpenUrl` gRPC method.
    OpenUrl {
        /// The URL to open.
        url: String,

        /// Channel the viewer reports back on: `Ok(())` once the URL was opened, or `Err(message)`
        /// if it could not be parsed.
        on_done: futures::channel::mpsc::UnboundedSender<Result<(), String>>,
    },

    /// Move the time cursor (timeline position) of a recording.
    ///
    /// Used by `re_viewer_mcp`'s `SetTimeCursor` gRPC method.
    SetTimeCursor {
        /// `[StoreId]` of recording to seek, or `None` for the active recording.
        store_id: Option<StoreId>,

        /// Timeline name to seek on, or `None` for the active timeline.
        timeline: Option<String>,

        /// Time value: a sequence index for sequence timelines, or nanoseconds otherwise.
        time: i64,

        /// If true, start playing from the new time cursor instead of staying paused.
        play: bool,

        /// Channel the viewer reports back on: `Ok(response)` describing what was applied, or
        /// `Err(message)` if the recording/timeline could not be resolved.
        on_done: futures::channel::mpsc::UnboundedSender<Result<SetTimeCursorResponse, String>>,
    },
}

/// Why a [`DataSourceUiCommand::SaveScreenshot`] did not produce a file.
#[derive(thiserror::Error, Debug)]
pub enum SaveScreenshotError {
    /// The requested view id could not be parsed as a UUID.
    #[error("Failed to parse view id {view_id:?}, expected a UUID")]
    InvalidViewId { view_id: String },

    /// The captured pixel data could not be turned into an image.
    #[error("Failed to create image from screenshot data")]
    InvalidImageData,

    /// Writing the screenshot to disk failed.
    #[error("Failed to save screenshot to {path}: {reason}")]
    SaveToPathFailed { path: String, reason: String },
}

/// Why a [`DataSourceUiCommand::Inspect`] request could not be serviced.
#[derive(thiserror::Error, Debug)]
pub enum InspectError {
    /// The request bytes could not be decoded as an `egui_inspection` request.
    #[error("Failed to decode inspect request: {0}")]
    DecodeRequest(String),

    /// The `egui_inspection` response could not be encoded.
    #[error("Failed to encode inspect response: {0}")]
    EncodeResponse(String),
}

impl re_byte_size::SizeBytes for DataSourceUiCommand {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::SetUrlFragment { store_id, fragment } => {
                store_id.heap_size_bytes() + fragment.heap_size_bytes()
            }
            Self::SaveScreenshot {
                file_path,
                view_id,
                on_done: _,
            } => file_path.capacity() as u64 + view_id.heap_size_bytes(),

            Self::Inspect { request, .. } => request.len() as u64,

            Self::GetViewerState { on_done: _ } => 0,
            Self::OpenUrl { url, on_done: _ } => url.heap_size_bytes(),
            Self::SetTimeCursor {
                store_id: recording_id,
                timeline,
                time: _,
                play: _,
                on_done: _,
            } => recording_id.heap_size_bytes() + timeline.heap_size_bytes(),
        }
    }
}
