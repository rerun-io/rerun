//! The gRPC half of the `ViewerControlService` API, defined by `viewer_control.proto`.
//! That file lists every place an operation has to be added.
//!
//! Nothing here knows what any operation does. The request goes to the viewer as-is, and the
//! viewer answers with the matching response or a coded failure, which we turn into a gRPC
//! status. That keeps this crate out of the viewer's business, which matters because it sits a
//! layer below and cannot reach the viewer's types.
//!
//! `EguiInspect` is the second endpoint rather than an operation of the first: its payload is an
//! opaque body of another protocol, not something the viewer is being asked to do.

use re_log_channel::{InspectError, UiCallback, ViewerControlCommand, ViewerControlErrorCode};
use re_protos::viewer_control::v1alpha1::{
    EguiInspectRequest, EguiInspectResponse, ViewerControlRequest, ViewerControlResponse,
    viewer_control_service_server,
};
use re_quota_channel::async_mpsc_channel;

use crate::{Event, LogOrTableMsgProto};

/// How long to wait for the viewer to answer before giving up.
///
/// Covers handing the command over as well as waiting for the reply. The channel blocks its
/// sender under backpressure, so a viewer that has stopped draining it would otherwise hang the
/// caller before the clock even started.
///
/// Every operation here is answered either from viewer state or within a frame, so a viewer that
/// takes longer than this is one to fix rather than to wait for. Keep it in step with
/// `re_viewer_mcp`'s own deadlines: whichever of the two is shorter is the one that fires.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Exposes apis to interact with a running Viewer.
pub struct ViewerControl {
    pub(super) event_tx: async_mpsc_channel::Sender<Event>,
}

#[tonic::async_trait]
impl viewer_control_service_server::ViewerControlService for ViewerControl {
    async fn viewer_control(
        &self,
        request: tonic::Request<ViewerControlRequest>,
    ) -> tonic::Result<tonic::Response<ViewerControlResponse>> {
        let request = request.into_inner();
        if request.kind.is_none() {
            return Err(tonic::Status::invalid_argument(
                "`ViewerControlRequest.kind` is unset",
            ));
        }

        let (done_tx, done_rx) = futures::channel::oneshot::channel();
        let on_done = UiCallback::new(move |result| {
            done_tx.send(result).ok();
        });

        let exchange = async {
            self.push(ViewerControlCommand::ViewerControl { request, on_done })
                .await;
            done_rx.await
        };

        match tokio::time::timeout(REQUEST_TIMEOUT, exchange).await {
            Ok(Ok(Ok(response))) => Ok(tonic::Response::new(response)),
            Ok(Ok(Err(err))) => Err(match err.code {
                ViewerControlErrorCode::InvalidArgument => {
                    tonic::Status::invalid_argument(err.message)
                }
                ViewerControlErrorCode::NotFound => tonic::Status::not_found(err.message),
                ViewerControlErrorCode::FailedPrecondition => {
                    tonic::Status::failed_precondition(err.message)
                }
                ViewerControlErrorCode::Internal => tonic::Status::internal(err.message),
            }),
            Ok(Err(_)) => Err(tonic::Status::internal(
                "viewer dropped the request before responding (is a viewer running?)",
            )),
            Err(_) => Err(tonic::Status::deadline_exceeded(
                "viewer did not answer in time",
            )),
        }
    }

    /// Run one `egui_inspection` exchange against the viewer.
    ///
    /// The bodies are MessagePack-encoded `egui_inspection` protocol enums, opaque to us: we
    /// forward the bytes, and the viewer decodes, services, and re-encodes them. Decode and
    /// encode failures surface as gRPC errors rather than inside the response body.
    async fn egui_inspect(
        &self,
        request: tonic::Request<EguiInspectRequest>,
    ) -> tonic::Result<tonic::Response<EguiInspectResponse>> {
        let (done_tx, done_rx) = futures::channel::oneshot::channel();
        let on_done = UiCallback::new(move |result| {
            done_tx.send(result).ok();
        });

        let exchange = async {
            self.push(ViewerControlCommand::EguiInspect {
                request: request.into_inner().request,
                on_done,
            })
            .await;
            done_rx.await
        };

        match tokio::time::timeout(REQUEST_TIMEOUT, exchange).await {
            Ok(Ok(Ok(response))) => Ok(tonic::Response::new(EguiInspectResponse { response })),
            Ok(Ok(Err(err @ InspectError::DecodeRequest(_)))) => {
                Err(tonic::Status::invalid_argument(err.to_string()))
            }
            Ok(Ok(Err(err @ InspectError::EncodeResponse(_)))) => {
                Err(tonic::Status::internal(err.to_string()))
            }
            Ok(Err(_)) => Err(tonic::Status::internal(
                "viewer dropped the inspect request before responding (is a viewer running?)",
            )),
            Err(_) => Err(tonic::Status::deadline_exceeded(
                "viewer did not answer the inspect request in time",
            )),
        }
    }
}

impl ViewerControl {
    /// Hand a command to the viewer's UI thread.
    async fn push(&self, command: ViewerControlCommand) {
        self.event_tx
            .send(Event::Message(LogOrTableMsgProto::ViewerControl(command)))
            .await
            .ok();
    }
}
