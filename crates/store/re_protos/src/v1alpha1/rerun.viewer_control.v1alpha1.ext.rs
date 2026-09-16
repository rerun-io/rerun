//! Client-side helpers for the `ViewerControlService` API, defined by `viewer_control.proto`.
//! That file lists every place an operation has to be added.

use crate::viewer_control::v1alpha1::{
    CloseRecordingsRequest, CloseRecordingsResponse, GetRecordingSchemaRequest,
    GetRecordingSchemaResponse, GetViewerLogsRequest, GetViewerLogsResponse, GetViewerStateRequest,
    GetViewerStateResponse, OpenUrlRequest, OpenUrlResponse, SaveScreenshotRequest,
    SaveScreenshotResponse, SetTimeCursorRequest, SetTimeCursorResponse, ViewerControlRequest,
    ViewerControlResponse, viewer_control_request, viewer_control_response,
};

/// The peer answered a `ViewerControlService::ViewerControl` call with the wrong `kind`.
///
/// The service contract is that the response `kind` matches the request `kind`, so this only
/// happens against a buggy or mismatched peer.
#[derive(Debug, thiserror::Error)]
#[error("expected a `{expected}` response, got `{actual}`")]
pub struct UnexpectedResponseKind {
    /// The response variant the request asked for.
    pub expected: &'static str,

    /// The response variant that came back, or `"none"` if the `oneof` was unset.
    pub actual: &'static str,
}

/// One operation of `ViewerControlService`, pairing a request message with its response message.
///
/// Implemented for each request type, so a caller can go from a typed request into the
/// [`ViewerControlRequest`] envelope and back out of the [`ViewerControlResponse`] envelope
/// without naming the `oneof` variants.
pub trait ViewerControlOp: Sized {
    /// The response message this request is answered with.
    type Response;

    /// Name of this operation, as spelled in the `oneof`.
    const OP_NAME: &'static str;

    /// Wrap this request in the envelope the service accepts.
    fn into_envelope(self) -> ViewerControlRequest;

    /// Unwrap the matching response, or fail if the peer answered with a different `kind`.
    fn from_envelope(
        envelope: ViewerControlResponse,
    ) -> Result<Self::Response, UnexpectedResponseKind>;
}

/// Implements [`ViewerControlOp`] and the envelope conversions for each operation.
///
/// Adding an operation to `viewer_control.proto` means adding one line here, and nothing else in
/// this crate.
macro_rules! viewer_control_ops {
    ($($name:literal => $variant:ident($request:ident, $response:ident)),* $(,)?) => {
        impl ViewerControlResponse {
            /// Name of the set `kind`, or `"none"` if it is unset.
            pub fn kind_name(&self) -> &'static str {
                match &self.kind {
                    None => "none",
                    $(Some(viewer_control_response::Kind::$variant(_)) => $name,)*
                }
            }
        }

        $(
            impl ViewerControlOp for $request {
                type Response = $response;

                const OP_NAME: &'static str = $name;

                fn into_envelope(self) -> ViewerControlRequest {
                    ViewerControlRequest {
                        kind: Some(viewer_control_request::Kind::$variant(self)),
                    }
                }

                fn from_envelope(
                    envelope: ViewerControlResponse,
                ) -> Result<Self::Response, UnexpectedResponseKind> {
                    let actual = envelope.kind_name();
                    match envelope.kind {
                        Some(viewer_control_response::Kind::$variant(response)) => Ok(response),
                        _ => Err(UnexpectedResponseKind { expected: $name, actual }),
                    }
                }
            }

            impl From<$request> for ViewerControlRequest {
                fn from(request: $request) -> Self {
                    ViewerControlOp::into_envelope(request)
                }
            }

            impl From<$response> for ViewerControlResponse {
                fn from(response: $response) -> Self {
                    Self {
                        kind: Some(viewer_control_response::Kind::$variant(response)),
                    }
                }
            }
        )*
    };
}

viewer_control_ops! {
    "close_recordings" => CloseRecordings(CloseRecordingsRequest, CloseRecordingsResponse),
    "get_recording_schema" => GetRecordingSchema(GetRecordingSchemaRequest, GetRecordingSchemaResponse),
    "get_viewer_logs" => GetViewerLogs(GetViewerLogsRequest, GetViewerLogsResponse),
    "get_viewer_state" => GetViewerState(GetViewerStateRequest, GetViewerStateResponse),
    "open_url" => OpenUrl(OpenUrlRequest, OpenUrlResponse),
    "save_screenshot" => SaveScreenshot(SaveScreenshotRequest, SaveScreenshotResponse),
    "set_time_cursor" => SetTimeCursor(SetTimeCursorRequest, SetTimeCursorResponse),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_the_envelope() {
        let request = OpenUrlRequest {
            url: "rerun://example".to_owned(),
        };
        assert!(matches!(
            request.into_envelope().kind,
            Some(viewer_control_request::Kind::OpenUrl(_))
        ));

        let envelope = ViewerControlResponse::from(OpenUrlResponse {});
        assert!(OpenUrlRequest::from_envelope(envelope).is_ok());
    }

    #[test]
    fn rejects_a_mismatched_response_kind() {
        let envelope = ViewerControlResponse::from(OpenUrlResponse {});
        let err = GetViewerLogsRequest::from_envelope(envelope)
            .expect_err("a get_viewer_logs request must not accept an open_url response");

        assert_eq!(err.expected, "get_viewer_logs");
        assert_eq!(err.actual, "open_url");
    }

    #[test]
    fn reports_an_unset_response_kind() {
        let err = OpenUrlRequest::from_envelope(ViewerControlResponse { kind: None })
            .expect_err("an unset response kind must not be accepted");

        assert_eq!(err.actual, "none");
    }
}
