//! The viewer-side implementation of the `ViewerControlService` API, defined by
//! `viewer_control.proto`: state snapshots, time-cursor moves, closing recordings, and
//! `egui_inspection` requests.
//!
//! That file lists every place an operation has to be added.

use std::collections::BTreeMap;

use re_chunk::TimelineName;
use re_log_channel::{
    CloseRecordingTarget, InspectError, SaveScreenshotError, UiCallback, ViewerControlError,
};
use re_log_types::{EntityPath, StoreId, TimeReal, TimeType};
use re_protos::common::v1alpha1::TimeType as ProtoTimeType;
use re_protos::viewer_control::v1alpha1::{
    CloseRecordingsRequest, CloseRecordingsResponse, GetRecordingSchemaRequest,
    GetRecordingSchemaResponse, GetViewerLogsRequest, GetViewerLogsResponse,
    GetViewerStateResponse, OpenUrlRequest, OpenUrlResponse, RecordingComponentSchema,
    RecordingEntitySchema, SaveScreenshotRequest, SaveScreenshotResponse, SetTimeCursorRequest,
    SetTimeCursorResponse, TimeCursor, ViewerControlRequest, ViewerControlResponse,
    ViewerLoadingSource, ViewerRecording, ViewerReport, ViewerTimeline, ViewerView,
    viewer_control_request,
};
use re_sdk_types::external::uuid;
use re_viewer_context::{
    Route, StoreHub, SystemCommand, SystemCommandSender as _, TimeControlCommand,
    open_url::{OpenUrlOptions, ViewerOpenUrl},
};

use super::App;

/// How many entities `get_recording_schema` describes when the request does not say.
///
/// A recording of a few thousand entities would otherwise answer with more than the caller can
/// read. The response reports how many entities were left out, so a truncated answer is visibly
/// truncated rather than silently wrong.
const DEFAULT_MAX_SCHEMA_ENTITIES: u32 = 100;

/// How many entities `get_recording_schema` names when the request asks for `paths_only`.
///
/// Far higher than [`DEFAULT_MAX_SCHEMA_ENTITIES`], because a path costs one line where the
/// components on it cost many — which is the whole reason to ask for paths alone.
const DEFAULT_MAX_SCHEMA_PATHS: u32 = 10_000;

impl App {
    /// Run one operation of the `ViewerControlService` API on the UI thread.
    ///
    /// Every transport lands here: the request arrives decoded, and `on_done` carries the matching
    /// response or a coded failure back to whoever asked. Only `save_screenshot` answers later
    /// than this call, once the image has been written.
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
            Kind::CloseRecordings(request) => {
                let result = close_recordings_target(request)
                    .and_then(|target| {
                        self.apply_close_recordings(store_hub, target)
                            .map_err(ViewerControlError::not_found)
                    })
                    .map(ViewerControlResponse::from);
                on_done.call(result);
            }
            Kind::GetViewerLogs(GetViewerLogsRequest { after_sequence }) => {
                on_done.call(Ok(GetViewerLogsResponse {
                    entries: self.viewer_log.entries_after(after_sequence),
                }
                .into()));
            }
            Kind::GetRecordingSchema(request) => {
                on_done.call(
                    self.collect_recording_schema(store_hub, request)
                        .map(ViewerControlResponse::from),
                );
            }
            Kind::GetViewerState(_) => {
                on_done.call(Ok(self.collect_viewer_state(store_hub).into()));
            }
            Kind::OpenUrl(OpenUrlRequest { url }) => {
                let parsed = ViewerOpenUrl::parse_with_options(
                    &url,
                    &re_data_source::FromUriOptions {
                        accept_extensionless_http: true,
                    },
                );
                match parsed {
                    Ok(open_url) => {
                        open_url.open(egui_ctx, &OpenUrlOptions::default(), &self.command_sender);
                        on_done.call(Ok(OpenUrlResponse {}.into()));
                    }
                    Err(err) => {
                        on_done.call(Err(ViewerControlError::invalid_argument(format!(
                            "Failed to open URL {url:?}: {err}"
                        ))));
                    }
                }
            }

            Kind::SaveScreenshot(request) => self.begin_screenshot(request, on_done),

            Kind::SetTimeCursor(request) => {
                let SetTimeCursorRequest {
                    store_id,
                    timeline,
                    time,
                    play,
                } = request;
                // `time` is required. Defaulting it would seek to the start of the recording,
                // which is a move the caller did not ask for, so every transport must be refused
                // here rather than only in the clients that happen to validate.
                let time = time.ok_or_else(|| {
                    ViewerControlError::invalid_argument("`time` is required by `set_time_cursor`")
                });
                let store_id = store_id
                    .map(|store_id| store_id.parse::<StoreId>())
                    .transpose()
                    .map_err(|err| {
                        ViewerControlError::invalid_argument(format!("invalid store_id: {err}"))
                    });
                let result = time
                    .and_then(|time| Ok((time, store_id?)))
                    .and_then(|(time, store_id)| {
                        self.apply_set_time_cursor(
                            store_hub,
                            store_id,
                            timeline.map(|timeline| timeline.name).as_deref(),
                            time.time,
                            play.unwrap_or(false),
                            egui_ctx,
                        )
                        .map_err(ViewerControlError::invalid_argument)
                    })
                    .map(ViewerControlResponse::from);
                on_done.call(result);
            }
        }
    }

    /// Ask for a screenshot, and answer `on_done` once it has been written.
    ///
    /// The capture needs at least one more frame, so the callback is parked until then. Wrapping
    /// it here keeps the screenshot plumbing typed in terms of its own error.
    fn begin_screenshot(
        &mut self,
        request: SaveScreenshotRequest,
        on_done: UiCallback<Result<ViewerControlResponse, ViewerControlError>>,
    ) {
        let SaveScreenshotRequest { view_id, file_path } = request;

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

        let file_path: camino::Utf8PathBuf = file_path.into();
        self.pending_screenshot_notifiers.insert(
            file_path.clone(),
            UiCallback::new(move |result: Result<(), SaveScreenshotError>| {
                on_done.call(match result {
                    Ok(()) => Ok(SaveScreenshotResponse {}.into()),
                    Err(err @ SaveScreenshotError::InvalidViewId { .. }) => {
                        Err(ViewerControlError::invalid_argument(err.to_string()))
                    }
                    Err(err @ SaveScreenshotError::ViewNotFound { .. }) => {
                        Err(ViewerControlError::not_found(err.to_string()))
                    }
                    Err(err @ SaveScreenshotError::ViewTooSmall { .. }) => {
                        Err(ViewerControlError::failed_precondition(err.to_string()))
                    }
                    Err(
                        err @ (SaveScreenshotError::InvalidImageData
                        | SaveScreenshotError::SaveToPathFailed { .. }),
                    ) => Err(ViewerControlError::internal(err.to_string())),
                });
            }),
        );

        self.command_sender
            .send_system(SystemCommand::SaveScreenshot {
                target: re_viewer_context::ScreenshotTarget::SaveToPath(file_path),
                view_id,
                notify: false,
            });
    }
}

/// Which recordings a `close_recordings` request selects.
fn close_recordings_target(
    request: CloseRecordingsRequest,
) -> Result<CloseRecordingTarget, ViewerControlError> {
    use re_protos::viewer_control::v1alpha1::close_recordings_request::Target;

    Ok(match request.target {
        // A `oneof` is an enum: an unset target names no recording, so there is nothing to
        // default to without closing something the caller did not ask for.
        None => {
            return Err(ViewerControlError::invalid_argument(
                "`close_recordings` takes exactly one of `current`, `all` or `store_ids`",
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
    })
}

impl App {
    /// Snapshot the current viewer state for the `get_viewer_state` operation:
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
                let timelines = recording_timelines(db);

                let current_time = self
                    .state
                    .time_control(store_id)
                    .map(|time_ctrl| TimeCursor {
                        timeline: Some((*time_ctrl.timeline_name()).into()),
                        time_type: time_ctrl.time_type().map(|t| ProtoTimeType::from(t) as i32),
                        time: time_ctrl.time_int().map(|t| t.as_i64().into()),
                    });

                ViewerRecording {
                    store_id: store_id.to_string(),
                    timelines,
                    current_time,
                }
            })
            .collect();

        // `loading_name` is the viewer's own "actively loading" predicate — the one the welcome
        // screen shows a spinner for — so a source that is only waiting to be sent data is left
        // out, as it would otherwise never clear.
        let loading = self
            .rx_log
            .sources()
            .iter()
            .filter_map(|source| {
                source.loading_name().map(|name| ViewerLoadingSource {
                    name,
                    status: source.status_string(),
                })
            })
            .collect();

        GetViewerStateResponse {
            url,
            active_store_id: active_id.map(|store_id| store_id.to_string()),
            recordings,
            loading,
            views,
            catalog_url: self
                .connection_registry
                .internal_origin()
                .map(|origin| origin.to_string()),
            viewer_version: Some(self.build_info.version.to_string()),
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
            paths_only,
        } = request;
        let paths_only = paths_only.unwrap_or(false);

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

        let root = entity_path
            .map(|entity_path| EntityPath::parse_strict(&entity_path))
            .transpose()
            .map_err(|err| {
                ViewerControlError::invalid_argument(format!("invalid entity_path: {err}"))
            })?;

        let engine = db.storage_engine();
        let columns = engine
            .schema()
            .all_column_metadata()
            .map(|(entity_path, entry)| {
                let component = (!paths_only).then(|| RecordingComponentSchema {
                    component: entry.descriptor.component.to_string(),
                    archetype: entry.descriptor.archetype.map(|name| name.to_string()),
                    component_type: entry
                        .descriptor
                        .component_type
                        .map(|component_type| component_type.to_string()),
                    datatype: entry.datatype.to_string(),
                    has_static: Some(entry.metadata_state.is_static),
                });
                (entity_path, component)
            });

        let default_max = if paths_only {
            DEFAULT_MAX_SCHEMA_PATHS
        } else {
            DEFAULT_MAX_SCHEMA_ENTITIES
        };
        let (entities, omitted_entities) = group_schema_by_entity(
            columns,
            root.as_ref(),
            max_entities.unwrap_or(default_max) as usize,
        );

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
            timelines: recording_timelines(db),
            entities,
            omitted_entities,
        })
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
            closed: to_close.iter().map(StoreId::to_string).collect(),
        })
    }

    /// The views of the current blueprint with the warnings and errors they reported when last
    /// shown, for the `get_viewer_state` operation.
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
            store_id: store_id.to_string(),
            timeline: Some(timeline_name.into()),
            time_type: ProtoTimeType::from(time_type) as i32,
            time: Some(time.into()),
        })
    }
}

/// A recording's timelines with their time ranges, as both `get_viewer_state` and
/// `get_recording_schema` report them.
///
/// A timeline with no data yet has no range, which is what tells "still arriving" from "empty".
fn recording_timelines(db: &re_entity_db::EntityDb) -> Vec<ViewerTimeline> {
    db.timelines()
        .values()
        .map(|timeline| {
            let name = timeline.name();
            ViewerTimeline {
                timeline: Some((*name).into()),
                time_type: ProtoTimeType::from(timeline.typ()) as i32,
                time_range: db.time_range_for(name).map(Into::into),
            }
        })
        .collect()
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
            }
        })
        .collect();

    (entities, omitted_entities)
}

/// Handle one `egui_inspection` request against the running viewer UI.
///
/// This is the viewer-side half of the [`egui_inspection`] protocol: read the accessibility tree,
/// take a screenshot, inject pointer or keyboard events, and so on.
///
/// `re_viewer_mcp` is one client of it: that server turns each of its egui UI tool calls
/// (`click`, `query_tree`, `screenshot`, …) into such requests and sends them over the
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

#[cfg(test)]
mod tests {
    use re_log_channel::ViewerControlErrorCode;
    use re_protos::viewer_control::v1alpha1::close_recordings_request::Target;

    use super::*;

    #[test]
    fn an_omitted_close_target_is_refused() {
        let err = close_recordings_target(CloseRecordingsRequest { target: None })
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

    #[test]
    fn a_false_close_selector_is_refused() {
        for target in [Target::Current(false), Target::All(false)] {
            let err = close_recordings_target(CloseRecordingsRequest {
                target: Some(target),
            })
            .expect_err("a false selector says nothing about what to close");

            assert_eq!(err.code, ViewerControlErrorCode::InvalidArgument);
        }
    }
}
