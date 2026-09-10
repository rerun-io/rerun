//! The viewer-side implementation of the `ViewerControlService` RPCs that `re_viewer_mcp` uses:
//! state snapshots, time-cursor moves, closing recordings, and `egui_inspection` requests.

use re_chunk::TimelineName;
use re_log_channel::{InspectError, UiCallback};
use re_log_types::{StoreId, TimeReal, TimeType};
use re_protos::common::v1alpha1::TimeType as ProtoTimeType;
use re_protos::sdk_comms::v1alpha1::{
    CloseRecordingsResponse, GetViewerStateResponse, SetTimeCursorResponse, TimeCursor,
    ViewerRecording, ViewerReport, ViewerTimeline, ViewerView,
};
use re_viewer_context::{
    Route, StoreHub, SystemCommand, SystemCommandSender as _, TimeControlCommand,
    open_url::ViewerOpenUrl,
};

use super::App;

impl App {
    /// Snapshot the current viewer state for `re_viewer_mcp`'s `GetViewerState`:
    /// the active recording, the current page as a sharable URL, the catalog the viewer hosts,
    /// and every open recording's timelines with their time ranges and current time cursor.
    pub(super) fn collect_viewer_state(&mut self, store_hub: &StoreHub) -> GetViewerStateResponse {
        let active_id = self.state.active_recording_id().cloned();
        let route = self.state.navigation.current().clone();
        let views = self.collect_views(store_hub, &route, active_id.as_ref());
        let route = &route;

        // Best-effort sharable URL for the current page; some routes (e.g. local tables) can't be
        // turned into a URL, in which case we leave it empty.
        let url = ViewerOpenUrl::from_route(store_hub, route)
            .and_then(|open_url| open_url.sharable_url(None))
            .unwrap_or_default();

        let recordings = store_hub
            .store_bundle()
            .recordings()
            .map(|db| {
                let store_id = db.store_id();
                let timelines = db
                    .timelines()
                    .values()
                    .map(|timeline| {
                        let name = timeline.name();
                        let range = db.time_range_for(name);
                        ViewerTimeline {
                            timeline: Some((*name).into()),
                            time_type: ProtoTimeType::from(timeline.typ()) as i32,
                            time_range: range.map(Into::into),
                        }
                    })
                    .collect();

                let current_time = self
                    .state
                    .time_control(store_id)
                    .map(|time_ctrl| TimeCursor {
                        timeline: Some((*time_ctrl.timeline_name()).into()),
                        time_type: time_ctrl.time_type().map(|t| ProtoTimeType::from(t) as i32),
                        time: time_ctrl.time_int().map(|t| t.as_i64().into()),
                    });

                ViewerRecording {
                    store_id: Some(store_id.clone().into()),
                    timelines,
                    current_time,
                }
            })
            .collect();

        GetViewerStateResponse {
            url,
            active_store_id: active_id.map(Into::into),
            recordings,
            views,
            catalog_url: self
                .connection_registry
                .internal_origin()
                .map(|origin| origin.to_string()),
        }
    }

    /// Close recordings.
    pub(super) fn apply_close_recordings(
        &self,
        store_hub: &StoreHub,
        target: re_log_channel::CloseRecordingTarget,
    ) -> Result<CloseRecordingsResponse, String> {
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
        };
        for store_id in &to_close {
            if store_hub.entity_db(store_id).is_none() {
                return Err(format!("no such recording: {store_id}"));
            }
            self.command_sender
                .send_system(SystemCommand::CloseRecordingOrTable(
                    store_id.clone().into(),
                ));
        }

        Ok(CloseRecordingsResponse {
            closed: to_close.into_iter().map(Into::into).collect(),
        })
    }

    /// The views of the current blueprint with the warnings and errors they reported when last
    /// shown, for `re_viewer_mcp`'s `GetViewerState`.
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
        time: i64,
        play: bool,
        egui_ctx: &egui::Context,
    ) -> Result<SetTimeCursorResponse, String> {
        use re_sdk_types::blueprint::components::PlayState;

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

        let play_state = if play {
            PlayState::Playing
        } else {
            PlayState::Paused
        };

        // The order of these commands matters.
        let time_commands = vec![
            TimeControlCommand::SetActiveTimeline(timeline_name),
            TimeControlCommand::SetPlayState(play_state),
            TimeControlCommand::SetTime(TimeReal::from(time)),
        ];

        self.command_sender
            .send_system(SystemCommand::TimeControlCommands {
                store_id: store_id.clone(),
                time_commands,
            });
        egui_ctx.request_repaint();

        Ok(SetTimeCursorResponse {
            store_id: Some(store_id.into()),
            timeline: Some(timeline_name.into()),
            time_type: ProtoTimeType::from(time_type) as i32,
            time: Some(time.into()),
        })
    }
}

/// Handle one `egui_inspection` request against the running viewer UI.
///
/// This is the viewer-side half of the [`egui_inspection`] protocol: read the accessibility tree,
/// take a screenshot, inject pointer or keyboard events, and so on.
///
/// `re_viewer_mcp` is one client of it: that server turns each of its egui UI tool calls
/// (`click`, `query_tree`, `screenshot`, …) into such requests and sends them over the `Inspect`
/// gRPC RPC. Its Rerun-specific tools (`viewer_state`, `set_time`, …) use their own RPCs and
/// never come here. Any other tool speaking the protocol works too.
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
pub(crate) fn serve_inspect_request(
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
