//! The viewer-side implementation of the `ViewerControlService` API, defined by
//! `viewer_control.proto`: state snapshots, time-cursor moves, closing recordings, and
//! `egui_inspection` requests.
//!
//! That file lists every place an operation has to be added.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use re_chunk::TimelineName;
use re_entity_db::LogSource;
use re_log_channel::{
    CloseRecordingTarget, InspectError, SaveScreenshotError, UiCallback, ViewerControlError,
};
use re_log_types::{EntityPath, StoreId, StoreKind, TimeReal, TimeType};
use re_protos::common::v1alpha1::TimeType as ProtoTimeType;
use re_protos::viewer_control::v1alpha1::PlayState as ProtoPlayState;
use re_protos::viewer_control::v1alpha1::{
    BlueprintUpdateMode, CloseRequest, CloseResponse, DatasetAssetSchema, GetBlueprintRequest,
    GetBlueprintResponse, GetDatasetAssetsSchemaRequest, GetDatasetAssetsSchemaResponse,
    GetDatasetSchemaRequest, GetDatasetSchemaResponse, GetLocationRequest, GetLocationResponse,
    GetLogsRequest, GetLogsResponse, GetRecordingSchemaRequest, GetRecordingSchemaResponse,
    GetStateRequest, GetStateResponse, GetTableSchemaRequest, GetTableSchemaResponse,
    HighlightRectRequest, HighlightRectResponse, OpenRequest, OpenResponse,
    RecordingComponentSchema, RecordingEntitySchema, SaveScreenshotRequest, SaveScreenshotResponse,
    ScreenRect, SetBlueprintRequest, SetBlueprintResponse, SetLocationRequest, SetLocationResponse,
    SetTimeCursorRequest, SetTimeCursorResponse, TableColumnSchema, TimeCursor,
    ViewerControlRequest, ViewerControlResponse, ViewerLoadingSource, ViewerRecording,
    ViewerReport, ViewerServer, ViewerServerEntry, ViewerSourceUrls, ViewerTimeline, ViewerView,
    get_blueprint_request, set_blueprint_request, viewer_control_request,
};
#[cfg(feature = "analytics")]
use re_protos::viewer_control::v1alpha1::{SubmitFeedbackRequest, SubmitFeedbackResponse};
use re_sdk_types::blueprint::components::PlayState;
use re_sdk_types::external::uuid;
use re_types_core::SegmentId;
use re_viewer_context::{
    Route, StoreHub, SystemCommand, SystemCommandSender as _, TimeControl, TimeControlCommand,
    viewer_url::{OpenUrlOptions, ViewerUrl},
};

use super::{App, AppCore};
use crate::registrations_in_flight::{InFlightRegistration, RegistrationId};

mod blueprint;
mod commands;
mod transform_graph;

/// How many entities `get_recording_schema` describes when the request does not say.
///
/// A recording of a few thousand entities would otherwise answer with more than the caller can
/// read. The response reports how many entities were left out, so a truncated answer is visibly
/// truncated rather than silently wrong.
const DEFAULT_MAX_SCHEMA_ENTITIES: u32 = 100;

/// How many entities `get_recording_schema` names when neither archetypes nor components are requested.
///
/// Far higher than [`DEFAULT_MAX_SCHEMA_ENTITIES`], because a path costs one line where the
/// components on it cost many — which is the whole reason to ask for paths alone.
const DEFAULT_MAX_SCHEMA_PATHS: u32 = 10_000;

/// How long `open` waits for what it loads before answering that it is still loading.
///
/// Must stay well below the transport's deadline for an answer, or a slow load reads as a viewer
/// that has stopped responding. The `Open` docs in `viewer_control.proto` state this number.
const OPEN_MAX_WAIT: Duration = Duration::from_secs(10);

const _: () = assert!(
    OPEN_MAX_WAIT.as_millis() + 5_000 <= re_log_channel::VIEWER_CONTROL_TIMEOUT.as_millis(),
    "`open` must answer well before the transport gives up"
);

/// How long `get_blueprint` waits for the recording or table on screen to get the blueprint the
/// viewer shows for it by default.
///
/// A recording or table that is not on screen answers right away. A table gets its default
/// blueprint once its query returns. A recording gets its default views a frame after its data
/// arrives, so a recording with no data waits the full time.
const DEFAULT_BLUEPRINT_MAX_WAIT: Duration = Duration::from_secs(2);

const _: () = assert!(
    DEFAULT_BLUEPRINT_MAX_WAIT.as_millis() + 5_000
        <= re_log_channel::VIEWER_CONTROL_TIMEOUT.as_millis(),
    "`get_blueprint` must answer well before the transport gives up"
);

/// How often a waiting `open` checks whether its catalog registration is done.
///
/// Nothing wakes the viewer when a registration finishes, so it has to poll. A few frames is
/// quick enough for a caller, and cheap enough for an idle viewer.
const OPEN_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// What the viewer keeps for, and shows to, `ViewerControlService` callers.
#[derive(Default)]
pub struct ViewerControlState {
    /// Answers held back until the commands they queued have run.
    pending_replies: Vec<PendingReply>,

    /// Recent log messages, served to agents through `re_viewer_mcp`.
    pub log: crate::viewer_log::ViewerLog,

    /// A rectangle an agent asked the viewer to point the user at, painted on top of everything
    /// until the user clicks.
    pub screen_highlight: Option<crate::screen_highlight::ScreenHighlight>,
}

/// A `ViewerControlService` answer held back until the work it reports on has happened.
///
/// Operations that act through queued commands would otherwise answer before those commands
/// run, so a caller that reads the viewer state right after would see the old state.
pub(super) struct PendingReply {
    on_done: UiCallback<Result<ViewerControlResponse, ViewerControlError>>,
    answer: PendingAnswer,
}

enum PendingAnswer {
    /// Sent as soon as the queued commands have run.
    Ready(Result<ViewerControlResponse, ViewerControlError>),

    /// `open`, which also waits for what the URL loads from, until `deadline`.
    Open {
        /// The URL as the caller passed it, which the answer names if it is still loading.
        url: String,

        /// The parsed URL. Its sources are worked out when answering, since a file registered
        /// with the catalog only gets its segment once the registration is done.
        parsed: ViewerUrl,

        /// When to stop waiting, and answer with what is still loading.
        deadline: web_time::Instant,
    },

    /// `get_blueprint`, read once the target has an active blueprint, and once a target on screen
    /// has the blueprint the viewer shows for it by default, or `deadline` has passed.
    GetBlueprint {
        target: BlueprintTarget,
        deadline: web_time::Instant,
    },

    /// `set_blueprint`, written once the target has an active blueprint.
    SetBlueprint {
        target: BlueprintTarget,
        json: serde_json::Value,
        mode: BlueprintUpdateMode,
    },
}

/// The `target` of a `get_blueprint` or `set_blueprint` request, before a missing id is filled in.
enum RequestedBlueprint {
    /// The blueprint of the recording with this store id, or of the active recording.
    Recording(Option<String>),

    /// The blueprint of the table with this URL, or of the table on screen.
    Table(Option<String>),
}

impl From<get_blueprint_request::Target> for RequestedBlueprint {
    fn from(target: get_blueprint_request::Target) -> Self {
        match target {
            get_blueprint_request::Target::Recording(recording) => {
                Self::Recording(recording.store_id)
            }
            get_blueprint_request::Target::Table(table) => Self::Table(table.table_url),
        }
    }
}

impl From<set_blueprint_request::Target> for RequestedBlueprint {
    fn from(target: set_blueprint_request::Target) -> Self {
        match target {
            set_blueprint_request::Target::Recording(recording) => {
                Self::Recording(recording.store_id)
            }
            set_blueprint_request::Target::Table(table) => Self::Table(table.table_url),
        }
    }
}

/// The blueprint a `get_blueprint` or `set_blueprint` request reads or writes.
enum BlueprintTarget {
    /// The blueprint of the application of this open recording.
    Recording(StoreId),

    /// The blueprint of a table.
    Table(re_uri::TableReference),
}

impl BlueprintTarget {
    /// The command that gives the target an active blueprint if it has none yet.
    fn ensure_blueprint_command(&self) -> SystemCommand {
        match self {
            Self::Recording(store_id) => SystemCommand::EnsureRecordingBlueprint(store_id.clone()),
            Self::Table(table) => SystemCommand::EnsureTableBlueprint(table.clone()),
        }
    }

    /// The target as the `store_id` and `table_url` fields of a response.
    fn response_fields(&self) -> (Option<String>, Option<String>) {
        match self {
            Self::Recording(store_id) => (Some(store_id.to_string()), None),
            Self::Table(table) => (None, table.url().map(|url| url.to_string())),
        }
    }
}

impl AppCore {
    /// Answer `on_done` with `result` once every command queued so far has run.
    fn reply_after_commands(
        &mut self,
        on_done: UiCallback<Result<ViewerControlResponse, ViewerControlError>>,
        result: Result<ViewerControlResponse, ViewerControlError>,
        egui_ctx: &egui::Context,
    ) {
        // A failed operation queued nothing, so there is nothing to wait for.
        if result.is_err() {
            on_done.call(result);
            return;
        }
        self.viewer_control.pending_replies.push(PendingReply {
            on_done,
            answer: PendingAnswer::Ready(result),
        });
        // The queued commands run, and the answer goes out, only as part of a frame. An idle or
        // hidden viewer would not run one on its own.
        egui_ctx.request_repaint();
    }

    /// Send the held-back answers whose work is done.
    ///
    /// Must be called right after draining the system commands, so that an empty queue means
    /// the commands a reply waits for have run rather than not yet been sent.
    pub(super) fn answer_pending_viewer_control_replies(&mut self, store_hub: &StoreHub) {
        if self.viewer_control.pending_replies.is_empty() {
            return;
        }

        // Read before the queues: a registration queues its commands before it stops counting.
        let registrations = self.connections.registrations_in_flight.snapshot();
        if !self.command_receiver.is_empty() {
            return;
        }

        let sources = self.rx_log.sources();
        let now = web_time::Instant::now();
        // A blueprint write is checked against the blueprint as it is, so a later read or write
        // waits until the write queued before it has run.
        let mut queued_blueprint_write = false;
        for PendingReply { on_done, answer } in
            std::mem::take(&mut self.viewer_control.pending_replies)
        {
            if queued_blueprint_write
                && matches!(
                    answer,
                    PendingAnswer::GetBlueprint { .. } | PendingAnswer::SetBlueprint { .. }
                )
            {
                self.viewer_control
                    .pending_replies
                    .push(PendingReply { on_done, answer });
                continue;
            }
            match answer {
                PendingAnswer::Ready(result) => on_done.call(result),
                PendingAnswer::Open {
                    url,
                    parsed,
                    deadline,
                } => {
                    let url_sources = self.connections.sources_of_url(&parsed);
                    let still_loading = is_loading(&url_sources, &sources, &registrations);
                    if still_loading && now < deadline {
                        self.viewer_control.pending_replies.push(PendingReply {
                            on_done,
                            answer: PendingAnswer::Open {
                                url,
                                parsed,
                                deadline,
                            },
                        });
                        self.egui_ctx.request_repaint_after(OPEN_POLL_INTERVAL);
                    } else {
                        let store_ids = store_hub
                            .recordings_from_sources(&url_sources)
                            .iter()
                            .map(StoreId::to_string)
                            .collect();
                        let loading = if still_loading { vec![url] } else { Vec::new() };
                        on_done.call(Ok(OpenResponse { store_ids, loading }.into()));
                    }
                }
                PendingAnswer::GetBlueprint { target, deadline } => {
                    if now < deadline && self.waits_for_default_blueprint(store_hub, &target) {
                        self.viewer_control.pending_replies.push(PendingReply {
                            on_done,
                            answer: PendingAnswer::GetBlueprint { target, deadline },
                        });
                        // The default views and table columns are only made while the UI paints.
                        self.egui_ctx.request_repaint();
                    } else {
                        on_done.call(
                            self.collect_blueprint_json(store_hub, &target)
                                .map(ViewerControlResponse::from),
                        );
                    }
                }
                PendingAnswer::SetBlueprint { target, json, mode } => {
                    match self.apply_set_blueprint(store_hub, &target, &json, mode) {
                        // The write is queued, so the answer waits for it to run.
                        Ok(response) => {
                            queued_blueprint_write = true;
                            self.viewer_control.pending_replies.push(PendingReply {
                                on_done,
                                answer: PendingAnswer::Ready(Ok(response.into())),
                            });
                            self.egui_ctx.request_repaint();
                        }
                        Err(err) => on_done.call(Err(err)),
                    }
                }
            }
        }
    }

    /// Run one operation of the `ViewerControlService` API on the UI thread.
    ///
    /// Every transport lands here: the request arrives decoded, and `on_done` carries the matching
    /// response or a coded failure back to whoever asked. Operations that act through queued
    /// commands answer once those commands have run, and `save_screenshot` and
    /// `capture_profile_trace` once their file has been written.
    pub(super) fn serve_viewer_control(
        &mut self,
        request: ViewerControlRequest,
        on_done: UiCallback<Result<ViewerControlResponse, ViewerControlError>>,
        store_hub: &StoreHub,
        egui_ctx: &egui::Context,
    ) {
        use viewer_control_request::Kind;

        let Some(kind) = request.kind else {
            on_done.call(Err(ViewerControlError::invalid_argument(
                "`ViewerControlRequest.kind` is unset",
            )));
            return;
        };

        match kind {
            Kind::CaptureProfileTrace(request) => {
                cfg_select! {
                    target_arch = "wasm32" => {
                        _ = request;
                        on_done.call(Err(ViewerControlError::failed_precondition(
                            "The web viewer cannot capture profile traces",
                        )));
                    }
                    _ => {
                        self.begin_profile_capture(request, on_done);
                    }
                }
            }

            Kind::Close(request) => {
                let result = close_recordings_target(request)
                    .and_then(|target| {
                        self.apply_close_recordings(store_hub, target)
                            .map_err(ViewerControlError::not_found)
                    })
                    .map(ViewerControlResponse::from);
                self.reply_after_commands(on_done, result, egui_ctx);
            }
            Kind::DescribeCommands(request) => {
                on_done.call(
                    self.describe_commands(store_hub, request, egui_ctx)
                        .map(ViewerControlResponse::from),
                );
            }
            Kind::GetBlueprint(GetBlueprintRequest { target }) => {
                let requested =
                    target.map_or_else(|| self.blueprint_on_screen(), RequestedBlueprint::from);
                match self.requested_blueprint_target(store_hub, Some(requested)) {
                    Ok(target) => self.answer_after_blueprint(
                        on_done,
                        target.ensure_blueprint_command(),
                        PendingAnswer::GetBlueprint {
                            target,
                            deadline: web_time::Instant::now() + DEFAULT_BLUEPRINT_MAX_WAIT,
                        },
                        egui_ctx,
                    ),
                    Err(err) => on_done.call(Err(err)),
                }
            }
            Kind::GetTableSchema(GetTableSchemaRequest { table_url }) => {
                on_done.call(
                    self.collect_table_schema(table_url, egui_ctx)
                        .map(ViewerControlResponse::from),
                );
            }
            Kind::GetTransformGraph(request) => {
                on_done.call(
                    self.collect_transform_graph(store_hub, request)
                        .map(ViewerControlResponse::from),
                );
            }
            Kind::GetLogs(GetLogsRequest { after_sequence }) => {
                on_done.call(Ok(GetLogsResponse {
                    entries: self.viewer_control.log.entries_after(after_sequence),
                }
                .into()));
            }
            Kind::GetDatasetSchema(request) => {
                self.answer_dataset_schema(store_hub, on_done, request);
            }
            Kind::GetDatasetAssetsSchema(request) => {
                self.answer_dataset_assets_schema(store_hub, on_done, request);
            }
            Kind::GetRecordingSchema(request) => {
                on_done.call(
                    self.collect_recording_schema(store_hub, request)
                        .map(ViewerControlResponse::from),
                );
            }
            Kind::GetLocation(GetLocationRequest {}) => {
                let location = self.current_viewer_url(store_hub).and_then(|url| {
                    url.sharable_url(None).map_err(|err| {
                        ViewerControlError::internal(format!(
                            "Failed to format the location as a URL: {err}"
                        ))
                    })
                });
                on_done.call(location.map(|location| GetLocationResponse { location }.into()));
            }
            Kind::SetLocation(SetLocationRequest { location }) => {
                let navigated = location
                    .parse::<ViewerUrl>()
                    .map_err(|err| {
                        ViewerControlError::invalid_argument(format!(
                            "Not a location: {err}\nLocation: {location:?}"
                        ))
                    })
                    .and_then(|url| self.route_if_open(store_hub, &url));
                match navigated {
                    Err(err) => on_done.call(Err(err)),
                    Ok(None) => on_done.call(Err(ViewerControlError::not_found(format!(
                        "The viewer does not have this location open: {location:?}"
                    )))),
                    Ok(Some(route)) => {
                        self.command_sender
                            .send_system(SystemCommand::SetRoute(route));
                        self.reply_after_commands(
                            on_done,
                            Ok(SetLocationResponse {}.into()),
                            egui_ctx,
                        );
                    }
                }
            }
            Kind::GetState(GetStateRequest {}) => {
                on_done.call(Ok(self.collect_viewer_state(store_hub).into()));
            }
            Kind::HighlightRect(request) => {
                let result = self.apply_highlight_rect(request, egui_ctx);
                on_done.call(result.map(ViewerControlResponse::from));
            }
            Kind::ListCommands(request) => {
                on_done.call(
                    self.list_commands(store_hub, request)
                        .map(ViewerControlResponse::from),
                );
            }
            Kind::Open(OpenRequest { url, navigate }) => {
                let navigate = navigate.unwrap_or(true);
                let parsed = ViewerUrl::parse_with_options(
                    &url,
                    &re_data_source::FromUriOptions {
                        accept_extensionless_http: true,
                    },
                );
                match parsed {
                    Err(err) => {
                        on_done.call(Err(ViewerControlError::invalid_argument(format!(
                            "Failed to parse the URL: {err}\nURL: {url:?}"
                        ))));
                    }
                    Ok(open_url) => match self.route_if_open(store_hub, &open_url) {
                        Err(err) => on_done.call(Err(err)),
                        // Opening a server, folder, or entry again also refreshes it, unlike
                        // `set_location`.
                        Ok(Some(route))
                            if !matches!(
                                route,
                                Route::RedapServer(_)
                                    | Route::RedapFolder { .. }
                                    | Route::RedapEntry { .. }
                            ) =>
                        {
                            let mut store_ids = store_hub.recordings_from_sources(
                                &self.connections.sources_of_url(&open_url),
                            );
                            if let Route::LocalRecording { recording_id } = &route
                                && !store_ids.contains(recording_id)
                            {
                                store_ids.push(recording_id.clone());
                            }
                            let store_ids = store_ids.iter().map(StoreId::to_string).collect();
                            if navigate {
                                self.command_sender
                                    .send_system(SystemCommand::SetRoute(route));
                            }
                            let response = OpenResponse {
                                store_ids,
                                loading: Vec::new(),
                            };
                            self.reply_after_commands(on_done, Ok(response.into()), egui_ctx);
                        }
                        Ok(_) => {
                            let parsed = open_url.clone();
                            if navigate {
                                // Opens like a URL the user pastes: a recording becomes the active one.
                                open_url.open(
                                    egui_ctx,
                                    &OpenUrlOptions {
                                        recording_open_behavior:
                                            re_log_channel::RecordingOpenBehavior::OpenAndSelect,
                                        show_loader: true,
                                    },
                                    &self.command_sender,
                                );
                            } else if !self.connect_without_navigating(&open_url) {
                                open_url.open(
                                    egui_ctx,
                                    &OpenUrlOptions {
                                        recording_open_behavior:
                                            re_log_channel::RecordingOpenBehavior::Open,
                                        show_loader: false,
                                    },
                                    &self.command_sender,
                                );
                            }
                            self.viewer_control.pending_replies.push(PendingReply {
                                on_done,
                                answer: PendingAnswer::Open {
                                    url,
                                    parsed,
                                    deadline: web_time::Instant::now() + OPEN_MAX_WAIT,
                                },
                            });
                            egui_ctx.request_repaint();
                        }
                    },
                }
            }

            Kind::RunCommand(request) => {
                let result = self
                    .run_command(store_hub, request, egui_ctx)
                    .map(ViewerControlResponse::from);
                self.reply_after_commands(on_done, result, egui_ctx);
            }

            Kind::SaveScreenshot(request) => self.begin_screenshot(request, on_done),

            Kind::SetBlueprint(SetBlueprintRequest { json, target, mode }) => {
                let parsed = serde_json::from_str(&json)
                    .map_err(|err| {
                        ViewerControlError::invalid_argument(format!(
                            "`json` is not valid JSON: {err}"
                        ))
                    })
                    .and_then(|json| {
                        let mode = mode
                            .map(BlueprintUpdateMode::try_from)
                            .transpose()
                            .map_err(|err| {
                                ViewerControlError::invalid_argument(format!(
                                    "invalid blueprint update mode: {err}"
                                ))
                            })?
                            .unwrap_or(BlueprintUpdateMode::Full);
                        let target = self.requested_blueprint_target(
                            store_hub,
                            target.map(RequestedBlueprint::from),
                        )?;
                        Ok((target, json, mode))
                    });
                match parsed {
                    Ok((target, json, mode)) => self.answer_after_blueprint(
                        on_done,
                        target.ensure_blueprint_command(),
                        PendingAnswer::SetBlueprint { target, json, mode },
                        egui_ctx,
                    ),
                    Err(err) => on_done.call(Err(err)),
                }
            }

            Kind::SubmitFeedback(request) => {
                cfg_select! {
                    feature = "analytics" => {
                        if let Err(err) = check_feedback(&request) {
                            on_done.call(Err(err));
                        } else {
                            // The text is only sent with "Share redacted prompts with Rerun",
                            // after the panel's agent has redacted it.
                            let share_text = self.state.agent_settings.share_redacted_prompts;
                            self.agent_panel.submit_product_feedback(request, share_text);
                            on_done.call(Ok(SubmitFeedbackResponse {}.into()));
                        }
                    }
                    _ => {
                        _ = request;
                        on_done.call(Err(ViewerControlError::failed_precondition(
                            "This viewer was built without analytics, so it cannot send feedback",
                        )));
                    }
                }
            }

            Kind::SetTimeCursor(request) => {
                let SetTimeCursorRequest {
                    store_id,
                    timeline,
                    time,
                    play_state,
                } = request;
                let store_id = store_id
                    .map(|store_id| store_id.parse::<StoreId>())
                    .transpose()
                    .map_err(|err| {
                        ViewerControlError::invalid_argument(format!("invalid store_id: {err}"))
                    });
                let play_state = play_state
                    .map(play_state_from_proto)
                    .transpose()
                    .map(Option::flatten);
                let result = store_id
                    .and_then(|store_id| Ok((store_id, play_state?)))
                    .and_then(|(store_id, play_state)| {
                        self.apply_set_time_cursor(
                            store_hub,
                            store_id,
                            timeline.map(|timeline| timeline.name).as_deref(),
                            time.map(|time| time.time),
                            play_state,
                            egui_ctx,
                        )
                        .map_err(ViewerControlError::invalid_argument)
                    })
                    .map(ViewerControlResponse::from);
                self.reply_after_commands(on_done, result, egui_ctx);
            }
        }
    }

    /// Ask for a screenshot, and answer `on_done` once it has been written.
    ///
    /// The capture needs at least one more frame, so the callback is parked until then. Wrapping
    /// it here keeps the screenshot plumbing typed in terms of its own error.
    fn begin_screenshot(
        &self,
        request: SaveScreenshotRequest,
        on_done: UiCallback<Result<ViewerControlResponse, ViewerControlError>>,
    ) {
        let SaveScreenshotRequest {
            view_id,
            file_path,
            max_size,
        } = request;

        let view_id = match view_id.map(|view_id| {
            uuid::Uuid::parse_str(&view_id)
                .map_err(|_err| SaveScreenshotError::InvalidViewId { view_id })
        }) {
            None => None,
            Some(Ok(uuid)) => Some(uuid.into()),
            Some(Err(err)) => {
                on_done.call(Err(ViewerControlError::invalid_argument(err.to_string())));
                return;
            }
        };

        let target = if let Some(file_path) = file_path {
            re_viewer_context::ScreenshotTarget::SaveToPath {
                path: file_path.into(),
                max_size,
                on_done: Some(UiCallback::new(
                    move |result: Result<(), SaveScreenshotError>| {
                        on_done.call(
                            result
                                .map(|()| SaveScreenshotResponse { png: None }.into())
                                .map_err(|err| screenshot_error(&err)),
                        );
                    },
                )),
            }
        } else {
            re_viewer_context::ScreenshotTarget::EncodePng {
                max_size,
                on_done: UiCallback::new(move |result: Result<Vec<u8>, SaveScreenshotError>| {
                    on_done.call(
                        result
                            .map(|png| SaveScreenshotResponse { png: Some(png) }.into())
                            .map_err(|err| screenshot_error(&err)),
                    );
                }),
            }
        };

        self.command_sender
            .send_system(SystemCommand::SaveScreenshot {
                target,
                view_id,
                notify: false,
            });
    }

    /// Point the user at a rectangle on screen, or clear the current highlight.
    fn apply_highlight_rect(
        &mut self,
        request: HighlightRectRequest,
        egui_ctx: &egui::Context,
    ) -> Result<HighlightRectResponse, ViewerControlError> {
        let HighlightRectRequest { rect, label } = request;

        self.viewer_control.screen_highlight = rect
            .map(|rect| {
                let ScreenRect { x, y, w, h } = rect;
                let rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h));
                // A degenerate or non-finite rectangle would paint an outline the user cannot
                // see, which reads as the viewer having ignored the request. The protobuf floats
                // are whatever the caller sent, so NaN and infinity both have to be caught here.
                if !rect.is_finite() || !rect.is_positive() {
                    return Err(ViewerControlError::invalid_argument(format!(
                        "`rect` must be finite with a positive size, got {w}x{h} at ({x}, {y})"
                    )));
                }
                let now = egui_ctx.input(|i| i.time);
                Ok(crate::screen_highlight::ScreenHighlight::new(
                    rect, label, now,
                ))
            })
            .transpose()?;

        egui_ctx.request_repaint();

        Ok(HighlightRectResponse {})
    }
}

/// Whether feedback that an agent sent about Rerun can be sent with analytics.
#[cfg(feature = "analytics")]
fn check_feedback(request: &SubmitFeedbackRequest) -> Result<(), ViewerControlError> {
    if request.title.trim().is_empty() {
        return Err(ViewerControlError::invalid_argument("`title` is empty"));
    }
    if !re_analytics::is_enabled() {
        return Err(ViewerControlError::failed_precondition(
            "Analytics is turned off in this viewer, so feedback cannot be sent",
        ));
    }
    Ok(())
}

/// Which recordings a `close` request selects.
fn close_recordings_target(
    request: CloseRequest,
) -> Result<CloseRecordingTarget, ViewerControlError> {
    use re_protos::viewer_control::v1alpha1::close_request::Target;

    Ok(match request.target {
        // A `oneof` is an enum: an unset target names no recording, so there is nothing to
        // default to without closing something the caller did not ask for.
        None => {
            return Err(ViewerControlError::invalid_argument(
                "`close` takes exactly one of `current`, `all`, `store_ids`, or `urls`",
            ));
        }

        Some(Target::Current(true)) => CloseRecordingTarget::Current,
        Some(Target::All(true)) => CloseRecordingTarget::All,

        // These are selectors carried as bools, so a `false` says nothing about what to close.
        // Acting on it would close something the caller did not ask for.
        Some(Target::Current(false) | Target::All(false)) => {
            return Err(ViewerControlError::invalid_argument(
                "`current` and `all` select what to close, so they must be `true`",
            ));
        }

        Some(Target::StoreIds(store_ids)) => CloseRecordingTarget::Some(
            store_ids
                .store_ids
                .into_iter()
                .map(|store_id| store_id.parse::<StoreId>())
                .collect::<Result<_, _>>()
                .map_err(|err| {
                    ViewerControlError::invalid_argument(format!("invalid store_id: {err}"))
                })?,
        ),

        Some(Target::Urls(ViewerSourceUrls { urls })) => {
            let mut sources = Vec::new();
            for url in urls {
                let parsed = ViewerUrl::parse_with_options(
                    &url,
                    &re_data_source::FromUriOptions {
                        accept_extensionless_http: true,
                    },
                )
                .map_err(|err| {
                    ViewerControlError::invalid_argument(format!(
                        "Failed to parse the URL: {err}\nURL: {url:?}"
                    ))
                })?;
                let url_sources = parsed.data_sources();
                if url_sources.is_empty() {
                    return Err(ViewerControlError::invalid_argument(format!(
                        "This URL loads no recording, so there is nothing to close: {url:?}"
                    )));
                }
                sources.extend(url_sources);
            }
            CloseRecordingTarget::Sources(sources)
        }
    })
}

impl App {
    /// The `get_state` response.
    #[cfg(target_arch = "wasm32")]
    pub fn viewer_state(&mut self) -> GetStateResponse {
        let Self { store_hub, core } = self;
        core.collect_viewer_state(store_hub)
    }
}

impl AppCore {
    /// What the viewer is showing, as a URL that [`Self::route_if_open`] returns to.
    ///
    /// A recording is named by its store id, since it may not have been loaded from a URL.
    fn current_viewer_url(&self, store_hub: &StoreHub) -> Result<ViewerUrl, ViewerControlError> {
        Ok(match self.state.navigation.current() {
            Route::LocalRecording { recording_id } => {
                ViewerUrl::LocalRecording(recording_id.clone())
            }
            // Nothing but this API names a local table, so it gets no sharable URL.
            Route::LocalTable(table_id) => ViewerUrl::LocalTable(table_id.clone()),
            Route::ChunkStoreBrowser {
                store_id: Some(store_id),
                ..
            } if store_id.kind() != StoreKind::Recording => {
                // A chunk store browser URL can only name a recording.
                return Err(ViewerControlError::failed_precondition(
                    "The chunk store browser shows a blueprint, which has no URL",
                ));
            }
            route => ViewerUrl::from_route(store_hub, route).map_err(|err| {
                ViewerControlError::failed_precondition(format!(
                    "What the viewer shows has no URL: {err}"
                ))
            })?,
        })
    }

    /// The route to what `url` names, if the viewer already has it open.
    ///
    /// Showing it takes a plain route change, which navigates like the user does, so the back
    /// button returns to the page shown before. `ViewerUrl::open` would also add servers,
    /// refresh their entries, and load data again.
    fn route_if_open(
        &self,
        store_hub: &StoreHub,
        url: &ViewerUrl,
    ) -> Result<Option<Route>, ViewerControlError> {
        let not_loaded = |store_id: &StoreId| {
            // Nothing can load an `about:recording` URL, so it must name a loaded recording.
            ViewerControlError::not_found(format!("Recording {store_id} is not loaded"))
        };
        let has_server = |origin: &re_uri::Origin| {
            // The welcome page is a built-in server that the viewer never connects to.
            *origin == *re_redap_browser::EXAMPLES_ORIGIN
                || self.state.redap_servers.has_server(origin)
        };

        let Some(mut route) = url.to_route(self.state.navigation.current()) else {
            return Ok(None);
        };
        match &mut route {
            Route::LocalRecording { recording_id }
            | Route::ChunkStoreBrowser {
                store_id: Some(recording_id),
                ..
            } => {
                if store_hub.entity_db(recording_id).is_none() {
                    return Err(not_loaded(recording_id));
                }
            }
            Route::RedapServer(origin) | Route::RedapFolder { origin, .. } => {
                if !has_server(origin) {
                    return Ok(None);
                }
            }
            Route::RedapEntry {
                origin,
                entry_id,
                kind,
            } => {
                let Some(known_kind) = self.state.redap_servers.entry_kind(origin, *entry_id)
                else {
                    return Ok(None);
                };
                if matches!(kind, Some(re_viewer_context::EntryKind::Dataset(_)))
                    && known_kind == re_viewer_context::EntryKind::Table
                {
                    return Err(ViewerControlError::invalid_argument(format!(
                        "The URL names a table, not a dataset\nURL: {url:?}"
                    )));
                }
                kind.get_or_insert(known_kind);
            }
            Route::LocalTable(table_id) => {
                if !store_hub.has_table(table_id) {
                    // Nothing can load an `about:table` URL either.
                    return Err(ViewerControlError::not_found(format!(
                        "Table {table_id} is not in the viewer"
                    )));
                }
            }
            Route::Settings { .. } | Route::ChunkStoreBrowser { store_id: None, .. } => {}
            Route::Loading(_) => return Ok(None),
        }

        Ok(Some(route))
    }

    /// Connect to the server that `url` names, without showing it.
    ///
    /// Returns `false` for a URL that names no server.
    fn connect_without_navigating(&self, url: &ViewerUrl) -> bool {
        let (ViewerUrl::RedapCatalog(re_uri::CatalogUri { origin })
        | ViewerUrl::RedapFolder(re_uri::FolderUri { origin, .. })
        | ViewerUrl::RedapEntry(re_uri::EntryUri { origin, .. })
        | ViewerUrl::RedapDataset(re_uri::DatasetUri {
            origin,
            segment_id: None,
            ..
        })) = url
        else {
            return false;
        };
        self.command_sender
            .send_system(SystemCommand::AddRedapServer(origin.clone()));
        self.command_sender
            .send_system(SystemCommand::RefreshRedapServer(origin.clone()));
        true
    }

    /// Snapshot the current viewer state for the `get_state` operation:
    /// the active recording, the current page as a Viewer URL, the catalog the viewer hosts,
    /// and every open recording's timelines with their time ranges and current time cursor.
    pub(super) fn collect_viewer_state(&mut self, store_hub: &StoreHub) -> GetStateResponse {
        let active_id = self.state.active_recording_id().cloned();
        let route = self.state.navigation.current().clone();
        let views = self.collect_views(store_hub, &route, active_id.as_ref());

        // Empty when what is shown has no URL, like a local table.
        let location = self
            .current_viewer_url(store_hub)
            .ok()
            .and_then(|url| url.sharable_url(None).ok())
            .unwrap_or_default();

        let recordings = store_hub
            .store_bundle()
            .recordings()
            .map(|db| {
                let store_id = db.store_id();
                let time_ctrl = self.state.time_control(store_id);
                let timelines = recording_timelines(db, time_ctrl);

                let current_time = time_ctrl.map(|time_ctrl| TimeCursor {
                    timeline: Some((*time_ctrl.timeline_name()).into()),
                    time_type: time_ctrl.time_type().map(|t| ProtoTimeType::from(t) as i32),
                    time: time_ctrl.time_int().map(|t| t.as_i64().into()),
                });
                let play_state = time_ctrl.map_or(PlayState::Paused, TimeControl::play_state);

                ViewerRecording {
                    store_id: store_id.to_string(),
                    timelines,
                    current_time,
                    location: ViewerUrl::LocalRecording(store_id.clone())
                        .sharable_url(None)
                        .unwrap_or_default(),
                    play_state: play_state_to_proto(play_state) as i32,
                }
            })
            .collect();

        // `loading_name` is the viewer's own "actively loading" predicate — the one the welcome
        // screen shows a spinner for — so a source that is only waiting to be sent data is left
        // out, as it would otherwise never clear.
        let sources = self.rx_log.sources();
        let loading_sources = sources.iter().filter_map(|source| {
            source.loading_name().map(|name| ViewerLoadingSource {
                name,
                status: source.status_string(),
            })
        });
        // A file being registered with a catalog has no source yet: it gets one once registered.
        let registrations = self.connections.registrations_in_flight.snapshot();
        let loading_registrations =
            registrations
                .into_values()
                .map(
                    |InFlightRegistration { origin, name }| ViewerLoadingSource {
                        status: format!("Registering {name} with {origin}…"),
                        name,
                    },
                );
        let loading = std::iter::chain(loading_registrations, loading_sources).collect();

        GetStateResponse {
            location,
            active_store_id: active_id.map(|store_id| store_id.to_string()),
            recordings,
            loading,
            views,
            catalog_url: self
                .connections
                .registry
                .internal_origin()
                .map(|origin| origin.to_string()),
            viewer_version: Some(self.startup.build_info.version.to_string()),
            servers: self.collect_servers(),
        }
    }

    /// The Redap servers in the left panel, each with the catalog entries it has listed so far.
    fn collect_servers(&self) -> Vec<ViewerServer> {
        self.state
            .redap_servers
            .iter_servers()
            .map(|server| {
                let origin = server.origin();
                let (entries, error) = match server.entries().state() {
                    Poll::Pending => (Vec::new(), None),
                    Poll::Ready(Err(err)) => (Vec::new(), Some(err.to_string())),
                    Poll::Ready(Ok(entries)) => {
                        let mut entries: Vec<_> = entries
                            .values()
                            .map(|entry| ViewerServerEntry {
                                name: entry.name().to_string(),
                                kind: entry.details().kind as i32,
                                url: re_uri::EntryUri::new(origin.clone(), entry.id()).to_string(),
                            })
                            .collect();
                        entries.sort_by(|a, b| a.name.cmp(&b.name));
                        (entries, None)
                    }
                };
                ViewerServer {
                    origin: origin.to_string(),
                    entries,
                    error,
                }
            })
            .collect()
    }

    /// The active blueprint of a recording or a table, as JSON, for the `get_blueprint`
    /// operation.
    fn collect_blueprint_json(
        &mut self,
        store_hub: &StoreHub,
        target: &BlueprintTarget,
    ) -> Result<GetBlueprintResponse, ViewerControlError> {
        re_tracing::profile_function!();

        let blueprint_db = self.active_blueprint(store_hub, target)?;
        let query = self.state.blueprint_query_for_viewer(Some(blueprint_db));
        let mut json = re_arrow_json::json_from_store(
            blueprint_db.storage_engine().store(),
            &query,
            &self.registries.reflection.read(),
        );
        if let BlueprintTarget::Table(table) = target
            && json.as_object().is_some_and(serde_json::Map::is_empty)
            && let Some(default_json) = self.default_table_blueprint_json(table)
        {
            json = default_json;
        }

        let (store_id, table_url) = target.response_fields();
        Ok(GetBlueprintResponse {
            blueprint_id: blueprint_db.store_id().to_string(),
            json: json.to_string(),
            store_id,
            table_url,
        })
    }

    /// Update the active blueprint of a recording or a table with JSON, for the
    /// `set_blueprint` operation.
    ///
    /// Only queues the write, so the caller must hold the answer back until it has run.
    fn apply_set_blueprint(
        &self,
        store_hub: &StoreHub,
        target: &BlueprintTarget,
        json: &serde_json::Value,
        mode: BlueprintUpdateMode,
    ) -> Result<SetBlueprintResponse, ViewerControlError> {
        re_tracing::profile_function!();

        let blueprint_db = self.active_blueprint(store_hub, target)?;
        let chunks = blueprint::chunks_from_json(
            blueprint_db,
            json,
            mode,
            matches!(target, BlueprintTarget::Recording(_)),
            &self.registries.reflection.read(),
        )?;
        if let BlueprintTarget::Table(table) = target {
            self.check_table_filters(table, &chunks)?;
        }
        blueprint::check_references(
            blueprint_db,
            self.state.blueprint_undo_state.get(blueprint_db.store_id()),
            &chunks,
        )?;
        let created_entities = blueprint::created_entities(blueprint_db, &chunks);
        self.queue_blueprint_write(blueprint_db, chunks);

        let (store_id, table_url) = target.response_fields();
        Ok(SetBlueprintResponse {
            blueprint_id: blueprint_db.store_id().to_string(),
            store_id,
            table_url,
            created_entities,
        })
    }

    /// The blueprint a `get_blueprint` or `set_blueprint` request names, with the active
    /// recording or the table on screen filled in for a missing id.
    fn requested_blueprint_target(
        &self,
        store_hub: &StoreHub,
        requested: Option<RequestedBlueprint>,
    ) -> Result<BlueprintTarget, ViewerControlError> {
        let store_id = match requested {
            // A `oneof` is an enum: an unset target names neither a recording nor a table.
            None => {
                return Err(ViewerControlError::invalid_argument(
                    "set exactly one of `recording` and `table`: `{\"recording\": {}}` for the \
                     active recording, or `{\"table\": {}}` for the table on screen",
                ));
            }
            Some(RequestedBlueprint::Table(table_url)) => {
                return Ok(BlueprintTarget::Table(self.requested_table(table_url)?));
            }
            Some(RequestedBlueprint::Recording(Some(store_id))) => {
                store_id.parse::<StoreId>().map_err(|err| {
                    ViewerControlError::invalid_argument(format!("invalid store_id: {err}"))
                })?
            }
            Some(RequestedBlueprint::Recording(None)) => {
                self.state.active_recording_id().cloned().ok_or_else(|| {
                    ViewerControlError::failed_precondition(
                        "no active recording, and no `store_id`",
                    )
                })?
            }
        };

        if store_hub.entity_db(&store_id).is_none() {
            return Err(ViewerControlError::not_found(format!(
                "recording {store_id} is not open"
            )));
        }
        Ok(BlueprintTarget::Recording(store_id))
    }

    /// The blueprint of what is on screen: the table if one is shown, else the active recording.
    fn blueprint_on_screen(&self) -> RequestedBlueprint {
        if self.state.navigation.current().table_reference().is_some() {
            RequestedBlueprint::Table(None)
        } else {
            RequestedBlueprint::Recording(None)
        }
    }

    /// Whether `target` is on screen and does not have the blueprint the viewer shows for it by
    /// default yet.
    ///
    /// If the recording or table doesn't have an assigned blueprint and isn't the active route,
    /// the returned blueprint is empty.
    fn waits_for_default_blueprint(
        &mut self,
        store_hub: &StoreHub,
        target: &BlueprintTarget,
    ) -> bool {
        let route = self.state.navigation.current().clone();
        match target {
            BlueprintTarget::Recording(store_id) => {
                let on_screen = route
                    .recording_id()
                    .is_some_and(|shown| shown.application_id() == store_id.application_id());
                if !on_screen {
                    return false;
                }
                let Some(blueprint_db) =
                    store_hub.active_blueprint_for_app(store_id.application_id())
                else {
                    return false;
                };
                let query = self.state.blueprint_query_for_viewer(Some(blueprint_db));
                let viewport =
                    re_viewport_blueprint::ViewportBlueprint::from_db(blueprint_db, &query);
                viewport.auto_views() && viewport.views.is_empty()
            }
            BlueprintTarget::Table(table) => {
                route.table_reference().as_ref() == Some(table)
                    && re_dataframe_ui::DataFusionTableWidget::table_schema(&self.egui_ctx, table)
                        .is_none()
            }
        }
    }

    /// The blueprint a table that was never configured shows: its column order, and the columns
    /// it hides, as it was last shown.
    ///
    /// `None` if the table has not been shown yet.
    fn default_table_blueprint_json(
        &self,
        table: &re_uri::TableReference,
    ) -> Option<serde_json::Value> {
        use re_dataframe_ui::{TableColumn, TableLayoutKind};

        let columns = re_dataframe_ui::DataFusionTableWidget::table_schema(&self.egui_ctx, table)?;

        let mut json = serde_json::Map::new();
        json.insert(
            "/table/layouts/table".to_owned(),
            serde_json::json!({
                "TableLayout": {
                    "column_order": columns
                        .iter()
                        .map(|column| column.physical_name.as_str())
                        .collect::<Vec<_>>(),
                }
            }),
        );
        for column in columns.iter().filter(|column| !column.visible) {
            json.insert(
                TableColumn::blueprint_path(TableLayoutKind::Table, &column.physical_name)
                    .to_string(),
                serde_json::json!({ "TableColumn": { "visible": false } }),
            );
        }
        Some(json.into())
    }

    /// The table a request names, or else the table on screen.
    fn requested_table(
        &self,
        table_url: Option<String>,
    ) -> Result<re_uri::TableReference, ViewerControlError> {
        let Some(url) = table_url else {
            return self
                .state
                .navigation
                .current()
                .table_reference()
                .ok_or_else(|| {
                    ViewerControlError::failed_precondition(
                        "no table is on screen, and no `table_url`",
                    )
                });
        };

        let uri = url.parse::<re_uri::RedapUri>().map_err(|err| {
            ViewerControlError::invalid_argument(format!("invalid `table_url` {url:?}: {err}"))
        })?;

        let redap_servers = &self.state.redap_servers;
        match uri {
            re_uri::RedapUri::Catalog(catalog) if redap_servers.has_server(&catalog.origin) => {
                Ok(re_uri::TableReference::RedapServerEntries {
                    origin: catalog.origin,
                })
            }
            re_uri::RedapUri::Entry(entry)
                if redap_servers
                    .entry_kind(&entry.origin, entry.entry_id)
                    .is_some() =>
            {
                Ok(entry.into())
            }
            re_uri::RedapUri::Catalog(_) | re_uri::RedapUri::Entry(_) => {
                Err(ViewerControlError::not_found(format!(
                    "the viewer has no table at {url:?}. `GetState` lists the tables it has"
                )))
            }
            re_uri::RedapUri::Dataset(_)
            | re_uri::RedapUri::Proxy(_)
            | re_uri::RedapUri::Folder(_) => Err(ViewerControlError::invalid_argument(format!(
                "{url:?} is not a table URL"
            ))),
        }
    }

    /// The dataset a `get_dataset_schema` request names, or else the dataset on screen.
    fn requested_dataset(
        &self,
        store_hub: &StoreHub,
        dataset_url: Option<String>,
    ) -> Result<re_uri::EntryUri, ViewerControlError> {
        let Some(url) = dataset_url else {
            return self.dataset_on_screen(store_hub, self.state.navigation.current());
        };

        let uri = url.parse::<re_uri::RedapUri>().map_err(|err| {
            ViewerControlError::invalid_argument(format!("invalid `dataset_url` {url:?}: {err}"))
        })?;
        let re_uri::RedapUri::Entry(entry) = uri else {
            return Err(ViewerControlError::invalid_argument(format!(
                "{url:?} is not a dataset URL"
            )));
        };

        match self
            .state
            .redap_servers
            .entry_kind(&entry.origin, entry.entry_id)
        {
            Some(re_viewer_context::EntryKind::Dataset(_)) => Ok(entry),
            Some(re_viewer_context::EntryKind::Table) => Err(ViewerControlError::invalid_argument(
                format!("{url:?} is a table, not a dataset"),
            )),
            None => Err(ViewerControlError::not_found(format!(
                "the viewer has no dataset at {url:?}. `GetState` lists the datasets it has"
            ))),
        }
    }

    /// The dataset entry `route` shows, or the dataset of the segment it shows.
    ///
    /// An entry of unknown kind counts as a dataset, and the server answers for it.
    fn dataset_on_screen(
        &self,
        store_hub: &StoreHub,
        route: &Route,
    ) -> Result<re_uri::EntryUri, ViewerControlError> {
        let no_dataset = || {
            ViewerControlError::failed_precondition("no dataset is on screen, and no `dataset_url`")
        };

        match route {
            Route::Settings { return_route } => self.dataset_on_screen(store_hub, return_route),
            Route::RedapEntry {
                origin,
                entry_id,
                kind,
            } => {
                let kind = self
                    .state
                    .redap_servers
                    .entry_kind(origin, *entry_id)
                    .or(*kind);
                if matches!(kind, Some(re_viewer_context::EntryKind::Table)) {
                    return Err(ViewerControlError::invalid_argument(
                        "the table on screen is not a dataset. `GetTableSchema` names its columns",
                    ));
                }
                Ok(re_uri::EntryUri {
                    origin: origin.clone(),
                    entry_id: *entry_id,
                })
            }
            Route::LocalRecording { recording_id } => {
                let Some(uri) = store_hub
                    .entity_db(recording_id)
                    .and_then(|db| db.redap_uri())
                else {
                    return Err(no_dataset());
                };
                Ok(re_uri::EntryUri {
                    origin: uri.origin.clone(),
                    entry_id: uri.dataset_id.into(),
                })
            }
            _ => Err(no_dataset()),
        }
    }

    /// Fetch the schema of a dataset from its server, for the `get_dataset_schema` operation.
    ///
    /// Answers once the server has replied.
    fn answer_dataset_schema(
        &self,
        store_hub: &StoreHub,
        on_done: UiCallback<Result<ViewerControlResponse, ViewerControlError>>,
        request: GetDatasetSchemaRequest,
    ) {
        let GetDatasetSchemaRequest {
            dataset_url,
            entity_path,
            max_entities,
            paths_only,
        } = request;
        // A whole dataset describes many components on each entity, so without an `entity_path`
        // it only names the entities.
        let paths_only = paths_only.unwrap_or_else(|| entity_path.is_none());
        let max_entities = max_entities.unwrap_or(if paths_only {
            DEFAULT_MAX_SCHEMA_PATHS
        } else {
            DEFAULT_MAX_SCHEMA_ENTITIES
        }) as usize;

        let (entry, root, catalog) =
            match self.schema_request_target(store_hub, dataset_url, entity_path) {
                Ok(target) => target,
                Err(err) => {
                    on_done.call(Err(err));
                    return;
                }
            };

        self.async_runtime.spawn_future(async move {
            let (schema, assets) = futures::join!(
                catalog.get_dataset_schema(entry.entry_id),
                catalog.registered_assets(entry.entry_id)
            );

            let response = schema
                .map_err(|err| {
                    ViewerControlError::internal(format!(
                        "failed to fetch the schema of {entry}: {err}"
                    ))
                })
                .and_then(|schema| {
                    let asset_ids = assets
                        .map_err(|err| {
                            ViewerControlError::internal(format!(
                                "failed to fetch the assets of {entry}: {err}"
                            ))
                        })?
                        .map(|registered| {
                            registered
                                .assets
                                .iter()
                                .map(|asset| asset.id.to_string())
                                .collect()
                        })
                        .unwrap_or_default();
                    describe_dataset_schema(
                        &schema.schema,
                        asset_ids,
                        root.as_ref(),
                        max_entities,
                        paths_only,
                    )
                })
                .map(ViewerControlResponse::from);
            on_done.call(response);
        });
    }

    /// Fetch the schema of each asset of a dataset from its server, for the
    /// `get_dataset_assets_schema` operation.
    ///
    /// Answers once the server has replied.
    fn answer_dataset_assets_schema(
        &self,
        store_hub: &StoreHub,
        on_done: UiCallback<Result<ViewerControlResponse, ViewerControlError>>,
        request: GetDatasetAssetsSchemaRequest,
    ) {
        let GetDatasetAssetsSchemaRequest {
            dataset_url,
            entity_path,
            max_entities,
            paths_only,
        } = request;
        let paths_only = paths_only.unwrap_or(false);
        let max_entities = max_entities.unwrap_or(if paths_only {
            DEFAULT_MAX_SCHEMA_PATHS
        } else {
            DEFAULT_MAX_SCHEMA_ENTITIES
        }) as usize;

        let (entry, root, catalog) =
            match self.schema_request_target(store_hub, dataset_url, entity_path) {
                Ok(target) => target,
                Err(err) => {
                    on_done.call(Err(err));
                    return;
                }
            };

        self.async_runtime.spawn_future(async move {
            let response = catalog
                .get_asset_schemas(entry.entry_id)
                .await
                .map_err(|err| {
                    ViewerControlError::internal(format!(
                        "failed to fetch the assets of {entry}: {err}"
                    ))
                })
                .and_then(|asset_schemas| {
                    describe_dataset_assets_schema(
                        &asset_schemas,
                        root.as_ref(),
                        max_entities,
                        paths_only,
                    )
                })
                .map(ViewerControlResponse::from);
            on_done.call(response);
        });
    }

    /// The dataset a dataset schema request names, the entity it limits the answer to, and a
    /// client for the dataset's server.
    fn schema_request_target(
        &self,
        store_hub: &StoreHub,
        dataset_url: Option<String>,
        entity_path: Option<String>,
    ) -> Result<
        (
            re_uri::EntryUri,
            Option<EntityPath>,
            re_catalog_client::CatalogClient,
        ),
        ViewerControlError,
    > {
        let entry = self.requested_dataset(store_hub, dataset_url)?;
        let root = parse_schema_root(entity_path)?;
        let catalog = re_catalog_client::CatalogClient::new(
            self.connections
                .registry
                .connection_handle(entry.origin.clone()),
            self.async_runtime.clone(),
        );
        Ok((entry, root, catalog))
    }

    /// Hold `answer` back until `ensure_blueprint` has run, which gives the blueprint target an
    /// active blueprint if it has none yet.
    fn answer_after_blueprint(
        &mut self,
        on_done: UiCallback<Result<ViewerControlResponse, ViewerControlError>>,
        ensure_blueprint: SystemCommand,
        answer: PendingAnswer,
        egui_ctx: &egui::Context,
    ) {
        self.command_sender.send_system(ensure_blueprint);
        self.viewer_control
            .pending_replies
            .push(PendingReply { on_done, answer });
        egui_ctx.request_repaint();
    }

    /// The columns of a table as it was last shown, for the `get_table_schema` operation.
    fn collect_table_schema(
        &self,
        table_url: Option<String>,
        egui_ctx: &egui::Context,
    ) -> Result<GetTableSchemaResponse, ViewerControlError> {
        let table = self.requested_table(table_url)?;
        let columns = re_dataframe_ui::DataFusionTableWidget::table_schema(egui_ctx, &table)
            .ok_or_else(|| {
                ViewerControlError::not_found(format!(
                    "table {table:?} has not been shown yet. Open it with `open` first"
                ))
            })?;

        Ok(GetTableSchemaResponse {
            columns: columns
                .iter()
                .map(|column| TableColumnSchema {
                    name: column.physical_name.to_string(),
                    display_name: column.display_name.clone(),
                    datatype: column.datatype.to_string(),
                })
                .collect(),
        })
    }

    /// Check the filters of a table blueprint against the columns the table last showed.
    ///
    /// Nothing is checked if the table has not been shown yet.
    fn check_table_filters(
        &self,
        table: &re_uri::TableReference,
        chunks: &[re_chunk::Chunk],
    ) -> Result<(), ViewerControlError> {
        use re_sdk_types::blueprint::archetypes::TableBlueprint;
        use re_sdk_types::blueprint::components::SqlFilterExpression;

        let table_entity = EntityPath::from("/table");
        let component = TableBlueprint::descriptor_filters().component;
        let filters = chunks
            .iter()
            .filter(|chunk| chunk.entity_path() == &table_entity)
            .filter_map(|chunk| chunk.component_batch::<SqlFilterExpression>(component, 0))
            .flatten()
            .flatten();

        for filter in filters {
            let sql = filter.as_str();
            if let Some(Err(err)) =
                re_dataframe_ui::DataFusionTableWidget::check_filter(&self.egui_ctx, table, sql)
            {
                return Err(ViewerControlError::invalid_argument(format!(
                    "/table → TableBlueprint → filters: the filter {sql:?} fails: {err}"
                )));
            }
        }
        Ok(())
    }

    /// Queue a write of `chunks` to `blueprint_db`.
    fn queue_blueprint_write(
        &self,
        blueprint_db: &re_entity_db::EntityDb,
        chunks: Vec<re_chunk::Chunk>,
    ) {
        self.command_sender
            .send_system(SystemCommand::AppendToStore(
                blueprint_db.store_id().clone(),
                chunks,
            ));
    }

    /// The active blueprint of `target`.
    fn active_blueprint<'hub>(
        &self,
        store_hub: &'hub StoreHub,
        target: &BlueprintTarget,
    ) -> Result<&'hub re_entity_db::EntityDb, ViewerControlError> {
        match target {
            BlueprintTarget::Recording(store_id) => store_hub
                .active_blueprint_for_app(store_id.application_id())
                .ok_or_else(|| {
                    ViewerControlError::not_found(format!(
                        "recording {store_id} has no active blueprint"
                    ))
                }),
            BlueprintTarget::Table(table) => self
                .table_blueprints
                .active_id(table)
                .and_then(|blueprint_id| store_hub.store_bundle().get(blueprint_id))
                .ok_or_else(|| {
                    ViewerControlError::not_found(format!(
                        "table {table:?} has no active blueprint"
                    ))
                }),
        }
    }

    /// Snapshot a recording's schema for the `get_recording_schema` operation: every entity
    /// with the components ever logged on it, their Arrow datatypes, and whether they are static.
    ///
    /// The schema is purely additive and survives garbage collection, so this describes what the
    /// recording has ever carried rather than what is loaded right now.
    fn collect_recording_schema(
        &self,
        store_hub: &StoreHub,
        request: GetRecordingSchemaRequest,
    ) -> Result<GetRecordingSchemaResponse, ViewerControlError> {
        let GetRecordingSchemaRequest {
            store_id,
            entity_path,
            max_entities,
            with_archetypes,
            with_components,
        } = request;
        let with_archetypes = with_archetypes.unwrap_or(true);
        let with_components = with_components.unwrap_or(false);
        let paths_only = !with_archetypes && !with_components;

        let store_id = store_id
            .map(|store_id| store_id.parse::<StoreId>())
            .transpose()
            .map_err(|err| {
                ViewerControlError::invalid_argument(format!("invalid store_id: {err}"))
            })?
            .or_else(|| self.state.active_recording_id().cloned())
            .ok_or_else(|| {
                ViewerControlError::failed_precondition("no active recording to describe")
            })?;

        let db = store_hub.entity_db(&store_id).ok_or_else(|| {
            ViewerControlError::not_found(format!("recording {store_id} is not open"))
        })?;

        let root = parse_schema_root(entity_path)?;

        let engine = db.storage_engine();
        let columns = engine
            .schema()
            .all_column_metadata()
            .map(|(entity_path, entry)| {
                let component = if paths_only {
                    None
                } else if !with_components {
                    Some(RecordingComponentSchema {
                        archetype: entry.descriptor.archetype.map(|name| name.to_string()),
                        ..Default::default()
                    })
                } else {
                    Some(RecordingComponentSchema {
                        component: entry.descriptor.component.to_string(),
                        archetype: entry.descriptor.archetype.map(|name| name.to_string()),
                        component_type: entry
                            .descriptor
                            .component_type
                            .map(|component_type| component_type.to_string()),
                        datatype: entry.datatype.to_string(),
                        has_static: Some(entry.metadata_state.is_static),
                    })
                };
                (entity_path, component)
            });

        let default_max = if paths_only {
            DEFAULT_MAX_SCHEMA_PATHS
        } else {
            DEFAULT_MAX_SCHEMA_ENTITIES
        };
        let (mut entities, omitted_entities) = group_schema_by_entity(
            columns,
            root.as_ref(),
            max_entities.unwrap_or(default_max) as usize,
        );
        if with_archetypes {
            collect_archetypes(&mut entities);
        }
        if !with_components {
            for entity in &mut entities {
                entity.components.clear();
            }
        }

        // An empty answer to a filtered request reads as "this recording holds no such data",
        // when what actually happened is that the caller named a path the recording never had.
        // A zero `max_entities` empties the list without saying anything about the path, so the
        // omitted count has to be clear too.
        if let Some(root) = &root
            && entities.is_empty()
            && omitted_entities == 0
        {
            return Err(ViewerControlError::not_found(format!(
                "recording {store_id} has nothing at or below {root}"
            )));
        }

        Ok(GetRecordingSchemaResponse {
            store_id: store_id.to_string(),
            timelines: recording_timelines(db, self.state.time_control(&store_id)),
            entities,
            omitted_entities,
        })
    }

    /// Close recordings.
    pub(super) fn apply_close_recordings(
        &self,
        store_hub: &StoreHub,
        target: re_log_channel::CloseRecordingTarget,
    ) -> Result<CloseResponse, String> {
        let to_close = match target {
            re_log_channel::CloseRecordingTarget::All => store_hub
                .store_bundle()
                .recordings()
                .map(|db| db.store_id().clone())
                .collect(),
            re_log_channel::CloseRecordingTarget::Current => vec![
                self.state
                    .active_recording_id()
                    .cloned()
                    .ok_or_else(|| "no active recording to close".to_owned())?,
            ],
            re_log_channel::CloseRecordingTarget::Some(store_ids) => store_ids,
            re_log_channel::CloseRecordingTarget::Sources(sources) => {
                return self.close_recordings_from_sources(store_hub, &sources);
            }
        };
        // Check every id before closing any, so that a failed request closes nothing.
        if let Some(missing) = to_close
            .iter()
            .find(|store_id| store_hub.entity_db(store_id).is_none())
        {
            return Err(format!("no such recording: {missing}"));
        }
        for store_id in &to_close {
            self.command_sender
                .send_system(SystemCommand::CloseRecordingOrTable(
                    store_id.clone().into(),
                ));
        }

        Ok(CloseResponse {
            closed: to_close.iter().map(StoreId::to_string).collect(),
        })
    }

    /// Close the recordings loaded from `sources`, and stop loading from them.
    fn close_recordings_from_sources(
        &self,
        store_hub: &StoreHub,
        sources: &[LogSource],
    ) -> Result<CloseResponse, String> {
        // Check every source before closing any, so that a failed request closes nothing.
        let receivers = self.rx_log.sources();
        if let Some(missing) = sources.iter().find(|source| {
            store_hub
                .recordings_from_sources(std::slice::from_ref(source))
                .is_empty()
                && !receivers
                    .iter()
                    .any(|receiver| receiver.is_same_ignoring_uri_fragments(source))
        }) {
            return Err(format!("Nothing is loaded from {missing}"));
        }

        self.rx_log.retain(|receiver| {
            sources
                .iter()
                .all(|source| !receiver.source().is_same_ignoring_uri_fragments(source))
        });
        let closed = store_hub.recordings_from_sources(sources);
        for store_id in &closed {
            self.command_sender
                .send_system(SystemCommand::CloseRecordingOrTable(
                    store_id.clone().into(),
                ));
        }

        Ok(CloseResponse {
            closed: closed.iter().map(StoreId::to_string).collect(),
        })
    }

    /// The views of the current blueprint with the warnings and errors they reported when last
    /// shown, for the `get_state` operation.
    fn collect_views(
        &mut self,
        store_hub: &StoreHub,
        route: &Route,
        recording_id: Option<&StoreId>,
    ) -> Vec<ViewerView> {
        let Some(blueprint_db) = store_hub.active_blueprint_for_route(route) else {
            return Vec::new();
        };
        let blueprint_query = self.state.blueprint_query_for_viewer(Some(blueprint_db));
        let blueprint =
            re_viewport_blueprint::ViewportBlueprint::from_db(blueprint_db, &blueprint_query);

        blueprint
            .views
            .values()
            .map(|view| {
                let reports = recording_id
                    .into_iter()
                    .flat_map(|recording_id| {
                        self.state.view_states.all_reports(recording_id, view.id)
                    })
                    .filter_map(|diagnostic| {
                        let severity = match diagnostic.severity {
                            re_viewer_context::ViewerReportSeverity::Info => return None,
                            re_viewer_context::ViewerReportSeverity::Warning => "warning",
                            re_viewer_context::ViewerReportSeverity::Error => "error",
                        };
                        Some(ViewerReport {
                            severity: severity.to_owned(),
                            summary: diagnostic.summary.clone(),
                            details: diagnostic.details.clone(),
                        })
                    })
                    .collect();
                ViewerView {
                    view_id: view.id.uuid().to_string(),
                    class: view.class_identifier().to_string(),
                    name: view.display_name_or_default().to_string(),
                    origin: view.space_origin.to_string(),
                    visible: view.visible,
                    reports,
                }
            })
            .collect()
    }

    /// Resolve and apply a time-cursor move for `re_viewer_mcp`'s `SetTimeCursor`.
    ///
    /// Returns what was applied, or an error string if the recording or timeline could not
    /// be resolved.
    pub(super) fn apply_set_time_cursor(
        &self,
        store_hub: &StoreHub,
        store_id: Option<StoreId>,
        timeline: Option<&str>,
        time: Option<i64>,
        play_state: Option<PlayState>,
        egui_ctx: &egui::Context,
    ) -> Result<SetTimeCursorResponse, String> {
        let store_id = store_id
            .or_else(|| self.state.active_recording_id().cloned())
            .ok_or_else(|| "no active recording to set the time for".to_owned())?;

        let db = store_hub
            .entity_db(&store_id)
            .ok_or_else(|| format!("recording {} is not open", store_id.recording_id().as_str()))?;

        let timelines = db.timelines();
        if timelines.is_empty() {
            return Err(format!(
                "recording {} has no timelines yet",
                store_id.recording_id().as_str()
            ));
        }

        // Resolve the target timeline: explicit, else the active one, else the first.
        let timeline_name = if let Some(tl) = timeline {
            let name = TimelineName::try_new(tl).map_err(|err| err.to_string())?;
            if !timelines.contains_key(&name) {
                let available: Vec<&str> = timelines.keys().map(|n| n.as_str()).collect();
                return Err(format!(
                    "recording {} has no timeline {tl:?}; available: {available:?}",
                    store_id.recording_id().as_str()
                ));
            }
            name
        } else {
            let active = self
                .state
                .time_control(&store_id)
                .map(|tc| *tc.timeline_name());
            match active {
                Some(name) if timelines.contains_key(&name) => name,
                _ => *timelines.keys().next().expect("non-empty checked above"),
            }
        };

        let time_type = timelines
            .get(&timeline_name)
            .map_or(TimeType::Sequence, |t| t.typ());

        if time.is_some() && play_state == Some(PlayState::Following) {
            return Err(
                "`play_state` following moves the cursor to the latest data, so it cannot be combined with a `time`"
                    .to_owned(),
            );
        }

        // Only what the request names is sent: a `SetTime` would leave follow mode, and an
        // implied `SetActiveTimeline` would undo a switch still queued by an earlier request.
        // The order of these commands matters.
        let mut time_commands = Vec::new();
        if timeline.is_some() {
            time_commands.push(TimeControlCommand::SetActiveTimeline(timeline_name));
        }
        if let Some(play_state) = play_state {
            time_commands.push(TimeControlCommand::SetPlayState(play_state));
        }
        if let Some(time) = time {
            time_commands.push(TimeControlCommand::SetTime(TimeReal::from(time)));
        }
        if !time_commands.is_empty() {
            self.command_sender
                .send_system(SystemCommand::TimeControlCommands {
                    store_id: store_id.clone(),
                    time_commands,
                });
            re_viewer_context::on_external_blueprint_edit(egui_ctx);
            egui_ctx.request_repaint();
        }

        let time = time.or_else(|| {
            self.state
                .time_control(&store_id)
                .and_then(|time_ctrl| time_ctrl.time_for_timeline(timeline_name))
                .map(|time| time.floor().as_i64())
        });
        Ok(SetTimeCursorResponse {
            store_id: store_id.to_string(),
            timeline: Some(timeline_name.into()),
            time_type: ProtoTimeType::from(time_type) as i32,
            time: time.map(Into::into),
        })
    }
}

/// A recording's timelines with their time ranges, as both `get_state` and
/// `get_recording_schema` report them.
///
/// A timeline with no data yet has no range, which is what tells "still arriving" from "empty".
fn recording_timelines(
    db: &re_entity_db::EntityDb,
    time_ctrl: Option<&TimeControl>,
) -> Vec<ViewerTimeline> {
    db.timelines()
        .values()
        .map(|timeline| {
            let name = timeline.name();
            ViewerTimeline {
                timeline: Some((*name).into()),
                time_type: ProtoTimeType::from(timeline.typ()) as i32,
                time_range: db.time_range_for(name).map(Into::into),
                current_time: time_ctrl
                    .and_then(|time_ctrl| time_ctrl.time_for_timeline(*name))
                    .map(|time| time.floor().as_i64().into()),
            }
        })
        .collect()
}

/// The play state as `get_state` reports it.
fn play_state_to_proto(play_state: PlayState) -> ProtoPlayState {
    match play_state {
        PlayState::Paused => ProtoPlayState::Paused,
        PlayState::Playing => ProtoPlayState::Playing,
        PlayState::Following => ProtoPlayState::Following,
    }
}

/// The play state a `set_time_cursor` request asks for. `None` leaves it as it is.
fn play_state_from_proto(play_state: i32) -> Result<Option<PlayState>, ViewerControlError> {
    match ProtoPlayState::try_from(play_state) {
        Ok(ProtoPlayState::Unspecified) => Ok(None),
        Ok(ProtoPlayState::Paused) => Ok(Some(PlayState::Paused)),
        Ok(ProtoPlayState::Playing) => Ok(Some(PlayState::Playing)),
        Ok(ProtoPlayState::Following) => Ok(Some(PlayState::Following)),
        Err(_) => Err(ViewerControlError::invalid_argument(format!(
            "`play_state` must be paused, playing, or following, got {play_state}"
        ))),
    }
}

/// Group component schemas by entity for the `get_recording_schema` operation.
///
/// Returns the entities in path order, each with its components sorted by name, and how many
/// entities `max_entities` left out.
///
/// Only entities at or below `root` are described; `None` describes the whole recording. A column
/// whose component is `None` names its entity without describing it, which is what `paths_only`
/// asks for.
fn group_schema_by_entity<'a>(
    columns: impl Iterator<Item = (&'a EntityPath, Option<RecordingComponentSchema>)>,
    root: Option<&EntityPath>,
    max_entities: usize,
) -> (Vec<RecordingEntitySchema>, u32) {
    let mut per_entity: BTreeMap<&EntityPath, Vec<RecordingComponentSchema>> = BTreeMap::new();
    for (entity_path, component) in columns {
        // `starts_with` compares whole path parts and is inclusive, so `/world` describes
        // `/world` itself and everything below it, but not the sibling `/world2`.
        if root.is_some_and(|root| !entity_path.starts_with(root)) {
            continue;
        }
        // A `None` still names the entity: `paths_only` drops the components, not the path.
        let entry = per_entity.entry(entity_path).or_default();
        if let Some(component) = component {
            entry.push(component);
        }
    }

    let omitted_entities = per_entity.len().saturating_sub(max_entities) as u32;
    let entities = per_entity
        .into_iter()
        .take(max_entities)
        .map(|(entity_path, mut components)| {
            components.sort_by(|a, b| a.component.cmp(&b.component));
            RecordingEntitySchema {
                entity_path: entity_path.to_string(),
                components,
                archetypes: Vec::new(),
            }
        })
        .collect();

    (entities, omitted_entities)
}

/// Collect distinct archetype names without removing component details or untyped entities.
fn collect_archetypes(entities: &mut [RecordingEntitySchema]) {
    for entity in entities {
        entity.archetypes = entity
            .components
            .iter()
            .filter_map(|component| component.archetype.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }
}

/// The `entity_path` of a schema request, which limits the answer to that entity and below it.
fn parse_schema_root(
    entity_path: Option<String>,
) -> Result<Option<EntityPath>, ViewerControlError> {
    entity_path
        .map(|entity_path| EntityPath::parse_strict(&entity_path))
        .transpose()
        .map_err(|err| ViewerControlError::invalid_argument(format!("invalid entity_path: {err}")))
}

/// Describe a dataset's schema, as the server sends it, for the `get_dataset_schema` operation.
fn describe_dataset_schema(
    schema: &arrow::datatypes::Schema,
    asset_ids: Vec<String>,
    root: Option<&EntityPath>,
    max_entities: usize,
    paths_only: bool,
) -> Result<GetDatasetSchemaResponse, ViewerControlError> {
    let columns = sorbet_columns(schema).map_err(|err| {
        ViewerControlError::internal(format!("the dataset schema cannot be read: {err}"))
    })?;
    let (entities, omitted_entities) =
        describe_schema_entities(&columns, root, max_entities, paths_only);

    if let Some(root) = root
        && entities.is_empty()
        && omitted_entities == 0
    {
        let message = if asset_ids.is_empty() {
            format!("the dataset has nothing at or below {root}")
        } else {
            format!(
                "the segments of the dataset have nothing at or below {root}. It has {}, which `GetDatasetAssetsSchema` describes",
                re_format::format_plural_s(asset_ids.len(), "asset")
            )
        };
        return Err(ViewerControlError::not_found(message));
    }

    Ok(GetDatasetSchemaResponse {
        timelines: columns
            .index_columns()
            .map(|column| column.timeline_name().to_string())
            .collect(),
        entities,
        omitted_entities,
        asset_ids,
    })
}

/// Describe the schemas of a dataset's assets, as the server sends them, for the
/// `get_dataset_assets_schema` operation.
///
/// An asset with nothing at or below `root` is left out.
fn describe_dataset_assets_schema(
    asset_schemas: &[(SegmentId, arrow::datatypes::Schema)],
    root: Option<&EntityPath>,
    max_entities: usize,
    paths_only: bool,
) -> Result<GetDatasetAssetsSchemaResponse, ViewerControlError> {
    let mut assets = Vec::new();
    for (asset_id, schema) in asset_schemas {
        let columns = sorbet_columns(schema).map_err(|err| {
            ViewerControlError::internal(format!(
                "the schema of asset {asset_id} cannot be read: {err}"
            ))
        })?;
        let (entities, omitted_entities) =
            describe_schema_entities(&columns, root, max_entities, paths_only);
        if root.is_none() || !entities.is_empty() || omitted_entities != 0 {
            assets.push(DatasetAssetSchema {
                asset_id: asset_id.to_string(),
                entities,
                omitted_entities,
            });
        }
    }

    if let Some(root) = root
        && assets.is_empty()
    {
        return Err(ViewerControlError::not_found(format!(
            "no asset of the dataset has anything at or below {root}"
        )));
    }

    Ok(GetDatasetAssetsSchemaResponse { assets })
}

fn sorbet_columns(
    schema: &arrow::datatypes::Schema,
) -> Result<re_sorbet::SorbetColumnDescriptors, re_sorbet::SorbetError> {
    re_sorbet::SorbetColumnDescriptors::try_from_arrow_fields(None, schema.fields())
}

/// The entities of a schema at or below `root`, and how many of them `max_entities` left out.
fn describe_schema_entities(
    columns: &re_sorbet::SorbetColumnDescriptors,
    root: Option<&EntityPath>,
    max_entities: usize,
    paths_only: bool,
) -> (Vec<RecordingEntitySchema>, u32) {
    let components = columns.component_columns().map(|column| {
        let component = (!paths_only).then(|| RecordingComponentSchema {
            component: column.component.to_string(),
            archetype: column.archetype.as_ref().map(|name| name.to_string()),
            component_type: column
                .component_type
                .as_ref()
                .map(|component_type| component_type.to_string()),
            datatype: column.inner_datatype().to_string(),
            has_static: Some(column.is_static),
        });
        (&column.entity_path, component)
    });
    group_schema_by_entity(components, root, max_entities)
}

/// Handle one `egui_inspection` request against the running viewer UI.
///
/// This is the viewer-side half of the [`egui_inspection`] protocol: read the accessibility tree,
/// take a screenshot, inject pointer or keyboard events, and so on.
///
/// `re_viewer_mcp` is one client of it: that server turns each of its egui UI tool calls
/// (`click`, `widget_tree`, `screenshot`, …) into such requests and sends them over the
/// `egui_inspect` gRPC operation. Its Rerun-specific tools (`viewer_state`, `set_time`, …) use
/// their own operations and never come here. Any other tool speaking the protocol works too.
///
/// `request` and the `Ok` payload are bare `MessagePack` bodies (`rmp-serde`) of an
/// [`egui_inspection::Request`] and [`egui_inspection::Response`], as produced by
/// [`egui_inspection::protocol::encode_body`] and read by
/// [`egui_inspection::protocol::decode_body`]. They carry no length prefix and no handshake: the
/// transport (gRPC on native, `wasm_bindgen` on web) already frames them, so the TCP framing of
/// the protocol is skipped.
///
/// The response arrives asynchronously via `on_done`, since some requests (screenshots, settling)
/// need one or more frames to complete.
pub(crate) fn serve_egui_inspect_request(
    egui_ctx: &egui::Context,
    request: &[u8],
    on_done: UiCallback<Result<Vec<u8>, InspectError>>,
) {
    use egui_inspection::{InspectionPlugin, Request, protocol};

    let req: Request = match protocol::decode_body(request) {
        Ok(req) => req,
        Err(err) => {
            on_done.call(Err(InspectError::DecodeRequest(err.to_string())));
            return;
        }
    };

    if egui_ctx.plugin_opt::<InspectionPlugin>().is_none() {
        egui_ctx.add_plugin(InspectionPlugin::new(Some("rerun viewer".to_owned())));
    }

    egui_ctx.with_plugin::<InspectionPlugin, _>(|plugin| {
        plugin.submit(req, move |resp| {
            let encoded = protocol::encode_body(&resp)
                .map_err(|err| InspectError::EncodeResponse(err.to_string()));
            on_done.call(encoded);
        });
    });

    egui_ctx.request_repaint();
}

/// The [`ViewerControlError`] a failed screenshot answers with.
fn screenshot_error(err: &SaveScreenshotError) -> ViewerControlError {
    match err {
        SaveScreenshotError::InvalidViewId { .. } => {
            ViewerControlError::invalid_argument(err.to_string())
        }
        SaveScreenshotError::ViewNotFound { .. } => ViewerControlError::not_found(err.to_string()),
        SaveScreenshotError::ViewTooSmall { .. } => {
            ViewerControlError::failed_precondition(err.to_string())
        }
        SaveScreenshotError::InvalidImageData
        | SaveScreenshotError::SaveToPathFailed { .. }
        | SaveScreenshotError::EncodePngFailed { .. } => {
            ViewerControlError::internal(err.to_string())
        }
    }
}

/// Whether any of `url_sources` is still on its initial load: a receiver still loading, or a
/// file still being registered with a catalog.
fn is_loading(
    url_sources: &[LogSource],
    receivers: &[Arc<LogSource>],
    registrations: &BTreeMap<RegistrationId, InFlightRegistration>,
) -> bool {
    let receiver_loading = receivers.iter().any(|receiver| {
        receiver.loading_name().is_some()
            && url_sources
                .iter()
                .any(|source| receiver.is_same_ignoring_uri_fragments(source))
    });
    let file_registering = url_sources.iter().any(|source| match source {
        LogSource::File { path } => {
            let name = path.display().to_string();
            registrations
                .values()
                .any(|registration| registration.name == name)
        }
        _ => false,
    });
    receiver_loading || file_registering
}

#[cfg(test)]
mod tests {
    use re_log_channel::ViewerControlErrorCode;
    use re_protos::viewer_control::v1alpha1::close_request::Target;

    use super::*;

    #[cfg(feature = "testing")]
    mod close_recordings_waits {
        use super::*;

        /// Send `close` for `store_ids`, and return where its answer lands.
        fn close_recordings(
            app: &mut AppCore,
            store_hub: &StoreHub,
            egui_ctx: &egui::Context,
            store_ids: &[&StoreId],
        ) -> std::sync::Arc<
            parking_lot::Mutex<Option<Result<ViewerControlResponse, ViewerControlError>>>,
        > {
            use re_protos::viewer_control::v1alpha1::ViewerRecordingIds;

            let answer = std::sync::Arc::new(parking_lot::Mutex::new(None));
            let request = ViewerControlRequest {
                kind: Some(viewer_control_request::Kind::Close(CloseRequest {
                    target: Some(Target::StoreIds(ViewerRecordingIds {
                        store_ids: store_ids.iter().map(ToString::to_string).collect(),
                    })),
                })),
            };
            app.serve_viewer_control(
                request,
                UiCallback::new({
                    let answer = answer.clone();
                    move |result| *answer.lock() = Some(result)
                }),
                store_hub,
                egui_ctx,
            );
            answer
        }

        /// The caller reads the viewer state as soon as it has its answer, so the answer must wait
        /// for the commands the operation queued.
        #[tokio::test]
        async fn close_recordings_answers_once_the_recording_is_closed() {
            use re_log_types::StoreKind;

            let mut harness = crate::viewer_test_utils::viewer_harness(&Default::default());
            let egui_ctx = harness.ctx.clone();
            let crate::App {
                store_hub,
                core: app,
            } = harness.state_mut();

            let store_id = StoreId::random(StoreKind::Recording, "test_app");
            store_hub.entity_db_entry(&store_id);

            let answer = close_recordings(app, store_hub, &egui_ctx, &[&store_id]);
            assert!(answer.lock().is_none(), "answered before closing");

            app.run_pending_system_commands(store_hub, &egui_ctx);
            assert!(answer.lock().take().is_some_and(|result| result.is_ok()));
            assert!(store_hub.entity_db(&store_id).is_none());
        }

        /// Closing by URL closes only what was loaded from it, and nothing at all if one of the
        /// URLs has nothing loaded.
        #[tokio::test]
        async fn close_by_url_closes_what_was_loaded_from_it() {
            use re_log_types::StoreKind;
            use re_protos::viewer_control::v1alpha1::ViewerSourceUrls;

            let mut harness = crate::viewer_test_utils::viewer_harness(&Default::default());
            let egui_ctx = harness.ctx.clone();
            let crate::App {
                store_hub,
                core: app,
            } = harness.state_mut();

            let mut loaded_from = |path: &str| {
                let store_id = StoreId::random(StoreKind::Recording, "test_app");
                store_hub
                    .entity_db_entry(&store_id)
                    .add_data_source(&std::sync::Arc::new(LogSource::File { path: path.into() }));
                store_id
            };
            let a = loaded_from("a.rrd");
            let b = loaded_from("b.rrd");

            let close = |urls: &[&str]| {
                viewer_control_request::Kind::Close(CloseRequest {
                    target: Some(Target::Urls(ViewerSourceUrls {
                        urls: urls.iter().map(|url| (*url).to_owned()).collect(),
                    })),
                })
            };

            let answer = super::locations::serve(
                app,
                store_hub,
                &egui_ctx,
                close(&["a.rrd", "missing.rrd"]),
            );
            assert_eq!(
                super::locations::error_code(&answer),
                ViewerControlErrorCode::NotFound
            );
            app.run_pending_system_commands(store_hub, &egui_ctx);
            assert!(
                store_hub.entity_db(&a).is_some(),
                "closed despite the error"
            );

            let answer = super::locations::serve(app, store_hub, &egui_ctx, close(&["a.rrd"]));
            app.run_pending_system_commands(store_hub, &egui_ctx);
            assert!(answer.lock().take().is_some_and(|result| result.is_ok()));
            assert!(store_hub.entity_db(&a).is_none());
            assert!(store_hub.entity_db(&b).is_some());
        }

        /// An error answers at once, so it must not leave any recording queued to close.
        #[tokio::test]
        async fn close_recordings_with_a_missing_id_closes_nothing() {
            use re_log_types::StoreKind;

            let mut harness = crate::viewer_test_utils::viewer_harness(&Default::default());
            let egui_ctx = harness.ctx.clone();
            let crate::App {
                store_hub,
                core: app,
            } = harness.state_mut();

            let open = StoreId::random(StoreKind::Recording, "test_app");
            store_hub.entity_db_entry(&open);
            let missing = StoreId::random(StoreKind::Recording, "test_app");

            let answer = close_recordings(app, store_hub, &egui_ctx, &[&open, &missing]);
            assert!(answer.lock().take().is_some_and(|result| result.is_err()));

            app.run_pending_system_commands(store_hub, &egui_ctx);
            assert!(
                store_hub.entity_db(&open).is_some(),
                "closed despite the error"
            );
        }
    }

    #[cfg(feature = "testing")]
    mod locations {
        use re_protos::viewer_control::v1alpha1::viewer_control_response::Kind as ResponseKind;

        use super::*;

        pub type Answer = std::sync::Arc<
            parking_lot::Mutex<Option<Result<ViewerControlResponse, ViewerControlError>>>,
        >;

        /// Serve `kind`, and return where its answer lands.
        pub fn serve(
            app: &mut AppCore,
            store_hub: &StoreHub,
            egui_ctx: &egui::Context,
            kind: viewer_control_request::Kind,
        ) -> Answer {
            let answer = Answer::default();
            app.serve_viewer_control(
                ViewerControlRequest { kind: Some(kind) },
                UiCallback::new({
                    let answer = answer.clone();
                    move |result| *answer.lock() = Some(result)
                }),
                store_hub,
                egui_ctx,
            );
            answer
        }

        fn open_url(url: String, navigate: bool) -> viewer_control_request::Kind {
            viewer_control_request::Kind::Open(OpenRequest {
                url,
                navigate: Some(navigate),
            })
        }

        fn set_location(location: String) -> viewer_control_request::Kind {
            viewer_control_request::Kind::SetLocation(SetLocationRequest { location })
        }

        fn location_of(store_id: &StoreId) -> String {
            ViewerUrl::LocalRecording(store_id.clone())
                .sharable_url(None)
                .expect("a store id always makes a URL")
        }

        pub fn error_code(answer: &Answer) -> ViewerControlErrorCode {
            answer
                .lock()
                .take()
                .and_then(Result::err)
                .expect("answered with an error")
                .code
        }

        fn get_location(
            app: &mut AppCore,
            store_hub: &StoreHub,
            egui_ctx: &egui::Context,
        ) -> String {
            let answer = serve(
                app,
                store_hub,
                egui_ctx,
                viewer_control_request::Kind::GetLocation(GetLocationRequest {}),
            );
            match answer.lock().take() {
                Some(Ok(ViewerControlResponse {
                    kind: Some(ResponseKind::GetLocation(GetLocationResponse { location })),
                })) => location,
                other => panic!("expected a get_location response, got {other:?}"),
            }
        }

        #[tokio::test]
        async fn set_location_shows_a_loaded_recording_and_get_location_reports_it() {
            let mut harness = crate::viewer_test_utils::viewer_harness(&Default::default());
            let egui_ctx = harness.ctx.clone();
            let crate::App {
                store_hub,
                core: app,
            } = harness.state_mut();

            let first = StoreId::random(StoreKind::Recording, "test_app");
            let second = StoreId::random(StoreKind::Recording, "test_app");
            store_hub.entity_db_entry(&first);
            store_hub.entity_db_entry(&second);
            assert_ne!(app.active_recording_id(), Some(&second));

            let answer = serve(
                app,
                store_hub,
                &egui_ctx,
                set_location(location_of(&second)),
            );
            app.run_pending_system_commands(store_hub, &egui_ctx);
            assert!(answer.lock().take().is_some_and(|result| result.is_ok()));
            assert_eq!(app.active_recording_id(), Some(&second));
            assert_eq!(
                get_location(app, store_hub, &egui_ctx),
                location_of(&second)
            );
            assert_eq!(
                app.collect_viewer_state(store_hub).location,
                location_of(&second)
            );
        }

        #[tokio::test]
        async fn open_url_of_an_open_location_navigates_only_if_asked() {
            let mut harness = crate::viewer_test_utils::viewer_harness(&Default::default());
            let egui_ctx = harness.ctx.clone();
            let crate::App {
                store_hub,
                core: app,
            } = harness.state_mut();

            let recording = StoreId::random(StoreKind::Recording, "test_app");
            store_hub.entity_db_entry(&recording);
            let welcome = app.state.navigation.current().clone();

            let answer = serve(
                app,
                store_hub,
                &egui_ctx,
                open_url(location_of(&recording), false),
            );
            app.run_pending_system_commands(store_hub, &egui_ctx);
            assert!(answer.lock().take().is_some_and(|result| result.is_ok()));
            assert_eq!(app.state.navigation.current(), &welcome);

            let answer = serve(
                app,
                store_hub,
                &egui_ctx,
                open_url(location_of(&recording), true),
            );
            app.run_pending_system_commands(store_hub, &egui_ctx);
            assert!(answer.lock().take().is_some_and(|result| result.is_ok()));
            assert_eq!(app.active_recording_id(), Some(&recording));
        }

        #[tokio::test]
        async fn the_welcome_page_location_round_trips() {
            let mut harness = crate::viewer_test_utils::viewer_harness(&Default::default());
            let egui_ctx = harness.ctx.clone();
            let crate::App {
                store_hub,
                core: app,
            } = harness.state_mut();

            let welcome = get_location(app, store_hub, &egui_ctx);
            let answer = serve(app, store_hub, &egui_ctx, set_location(welcome));
            app.run_pending_system_commands(store_hub, &egui_ctx);
            assert!(answer.lock().take().is_some_and(|result| result.is_ok()));
            assert_eq!(app.state.navigation.current(), &Route::welcome_page());
        }

        #[tokio::test]
        async fn get_location_round_trips_through_set_location() {
            let mut harness = crate::viewer_test_utils::viewer_harness(&Default::default());
            let egui_ctx = harness.ctx.clone();
            let crate::App {
                store_hub,
                core: app,
            } = harness.state_mut();

            let answer = serve(
                app,
                store_hub,
                &egui_ctx,
                set_location("about:settings".to_owned()),
            );
            app.run_pending_system_commands(store_hub, &egui_ctx);
            assert!(answer.lock().take().is_some_and(|result| result.is_ok()));

            let location = get_location(app, store_hub, &egui_ctx);
            assert_eq!(location, "about:settings");
            let answer = serve(app, store_hub, &egui_ctx, set_location(location));
            app.run_pending_system_commands(store_hub, &egui_ctx);
            assert!(answer.lock().take().is_some_and(|result| result.is_ok()));

            let Route::Settings { return_route } = app.state.navigation.current() else {
                panic!("expected the settings");
            };
            assert!(
                !matches!(**return_route, Route::Settings { .. }),
                "closing the settings must leave them"
            );
        }

        #[tokio::test]
        async fn a_recording_location_that_is_not_loaded_is_not_found() {
            let mut harness = crate::viewer_test_utils::viewer_harness(&Default::default());
            let egui_ctx = harness.ctx.clone();
            let crate::App {
                store_hub,
                core: app,
            } = harness.state_mut();

            let missing = StoreId::random(StoreKind::Recording, "test_app");
            for kind in [
                open_url(location_of(&missing), true),
                set_location(location_of(&missing)),
            ] {
                let answer = serve(app, store_hub, &egui_ctx, kind);
                assert_eq!(error_code(&answer), ViewerControlErrorCode::NotFound);
            }
        }

        #[tokio::test]
        async fn set_location_never_loads() {
            let mut harness = crate::viewer_test_utils::viewer_harness(&Default::default());
            let egui_ctx = harness.ctx.clone();
            let crate::App {
                store_hub,
                core: app,
            } = harness.state_mut();
            let sources_before = app.rx_log.sources().len();

            let answer = serve(
                app,
                store_hub,
                &egui_ctx,
                set_location("https://example.com/recording.rrd".to_owned()),
            );
            assert_eq!(error_code(&answer), ViewerControlErrorCode::NotFound);
            assert_eq!(app.rx_log.sources().len(), sources_before);

            let answer = serve(
                app,
                store_hub,
                &egui_ctx,
                set_location("not a url".to_owned()),
            );
            assert_eq!(error_code(&answer), ViewerControlErrorCode::InvalidArgument);
        }
    }

    #[cfg(feature = "testing")]
    mod save_screenshot {
        use re_protos::viewer_control::v1alpha1::ViewerControlOp as _;

        use super::locations::{error_code, serve};
        use super::*;

        fn save_screenshot(
            view_id: Option<String>,
            max_size: Option<u32>,
        ) -> viewer_control_request::Kind {
            viewer_control_request::Kind::SaveScreenshot(SaveScreenshotRequest {
                view_id,
                file_path: None,
                max_size,
            })
        }

        /// Ask for a screenshot without a path, and return the PNG it answers with, decoded.
        fn take_png(
            harness: &mut egui_kittest::Harness<'static, crate::App>,
            max_size: Option<u32>,
        ) -> image::DynamicImage {
            let egui_ctx = harness.ctx.clone();
            let crate::App {
                store_hub,
                core: app,
            } = harness.state_mut();
            let answer = serve(app, store_hub, &egui_ctx, save_screenshot(None, max_size));
            crate::viewer_test_utils::step_until("the screenshot answered", harness, |_| {
                answer.lock().is_some()
            });

            let response = answer
                .lock()
                .take()
                .and_then(Result::ok)
                .and_then(|response| SaveScreenshotRequest::from_envelope(response).ok())
                .expect("a save_screenshot response");
            image::load_from_memory_with_format(
                &response.png.expect("a PNG without a file_path"),
                image::ImageFormat::Png,
            )
            .expect("a valid PNG")
        }

        #[tokio::test]
        async fn without_a_path_answers_with_a_png() {
            let mut harness = crate::viewer_test_utils::viewer_harness(&Default::default());

            let full = take_png(&mut harness, None);
            assert!(
                64 < full.width().max(full.height()),
                "{}x{}",
                full.width(),
                full.height()
            );

            let small = take_png(&mut harness, Some(64));
            assert_eq!(small.width().max(small.height()), 64);
        }

        #[tokio::test]
        async fn a_missing_view_fails() {
            let mut harness = crate::viewer_test_utils::viewer_harness(&Default::default());
            let egui_ctx = harness.ctx.clone();
            let crate::App {
                store_hub,
                core: app,
            } = harness.state_mut();

            let view_id = uuid::Uuid::new_v4().to_string();
            let answer = serve(
                app,
                store_hub,
                &egui_ctx,
                save_screenshot(Some(view_id), None),
            );
            app.run_pending_system_commands(store_hub, &egui_ctx);

            assert_eq!(error_code(&answer), ViewerControlErrorCode::NotFound);
        }
    }

    #[cfg(feature = "testing")]
    mod set_time_cursor {
        use std::sync::Arc;

        use re_chunk::{Chunk, RowId, TimePoint, Timeline};
        use re_log_types::StoreKind;

        use super::locations::{error_code, serve};
        use super::*;

        /// A loaded recording with data at frames 0 and 10.
        fn recording(store_hub: &mut StoreHub) -> StoreId {
            let store_id = StoreId::random(StoreKind::Recording, "test_app");
            let db = store_hub.entity_db_entry(&store_id);
            for frame in [0, 10] {
                let chunk = Chunk::builder("points")
                    .with_archetype(
                        RowId::new(),
                        TimePoint::default().with(Timeline::new_sequence("frame"), frame),
                        &re_sdk_types::archetypes::Points3D::new([(0.0, 0.0, 0.0)]),
                    )
                    .build()
                    .expect("a valid chunk");
                db.add_chunk(&Arc::new(chunk)).expect("a valid chunk");
            }
            store_id
        }

        fn set_time_cursor(
            store_id: &StoreId,
            time: Option<i64>,
            play_state: Option<ProtoPlayState>,
        ) -> viewer_control_request::Kind {
            viewer_control_request::Kind::SetTimeCursor(SetTimeCursorRequest {
                store_id: Some(store_id.to_string()),
                timeline: None,
                time: time.map(Into::into),
                play_state: play_state.map(|play_state| play_state as i32),
            })
        }

        #[tokio::test]
        async fn following_sticks_and_an_empty_request_changes_nothing() {
            let mut harness = crate::viewer_test_utils::viewer_harness(&Default::default());
            let egui_ctx = harness.ctx.clone();
            let crate::App {
                store_hub,
                core: app,
            } = harness.state_mut();
            let store_id = recording(store_hub);

            for request in [
                set_time_cursor(&store_id, None, Some(ProtoPlayState::Following)),
                set_time_cursor(&store_id, None, None),
            ] {
                let answer = serve(app, store_hub, &egui_ctx, request);
                app.run_pending_system_commands(store_hub, &egui_ctx);
                assert!(answer.lock().take().is_some_and(|result| result.is_ok()));
                assert_eq!(
                    app.state
                        .time_control(&store_id)
                        .map(TimeControl::play_state),
                    Some(PlayState::Following)
                );
            }
        }

        #[tokio::test]
        async fn following_cannot_be_combined_with_a_time() {
            let mut harness = crate::viewer_test_utils::viewer_harness(&Default::default());
            let egui_ctx = harness.ctx.clone();
            let crate::App {
                store_hub,
                core: app,
            } = harness.state_mut();
            let store_id = recording(store_hub);

            let answer = serve(
                app,
                store_hub,
                &egui_ctx,
                set_time_cursor(&store_id, Some(3), Some(ProtoPlayState::Following)),
            );
            assert_eq!(error_code(&answer), ViewerControlErrorCode::InvalidArgument);
        }
    }

    #[test]
    fn an_omitted_close_target_is_refused() {
        let err = close_recordings_target(CloseRequest { target: None })
            .expect_err("an unset target names no recording");
        assert_eq!(err.code, ViewerControlErrorCode::InvalidArgument);
    }

    /// One temporal component named `component` on `entity_path`.
    fn column(entity_path: &EntityPath, component: &str) -> (EntityPath, RecordingComponentSchema) {
        (
            entity_path.clone(),
            RecordingComponentSchema {
                component: component.to_owned(),
                archetype: None,
                component_type: None,
                datatype: "Float32".to_owned(),
                has_static: Some(false),
            },
        )
    }

    #[test]
    fn a_schema_subtree_takes_whole_path_parts() {
        let world = EntityPath::from("/world");
        let columns = [
            column(&world, "a"),
            column(&EntityPath::from("/world/robot"), "b"),
            column(&EntityPath::from("/world2"), "c"),
            column(&EntityPath::from("/elsewhere"), "d"),
        ];

        let (entities, omitted) = group_schema_by_entity(
            columns
                .iter()
                .map(|(path, schema)| (path, Some(schema.clone()))),
            Some(&world),
            10,
        );

        let paths: Vec<&str> = entities.iter().map(|e| e.entity_path.as_str()).collect();
        assert_eq!(paths, ["/world", "/world/robot"]);
        assert_eq!(omitted, 0);
    }

    /// A zero limit empties the list while the subtree is perfectly real, which is what tells
    /// `collect_recording_schema` apart from a path the recording never had.
    /// `paths_only` names an entity while saying nothing about it, so a column with no component
    /// still has to produce the entity it belongs to.
    #[test]
    fn paths_only_keeps_the_entity_and_drops_its_components() {
        let world = EntityPath::from("/world");
        let robot = EntityPath::from("/world/robot");

        let (entities, omitted) = group_schema_by_entity(
            [(&world, None), (&robot, None), (&robot, None)].into_iter(),
            None,
            10,
        );

        let paths: Vec<&str> = entities.iter().map(|e| e.entity_path.as_str()).collect();
        assert_eq!(paths, ["/world", "/world/robot"]);
        assert!(entities.iter().all(|e| e.components.is_empty()));
        assert_eq!(omitted, 0);
    }

    /// Checks that compact summaries retain untyped entities and respect subtree limits.
    #[test]
    fn archetype_summary_is_unique_and_keeps_untyped_entities_and_limits() {
        let root = EntityPath::from("/world");
        let typed = EntityPath::from("/world/a");
        let untyped = EntityPath::from("/world/b");
        let omitted = EntityPath::from("/world/c");
        let outside = EntityPath::from("/world2");
        let columns = [
            (&typed, Some("rerun.archetypes.Points3D")),
            (&typed, Some("rerun.archetypes.Points3D")),
            (&typed, Some("rerun.archetypes.CoordinateFrame")),
            (&untyped, None),
            (&omitted, Some("rerun.archetypes.Image")),
            (&outside, Some("rerun.archetypes.TextLog")),
        ];
        let (mut entities, omitted_count) = group_schema_by_entity(
            columns.into_iter().map(|(path, archetype)| {
                let (_, mut component) = column(path, "component");
                component.archetype = archetype.map(str::to_owned);
                (path, Some(component))
            }),
            Some(&root),
            2,
        );
        // Archetype collection preserves component details for callers requesting both.
        collect_archetypes(&mut entities);
        assert_eq!(omitted_count, 1);
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].entity_path, "/world/a");
        assert_eq!(
            entities[0].archetypes,
            [
                "rerun.archetypes.CoordinateFrame",
                "rerun.archetypes.Points3D"
            ]
        );
        assert_eq!(entities[1].entity_path, "/world/b");
        assert!(entities[1].archetypes.is_empty());
        assert_eq!(entities[0].components.len(), 3);
        assert_eq!(entities[1].components.len(), 1);
    }

    #[test]
    fn a_zero_limit_still_counts_a_matching_subtree() {
        let world = EntityPath::from("/world");
        let columns = [column(&world, "a")];

        let (entities, omitted) = group_schema_by_entity(
            columns
                .iter()
                .map(|(path, schema)| (path, Some(schema.clone()))),
            Some(&world),
            0,
        );

        assert!(entities.is_empty());
        assert_eq!(omitted, 1);
    }

    #[test]
    fn a_truncated_schema_reports_what_it_left_out() {
        let paths: Vec<EntityPath> = (0..5).map(|i| EntityPath::from(format!("/e{i}"))).collect();
        let columns: Vec<_> = paths.iter().map(|path| column(path, "a")).collect();

        let (entities, omitted) = group_schema_by_entity(
            columns
                .iter()
                .map(|(path, schema)| (path, Some(schema.clone()))),
            None,
            2,
        );

        let kept: Vec<&str> = entities.iter().map(|e| e.entity_path.as_str()).collect();
        assert_eq!(kept, ["/e0", "/e1"]);
        assert_eq!(omitted, 3);

        // Asking for nothing still says how much there was.
        let (entities, omitted) = group_schema_by_entity(
            columns
                .iter()
                .map(|(path, schema)| (path, Some(schema.clone()))),
            None,
            0,
        );
        assert!(entities.is_empty());
        assert_eq!(omitted, 5);
    }

    /// A dataset schema with one static component on each of `entity_paths`.
    fn sorbet_schema(entity_paths: &[&str]) -> arrow::datatypes::Schema {
        let fields: Vec<_> = entity_paths
            .iter()
            .map(|entity_path| {
                re_sorbet::ComponentColumnDescriptor {
                    store_datatype: arrow::datatypes::DataType::new_list(
                        arrow::datatypes::DataType::Float32,
                        true,
                    ),
                    component_type: None,
                    entity_path: EntityPath::from(*entity_path),
                    archetype: None,
                    component: "a".into(),
                    is_static: true,
                    is_tombstone: false,
                    is_semantically_empty: false,
                }
                .to_arrow_field(re_sorbet::BatchType::Dataframe)
            })
            .collect();
        arrow::datatypes::Schema::new_with_metadata(fields, Default::default())
    }

    /// An entity path below which only one asset has entities describes that asset alone. A path
    /// no asset has is not found.
    #[test]
    fn an_assets_schema_subtree_leaves_out_other_assets() {
        let assets = [
            (
                SegmentId::from("robot"),
                sorbet_schema(&["/world/robot/mesh"]),
            ),
            (SegmentId::from("table"), sorbet_schema(&["/world/table"])),
        ];
        let robot = EntityPath::from("/world/robot");

        let response = describe_dataset_assets_schema(&assets, Some(&robot), 10, false)
            .expect("the asset has the path");

        let asset_ids: Vec<&str> = response
            .assets
            .iter()
            .map(|a| a.asset_id.as_str())
            .collect();
        assert_eq!(asset_ids, ["robot"]);
        assert_eq!(
            response.assets[0].entities[0].entity_path,
            "/world/robot/mesh"
        );

        let err =
            describe_dataset_assets_schema(&assets, Some(&EntityPath::from("/nowhere")), 10, false)
                .expect_err("no asset has the path");
        assert_eq!(err.code, ViewerControlErrorCode::NotFound);
    }

    /// Without an entity path, every asset is described, even one without entities.
    #[test]
    fn an_assets_schema_describes_every_asset() {
        let assets = [
            (
                SegmentId::from("robot"),
                sorbet_schema(&["/world/robot/mesh"]),
            ),
            (SegmentId::from("empty"), sorbet_schema(&[])),
        ];

        let response =
            describe_dataset_assets_schema(&assets, None, 10, true).expect("no path to miss");

        let asset_ids: Vec<&str> = response
            .assets
            .iter()
            .map(|a| a.asset_id.as_str())
            .collect();
        assert_eq!(asset_ids, ["robot", "empty"]);
    }

    /// A path the segments do not have is not found, and the error names the assets tool when
    /// the dataset has assets.
    #[test]
    fn a_dataset_schema_miss_points_at_the_assets() {
        let dataset = sorbet_schema(&["/world/camera"]);
        let robot = EntityPath::from("/world/robot");

        let err =
            describe_dataset_schema(&dataset, vec!["robot".to_owned()], Some(&robot), 10, false)
                .expect_err("the segments do not have the path");
        assert_eq!(err.code, ViewerControlErrorCode::NotFound);
        assert!(err.message.contains("GetDatasetAssetsSchema"));

        let err = describe_dataset_schema(&dataset, Vec::new(), Some(&robot), 10, false)
            .expect_err("the segments do not have the path");
        assert!(!err.message.contains("GetDatasetAssetsSchema"));
    }

    #[test]
    fn a_false_close_selector_is_refused() {
        for target in [Target::Current(false), Target::All(false)] {
            let err = close_recordings_target(CloseRequest {
                target: Some(target),
            })
            .expect_err("a false selector says nothing about what to close");

            assert_eq!(err.code, ViewerControlErrorCode::InvalidArgument);
        }
    }

    #[test]
    fn closing_a_url_that_loads_no_recording_is_refused() {
        let err = close_recordings_target(CloseRequest {
            target: Some(Target::Urls(ViewerSourceUrls {
                urls: vec!["rerun+http://localhost:51234".to_owned()],
            })),
        })
        .expect_err("a server URL loads no recording");
        assert_eq!(err.code, ViewerControlErrorCode::InvalidArgument);
    }
}
