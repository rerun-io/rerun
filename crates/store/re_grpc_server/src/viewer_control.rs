use futures::StreamExt as _;
use re_log_channel::{DataSourceUiCommand, InspectError, SaveScreenshotError};
use re_protos::sdk_comms::v1alpha1::{
    GetViewerStateRequest, GetViewerStateResponse, InspectRequest, InspectResponse, OpenUrlRequest,
    OpenUrlResponse, SaveScreenshotRequest, SaveScreenshotResponse, SetTimeCursorRequest,
    SetTimeCursorResponse, viewer_control_service_server,
};
use re_quota_channel::async_mpsc_channel;

use crate::{Event, LogOrTableMsgProto};

/// Exposes apis to interact with a running Viewer.
pub struct ViewerControl {
    pub(super) event_tx: async_mpsc_channel::Sender<Event>,
}

impl ViewerControl {
    async fn push_ui_command(&self, cmd: DataSourceUiCommand) {
        self.event_tx
            .send(Event::Message(LogOrTableMsgProto::UiCommand(cmd)))
            .await
            .ok();
    }
}

#[tonic::async_trait]
impl viewer_control_service_server::ViewerControlService for ViewerControl {
    async fn save_screenshot(
        &self,
        request: tonic::Request<SaveScreenshotRequest>,
    ) -> tonic::Result<tonic::Response<SaveScreenshotResponse>> {
        let SaveScreenshotRequest { view_id, file_path } = request.into_inner();
        let (done_tx, mut done_rx) =
            futures::channel::mpsc::unbounded::<Result<(), SaveScreenshotError>>();
        self.push_ui_command(DataSourceUiCommand::SaveScreenshot {
            file_path: file_path.into(),
            view_id,
            on_done: Some(done_tx),
        })
        .await;

        match done_rx.next().await {
            Some(Ok(())) => Ok(tonic::Response::new(SaveScreenshotResponse {})),
            Some(Err(err @ SaveScreenshotError::InvalidViewId { .. })) => {
                Err(tonic::Status::invalid_argument(err.to_string()))
            }
            Some(Err(
                err @ (SaveScreenshotError::InvalidImageData
                | SaveScreenshotError::SaveToPathFailed { .. }),
            )) => Err(tonic::Status::internal(err.to_string())),
            None => Err(tonic::Status::internal(
                "Screenshot completion signal was dropped before the screenshot was taken",
            )),
        }
    }

    async fn set_time_cursor(
        &self,
        request: tonic::Request<SetTimeCursorRequest>,
    ) -> tonic::Result<tonic::Response<SetTimeCursorResponse>> {
        let SetTimeCursorRequest {
            store_id: recording,
            timeline,
            time,
            play,
        } = request.into_inner();
        let store_id = recording
            .map(re_log_types::StoreId::try_from)
            .transpose()
            .map_err(|err| {
                tonic::Status::invalid_argument(format!(
                    "invalid store_id: missing application id (kind: {}, recording_id: {})",
                    err.store_kind, err.recording_id
                ))
            })?;
        let (done_tx, mut done_rx) =
            futures::channel::mpsc::unbounded::<Result<SetTimeCursorResponse, String>>();
        self.push_ui_command(DataSourceUiCommand::SetTimeCursor {
            store_id,
            timeline: timeline.map(|t| t.name),
            time: time.map(|t| t.time).unwrap_or_default(),
            play,
            on_done: done_tx,
        })
        .await;

        match done_rx.next().await {
            Some(Ok(response)) => Ok(tonic::Response::new(response)),
            Some(Err(err)) => Err(tonic::Status::invalid_argument(err)),
            None => Err(tonic::Status::internal(
                "viewer dropped the set-time request before responding (is a viewer running?)",
            )),
        }
    }

    async fn open_url(
        &self,
        request: tonic::Request<OpenUrlRequest>,
    ) -> tonic::Result<tonic::Response<OpenUrlResponse>> {
        let OpenUrlRequest { url } = request.into_inner();
        let (done_tx, mut done_rx) = futures::channel::mpsc::unbounded::<Result<(), String>>();
        self.push_ui_command(DataSourceUiCommand::OpenUrl {
            url,
            on_done: done_tx,
        })
        .await;

        match done_rx.next().await {
            Some(Ok(())) => Ok(tonic::Response::new(OpenUrlResponse {})),
            Some(Err(err)) => Err(tonic::Status::invalid_argument(err)),
            None => Err(tonic::Status::internal(
                "viewer dropped the open-url request before responding (is a viewer running?)",
            )),
        }
    }

    /// Run one `egui_inspection` request against the viewer and return its response.
    ///
    /// The request/response bodies are MessagePack-encoded `egui_inspection` protocol enums,
    /// opaque to us: we forward the bytes to the viewer via [`DataSourceUiCommand::Inspect`]
    /// (which decodes, services, and re-encodes them) and return its reply. Decode/encode
    /// failures surface as gRPC errors rather than inside the response body.
    async fn inspect(
        &self,
        request: tonic::Request<InspectRequest>,
    ) -> tonic::Result<tonic::Response<InspectResponse>> {
        const INSPECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

        let request = request.into_inner().request;
        let (on_done, mut done_rx) =
            futures::channel::mpsc::unbounded::<Result<Vec<u8>, InspectError>>();
        self.push_ui_command(DataSourceUiCommand::Inspect { request, on_done })
            .await;

        match tokio::time::timeout(INSPECT_TIMEOUT, done_rx.next()).await {
            Ok(Some(Ok(response))) => Ok(tonic::Response::new(InspectResponse { response })),
            Ok(Some(Err(err @ InspectError::DecodeRequest(_)))) => {
                Err(tonic::Status::invalid_argument(err.to_string()))
            }
            Ok(Some(Err(err @ InspectError::EncodeResponse(_)))) => {
                Err(tonic::Status::internal(err.to_string()))
            }
            Ok(None) => Err(tonic::Status::internal(
                "viewer dropped the inspect request before responding (is a viewer running?)",
            )),
            Err(_) => Err(tonic::Status::deadline_exceeded(
                "viewer did not answer the inspect request in time",
            )),
        }
    }

    async fn get_viewer_state(
        &self,
        _request: tonic::Request<GetViewerStateRequest>,
    ) -> tonic::Result<tonic::Response<GetViewerStateResponse>> {
        let (done_tx, mut done_rx) = futures::channel::mpsc::unbounded::<GetViewerStateResponse>();
        self.push_ui_command(DataSourceUiCommand::GetViewerState { on_done: done_tx })
            .await;

        match done_rx.next().await {
            Some(response) => Ok(tonic::Response::new(response)),
            None => Err(tonic::Status::internal(
                "viewer dropped the state request before responding (is a viewer running?)",
            )),
        }
    }
}
