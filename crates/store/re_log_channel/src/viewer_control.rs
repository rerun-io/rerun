//! The viewer-control command that crosses into the viewer's UI thread.
//!
//! This is the transport-neutral half of the `ViewerControlService` API, defined by
//! `viewer_control.proto`. A gRPC server, or anything else that can reach this channel, turns a
//! request into a [`ViewerControlCommand`] and waits for the reply. Nothing here knows what an
//! operation does; that lives in the viewer.

use std::sync::Arc;

use re_log_types::StoreId;
use re_protos::viewer_control::v1alpha1::{ViewerControlRequest, ViewerControlResponse};

/// Calls back on the UI thread once a command completes.
type UiCallbackFn<T> = Box<dyn FnOnce(T) + Send>;

pub struct UiCallback<T>(Arc<parking_lot::Mutex<Option<UiCallbackFn<T>>>>);

impl<T> Clone for UiCallback<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T> UiCallback<T> {
    /// Creates a callback.
    pub fn new(callback: impl FnOnce(T) + Send + 'static) -> Self {
        Self(Arc::new(parking_lot::Mutex::new(Some(Box::new(callback)))))
    }

    /// Calls the callback once.
    pub fn call(&self, value: T) {
        if let Some(callback) = self.0.lock().take() {
            callback(value);
        }
    }
}

impl<T> std::fmt::Debug for UiCallback<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("UiCallback").finish()
    }
}

/// The recordings a close command targets.
#[derive(Clone, Debug)]
pub enum CloseRecordingTarget {
    /// The active recording.
    Current,

    /// Every open recording.
    All,

    /// The named recordings.
    Some(Vec<StoreId>),
}

/// Viewer-control commands, issued while streaming in datasets or by a remote client.
///
/// If you're not in a ui context you can safely ignore these.
///
/// [`Self::ViewerControl`] carries the whole `ViewerControlService` API, defined by
/// `viewer_control.proto`. That file lists every place an operation has to be added.
#[derive(Clone, Debug)]
pub enum ViewerControlCommand {
    /// Navigate to time/entities/anchors/etc. that are set in a `re_uri::Fragment`.
    SetUrlFragment {
        store_id: StoreId,

        /// Uri fragment, see `re_uri::Fragment` on how to parse it.
        // Not using `re_uri::Fragment` to avoid further dependency entanglement.
        fragment: String, //re_uri::Fragment,
    },

    /// One `egui_inspection` exchange, driving the viewer's widgets directly.
    ///
    /// Not a viewer-control operation: the payload is an opaque body of another protocol, which
    /// is why it travels beside the envelope rather than inside it.
    EguiInspect {
        /// MessagePack-encoded `egui_inspection::protocol::Request`.
        request: Vec<u8>,

        /// Callback receiving the MessagePack-encoded response, or why it could not be serviced.
        on_done: UiCallback<Result<Vec<u8>, InspectError>>,
    },

    /// One operation of the `ViewerControlService` API.
    ///
    /// The request names the operation, so this single variant covers all of them.
    ViewerControl {
        /// What to do.
        request: ViewerControlRequest,

        /// Callback receiving the matching response, or why the operation failed.
        ///
        /// Not necessarily called in the same frame: a screenshot completes only once the image
        /// has been captured and written to disk.
        on_done: UiCallback<Result<ViewerControlResponse, ViewerControlError>>,
    },
}

/// A viewer-control operation that failed, with the status a transport should report.
///
/// The viewer decides what went wrong; the transport decides how to say it. Keeping the code
/// here lets a non-gRPC transport report the same failure in its own terms.
#[derive(Clone, Debug)]
pub struct ViewerControlError {
    /// Which kind of failure this is.
    pub code: ViewerControlErrorCode,

    /// What went wrong, for a human.
    pub message: String,
}

impl ViewerControlError {
    /// The caller asked for something malformed.
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self {
            code: ViewerControlErrorCode::InvalidArgument,
            message: message.into(),
        }
    }

    /// The thing the caller named is not open.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: ViewerControlErrorCode::NotFound,
            message: message.into(),
        }
    }

    /// The viewer is not in a state where this can work.
    pub fn failed_precondition(message: impl Into<String>) -> Self {
        Self {
            code: ViewerControlErrorCode::FailedPrecondition,
            message: message.into(),
        }
    }

    /// The viewer failed to carry the operation out.
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: ViewerControlErrorCode::Internal,
            message: message.into(),
        }
    }
}

/// Why a viewer-control operation failed, in terms a transport can map to its own errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewerControlErrorCode {
    /// The request was malformed.
    InvalidArgument,

    /// The requested thing does not exist.
    NotFound,

    /// The viewer cannot do this right now.
    FailedPrecondition,

    /// The viewer failed to carry it out.
    Internal,
}

/// Why a `save_screenshot` operation did not produce a file.
#[derive(thiserror::Error, Debug)]
pub enum SaveScreenshotError {
    /// The requested view id could not be parsed as a UUID.
    #[error("Failed to parse view id {view_id:?}, expected a UUID")]
    InvalidViewId { view_id: String },

    /// The requested view does not exist (or is not currently visible).
    #[error("View {view_id} not found")]
    ViewNotFound { view_id: String },

    /// The requested view is too small to screenshot.
    #[error("View {view_id} is too small for a screenshot")]
    ViewTooSmall { view_id: String },

    /// The captured pixel data could not be turned into an image.
    #[error("Failed to create image from screenshot data")]
    InvalidImageData,

    /// Writing the screenshot to disk failed.
    #[error("Failed to save screenshot to {path}: {reason}")]
    SaveToPathFailed { path: String, reason: String },
}

/// Why an `egui_inspect` operation could not be serviced.
#[derive(thiserror::Error, Debug)]
pub enum InspectError {
    /// The request bytes could not be decoded as an `egui_inspection` request.
    #[error("Failed to decode inspect request: {0}")]
    DecodeRequest(String),

    /// The `egui_inspection` response could not be encoded.
    #[error("Failed to encode inspect response: {0}")]
    EncodeResponse(String),
}

impl re_byte_size::SizeBytes for ViewerControlCommand {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::SetUrlFragment { store_id, fragment } => {
                store_id.heap_size_bytes() + fragment.heap_size_bytes()
            }

            // Dominated by any `egui_inspect` payload, which `encoded_len` accounts for.
            Self::EguiInspect {
                request,
                on_done: _,
            } => request.len() as u64,

            Self::ViewerControl {
                request,
                on_done: _,
            } => re_protos::external::prost::Message::encoded_len(request) as u64,
        }
    }
}
