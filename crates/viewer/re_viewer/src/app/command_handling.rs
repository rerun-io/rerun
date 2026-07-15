use anyhow::Context as _;
use itertools::Itertools as _;
use re_build_info::CrateVersion;
use re_chunk::TimelineName;
use re_entity_db::{EntityDb, LogSource};
use re_log_channel::RecordingOpenBehavior;
use re_log_types::{ApplicationId, LogMsg, RecordingId, StoreId, StoreKind};
use re_sdk_types::blueprint::components::PlayState;
use re_ui::{RecordingCommand, UICommand, UICommandSender as _};
use re_viewer_context::open_url::{OpenUrlOptions, ViewerOpenUrl};
use re_viewer_context::{
    ActiveStoreContext, AppBlueprintCtx, NeedsRepaint, Route, StorageContext, StoreHub,
    SystemCommand, open_url::combine_with_base_url,
};
use re_viewer_context::{
    MoveDirection, MoveSpeed, RecordingOrTable, SystemCommandSender as _, TimeControlCommand,
    sanitize_file_name,
};
use std::sync::Arc;

use super::App;
use crate::{app_blueprint::AppBlueprint, event::ViewerEventDispatcher};

#[cfg(not(target_arch = "wasm32"))]
const MIN_ZOOM_FACTOR: f32 = 0.2;
#[cfg(not(target_arch = "wasm32"))]
const MAX_ZOOM_FACTOR: f32 = 5.0;

/// How [`App::close_recording`] should treat a recording that's still rendered as a preview.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CloseRecording {
    /// A recording still rendered as a preview stays loaded and streaming, just no longer in the
    /// recording list.
    KeepPreview,

    /// Fully remove the recording even if it's still rendered as a preview.
    /// Used when the recording's server is going away, so leaving it loaded makes no sense.
    Force,
}

impl App {
    pub(super) fn run_pending_system_commands(
        &mut self,
        store_hub: &mut StoreHub,
        egui_ctx: &egui::Context,
    ) {
        re_tracing::profile_function!();
        while let Some((from_where, cmd)) = self.command_receiver.recv_system() {
            self.run_system_command(from_where, cmd, store_hub, egui_ctx);
        }
    }

    pub(super) fn run_pending_ui_commands(
        &mut self,
        egui_ctx: &egui::Context,
        app_blueprint: &AppBlueprint<'_>,
        storage_context: &StorageContext<'_>,
        store_context: Option<&ActiveStoreContext<'_>>,
        route: &Route,
    ) {
        re_tracing::profile_function!();
        while let Some(cmd) = self.command_receiver.recv_ui() {
            self.run_ui_command(
                egui_ctx,
                app_blueprint,
                storage_context,
                store_context,
                route,
                cmd,
            );
        }
    }

    fn run_system_command(
        &mut self,
        sent_from: &std::panic::Location<'_>, // Who sent this command? Useful for debugging!
        cmd: SystemCommand,
        store_hub: &mut StoreHub,
        egui_ctx: &egui::Context,
    ) {
        re_tracing::profile_function!(cmd.debug_name());

        match cmd {
            SystemCommand::TimeControlCommands {
                store_id,
                time_commands,
            } => {
                match store_id.kind() {
                    StoreKind::Recording => {
                        let usage = store_hub.usage(&store_id);

                        if usage.was_preview()
                            && let Some(preview_state) = &mut self.state.view_states.preview_state
                            && let Some(time_control) =
                                preview_state.recording_time_control_mut(&store_id)
                            && let Some(db) = store_hub.entity_db(&store_id)
                        {
                            let response = time_control.handle_time_commands(
                                None::<&AppBlueprintCtx<'_>>,
                                db,
                                &time_commands,
                            );

                            if response.needs_repaint == NeedsRepaint::Yes {
                                self.egui_ctx.request_repaint();
                            }

                            return;
                        }

                        store_hub.load_blueprint_and_caches(&store_id, &self.view_class_registry); // Ensure caches and blueprints
                        store_hub.ensure_active_blueprint_for_app(store_id.application_id()); // Materialize the target blueprint on-demand

                        let Some(target_blueprint) =
                            store_hub.active_blueprint_for_app(store_id.application_id())
                        else {
                            re_log::debug_panic!(
                                "No active blueprint found for recording {store_id:?} when handling time control commands sent from {sent_from}. This should never happen for local recording routes.",
                            );
                            re_log::error_once!(
                                "Can't change time for recording {store_id:?} because it is not active."
                            );
                            return;
                        };

                        let blueprint_query = self
                            .state
                            .blueprint_query_for_viewer(Some(target_blueprint));

                        let blueprint_ctx = AppBlueprintCtx {
                            command_sender: &self.command_sender,
                            current_blueprint: target_blueprint,
                            default_blueprint: store_hub
                                .default_blueprint_for_app(store_id.application_id()),
                            blueprint_query,
                        };

                        let Some(recording) = store_hub.entity_db(&store_id) else {
                            re_log::error_once!(
                                "Can't change time for recording {store_id:?} because it is not loaded."
                            );
                            return;
                        };

                        let time_ctrl = self.state.time_control_mut(recording, &blueprint_ctx);

                        let response = time_ctrl.handle_time_commands(
                            Some(&blueprint_ctx),
                            recording,
                            &time_commands,
                        );

                        if response.needs_repaint == NeedsRepaint::Yes {
                            self.egui_ctx.request_repaint();
                        }

                        handle_time_ctrl_event(
                            recording,
                            self.event_dispatcher.as_ref(),
                            &response,
                        );
                    }
                    StoreKind::Blueprint => {
                        if let Some(target_store) = store_hub.store_bundle().get(&store_id) {
                            let blueprint_ctx: Option<&AppBlueprintCtx<'_>> = None;
                            let response = self.state.blueprint_time_control.handle_time_commands(
                                blueprint_ctx,
                                target_store,
                                &time_commands,
                            );

                            if response.needs_repaint == NeedsRepaint::Yes {
                                self.egui_ctx.request_repaint();
                            }
                        }
                    }
                }
            }
            SystemCommand::SetUrlFragment { store_id, fragment } => {
                // This adds new system commands, which will be handled later in the loop.
                self.go_to_dataset_data(store_id, fragment);
            }
            SystemCommand::CopyViewerUrl(url) => {
                if cfg!(target_arch = "wasm32") {
                    match combine_with_base_url(
                        self.startup_options.web_viewer_base_url().as_ref(),
                        [url],
                    ) {
                        Ok(url) => {
                            self.copy_text(url);
                        }
                        Err(err) => {
                            re_log::error!("{err}");
                        }
                    }
                } else {
                    self.copy_text(url);
                }
            }
            SystemCommand::ActivateApp(app_id) => {
                store_hub.load_persisted_blueprints_for_app(&app_id);
                if let Some(recording_id) = store_hub.earliest_recording_for_app(&app_id) {
                    store_hub.load_blueprint_and_caches(&recording_id, &self.view_class_registry);
                    self.state
                        .navigation
                        .replace(Route::LocalRecording { recording_id });
                } else {
                    // TODO(RR-3713): show a blueprint for it anyway
                    re_log::warn_once!("Can't switch app-id - we have no recording for it");
                    // If we can't go where we want to go, then go nowhere.
                }
            }

            SystemCommand::CloseApp(app_id) => {
                store_hub.close_app(&app_id);
            }

            SystemCommand::CloseRecordingOrTable(entry) => {
                // The active recording we're closing, if that's what this is. When set, we move off
                // it after closing.
                let active_being_closed = match &entry {
                    RecordingOrTable::Recording { store_id }
                        if self.state.active_recording_id() == Some(store_id) =>
                    {
                        Some(store_id.clone())
                    }
                    _ => None,
                };

                let new_navigation = active_being_closed.as_ref().and_then(|closing| {
                    // Look back through history for the closest entry that's still an open destination.
                    let back_target = self
                        .state
                        .history
                        .find_back(|url| {
                            self.is_back_destination_open(store_hub, url, Some(closing))
                        })
                        .cloned();

                    back_target.or_else(|| {
                        ViewerOpenUrl::from_route(
                            store_hub,
                            &Self::fallback_route_after_close(store_hub, closing),
                        )
                        .ok()
                    })
                });

                self.close_recording(store_hub, &entry, CloseRecording::KeepPreview);

                if let Some(new_navigation) = new_navigation {
                    self.navigate_to(egui_ctx, &new_navigation);
                } else if active_being_closed.is_some() {
                    self.state.navigation.reset();
                }
            }

            SystemCommand::CloseAllEntries => {
                self.state.navigation.reset();
                store_hub.clear_entries();

                // Stop receiving into the old recordings.
                // This is most important when going back to the example screen by using the "Back"
                // button in the browser, and there is still a connection downloading an .rrd.
                // That's the case of `LogSource::HttpStream`.
                // TODO(emilk): exactly what things get kept and what gets cleared?
                self.rx_log.retain(|r| match r.source() {
                    LogSource::File { .. } | LogSource::HttpStream { .. } => false,

                    LogSource::JsChannel { .. }
                    | LogSource::RrdWebEvent
                    | LogSource::Sdk
                    | LogSource::RedapGrpcStream { .. }
                    | LogSource::MessageProxy { .. }
                    | LogSource::Stdin => true,
                });
            }

            SystemCommand::AddReceiver(rx) => {
                re_log::debug!("Received AddReceiver");
                self.add_log_receiver(rx);
            }

            SystemCommand::SetRoute(new_route) => {
                if &new_route == self.state.navigation.current() {
                    return;
                }

                self.state.view_states.preview_state = None;

                // Suppress loading screen if we're loading a recording that's already loaded, even if only partially.
                if let Route::Loading(source) = &new_route
                    && let Some(re_uri::RedapUri::DatasetData(dataset_uri)) = source.redap_uri()
                    && store_hub
                        .store_bundle()
                        .entity_dbs()
                        .any(|db| db.store_id() == &dataset_uri.store_id())
                {
                    return;
                }

                if let Some(recording_id) = new_route.recording_id() {
                    store_hub.set_opened(recording_id, true);
                    store_hub.load_blueprint_and_caches(recording_id, &self.view_class_registry);
                    // If we're navigating to a recording that was only ever a preview, fetch the
                    // blueprint we skipped while previewing it.
                    self.fetch_pending_blueprint(store_hub, recording_id);
                }

                if matches!(new_route, Route::Loading(_)) {
                    self.state
                        .selection_state
                        .set_selection(re_viewer_context::ItemCollection::default());
                }

                self.state.navigation.replace(new_route);

                egui_ctx.request_repaint(); // Make sure we actually see the new mode.
            }

            SystemCommand::OpenSettings => {
                self.state.navigation.replace(Route::Settings {
                    return_route: Box::new(self.state.navigation.current().clone()),
                });

                #[cfg(feature = "analytics")]
                re_analytics::record(|| re_analytics::event::SettingsOpened {});
            }

            SystemCommand::OpenChunkStoreBrowser {
                store_id,
                selected_chunk,
            } => match self.state.navigation.current() {
                Route::ChunkStoreBrowser {
                    store_id: current_store_id,
                    return_route,
                    ..
                } => {
                    self.state.navigation.replace(Route::ChunkStoreBrowser {
                        // History/share URLs may carry an explicit store; otherwise keep
                        // using the current chunk browser store context.
                        store_id: store_id.or_else(|| current_store_id.clone()),
                        selected_chunk,
                        return_route: return_route.clone(),
                    });
                }
                current => {
                    self.state.navigation.replace(Route::ChunkStoreBrowser {
                        store_id: store_id.or_else(|| current.recording_id().cloned()),
                        selected_chunk,
                        return_route: Box::new(current.clone()),
                    });
                }
            },

            SystemCommand::ResetRoute => {
                self.state.navigation.reset();

                egui_ctx.request_repaint(); // Make sure we actually see the new mode.
            }

            SystemCommand::AddRedapServer(origin) => {
                if origin == *re_redap_browser::EXAMPLES_ORIGIN {
                    return;
                }
                if self.state.redap_servers.has_server(&origin) {
                    return;
                }

                self.state.redap_servers.add_server(origin.clone());

                if self.state.navigation.current().recording_id().is_none() {
                    self.state.navigation.replace(Route::RedapServer(origin));
                }
                self.command_sender.send_ui(UICommand::ExpandBlueprintPanel);
            }

            SystemCommand::RefreshRedapServer(origin) => {
                // Only refresh servers we already know about; adding a new server already fetches
                // its catalog, so there's nothing to refresh in that case.
                if self.state.redap_servers.has_server(&origin) {
                    self.state
                        .redap_servers
                        .send_command(re_redap_browser::Command::RefreshCollection(origin));
                }
            }

            SystemCommand::RefreshRedapEntry { origin, entry_id } => {
                self.state
                    .redap_servers
                    .refresh_entry(&origin, entry_id, egui_ctx);
            }

            SystemCommand::RemoveRedapServer(origin) => {
                // Clearing blueprints must happen before closing the recordings (so we can know
                // what to close)
                store_hub.clear_blueprints_for_origin(&origin);

                // Close any recordings streaming from this server, otherwise their
                // still-open connections keep emitting "Failed to connect to remote
                // data source" warnings.
                let recordings_to_close: Vec<_> = store_hub
                    .store_bundle()
                    .recordings_for_origin(&origin)
                    .map(|db| db.store_id().clone())
                    .collect();

                // Were we viewing one of the recordings we're about to close? Then we need to move
                // off it once the server is gone.
                let viewing_closed_recording = self
                    .state
                    .active_recording_id()
                    .is_some_and(|active| recordings_to_close.contains(active));

                // Close the recordings before removing the server, to avoid a race.
                // `Force` because a recording rendered as a preview last frame would otherwise stay
                // loaded and streaming even though its server is being removed.
                for store_id in recordings_to_close {
                    self.close_recording(store_hub, &store_id.into(), CloseRecording::Force);
                }

                self.state
                    .redap_servers
                    .remove_server(&origin, &self.connection_registry);

                let current_route = self.state.navigation.current();
                let on_removed_server = match current_route {
                    Route::RedapServer(route_origin)
                    | Route::RedapEntry {
                        origin: route_origin,
                        ..
                    } => route_origin == &origin,
                    _ => false,
                };

                if on_removed_server || viewing_closed_recording {
                    if let Some(url) = self
                        .state
                        .history
                        .find_back(|url| self.is_back_destination_open(store_hub, url, None))
                        .cloned()
                    {
                        self.navigate_to(egui_ctx, &url);
                    } else {
                        self.state.navigation.reset();
                    }
                }
            }

            SystemCommand::EditRedapServerModal(command) => {
                self.state.redap_servers.open_edit_server_modal(command);
            }

            SystemCommand::RedapServer(command) => {
                let re_ui::RedapServerCommand { origin, kind } = command;
                match kind {
                    re_ui::RedapServerCommandKind::Refresh => {
                        self.command_sender
                            .send_system(SystemCommand::RefreshRedapServer(origin));
                    }
                    re_ui::RedapServerCommandKind::Edit => {
                        self.command_sender
                            .send_system(SystemCommand::EditRedapServerModal(
                                re_viewer_context::EditRedapServerModalCommand::new(origin),
                            ));
                    }
                    re_ui::RedapServerCommandKind::CopyUrl => {
                        let url = origin.to_string();
                        re_log::info!("Copied {url:?} to clipboard");
                        egui_ctx.copy_text(url);
                    }
                    re_ui::RedapServerCommandKind::Remove => {
                        self.command_sender
                            .send_system(SystemCommand::RemoveRedapServer(origin));
                    }
                }
            }

            SystemCommand::Table(command) => {
                let re_ui::TableCommand {
                    origin,
                    entry_id,
                    kind,
                } = command;
                match kind {
                    re_ui::TableCommandKind::Refresh => {
                        self.state
                            .redap_servers
                            .refresh_entry(&origin, entry_id, egui_ctx);
                    }
                }
            }

            SystemCommand::LoadDataSource(data_source) => {
                self.load_data_source(store_hub, egui_ctx, &data_source);
            }

            SystemCommand::ResetViewer => self.reset_viewer(store_hub, egui_ctx),
            SystemCommand::ClearActiveBlueprintAndEnableHeuristics => {
                re_log::debug!("Clear and generate new blueprint");
                store_hub.clear_active_blueprint_and_generate(self.state.navigation.current());
                egui_ctx.request_repaint(); // Many changes take a frame delay to show up.
            }
            SystemCommand::ClearActiveBlueprint => {
                // By clearing the blueprint the default blueprint will be restored
                // at the beginning of the next frame.
                re_log::debug!("Reset blueprint to default");
                store_hub.clear_active_blueprint(self.state.navigation.current());
                egui_ctx.request_repaint(); // Many changes take a frame delay to show up.
            }

            SystemCommand::AppendToStore(store_id, chunks) => {
                re_log::trace!(
                    "{}:{} Update {} entities: {}",
                    sent_from.file(),
                    sent_from.line(),
                    store_id.kind(),
                    chunks.iter().map(|c| c.entity_path()).join(", ")
                );

                let db = store_hub.entity_db_entry(&store_id);

                // No need to clear undo buffer if we're just appending static data.
                //
                // It would be nice to be able to undo edits to a recording, but
                // we haven't implemented that yet.
                if store_id.is_blueprint() && chunks.iter().any(|c| !c.is_static()) {
                    self.state
                        .blueprint_undo_state
                        .entry(store_id.clone())
                        .or_default()
                        .clear_redo_buffer(db);

                    if self.app_options().inspect_blueprint_timeline {
                        self.command_sender
                            .send_system(SystemCommand::TimeControlCommands {
                                store_id,
                                time_commands: vec![TimeControlCommand::SetPlayState(
                                    PlayState::Following,
                                )],
                            });
                    }
                }

                for chunk in chunks {
                    match db.add_chunk(&Arc::new(chunk)) {
                        Ok(_store_events) => {}
                        Err(err) => {
                            re_log::warn_once!("Failed to append chunk: {err}");
                        }
                    }
                }
            }

            SystemCommand::UndoBlueprint { blueprint_id } => {
                let inspect_blueprint_timeline = self.app_options().inspect_blueprint_timeline;
                let blueprint_db = store_hub.entity_db_entry(&blueprint_id);
                let undo_state = self
                    .state
                    .blueprint_undo_state
                    .entry(blueprint_id.clone())
                    .or_default();

                undo_state.undo(blueprint_db);

                // Update blueprint inspector timeline.
                if inspect_blueprint_timeline {
                    if let Some(redo_time) = undo_state.redo_time() {
                        self.command_sender
                            .send_system(SystemCommand::TimeControlCommands {
                                store_id: blueprint_id,
                                time_commands: vec![
                                    TimeControlCommand::SetPlayState(PlayState::Paused),
                                    TimeControlCommand::SetTime(redo_time.into()),
                                ],
                            });
                    } else {
                        self.command_sender
                            .send_system(SystemCommand::TimeControlCommands {
                                store_id: blueprint_id,
                                time_commands: vec![TimeControlCommand::SetPlayState(
                                    PlayState::Following,
                                )],
                            });
                    }
                }
            }
            SystemCommand::RedoBlueprint { blueprint_id } => {
                let inspect_blueprint_timeline = self.app_options().inspect_blueprint_timeline;
                let undo_state = self
                    .state
                    .blueprint_undo_state
                    .entry(blueprint_id.clone())
                    .or_default();

                undo_state.redo();

                // Update blueprint inspector timeline.
                if inspect_blueprint_timeline {
                    if let Some(redo_time) = undo_state.redo_time() {
                        self.command_sender
                            .send_system(SystemCommand::TimeControlCommands {
                                store_id: blueprint_id,
                                time_commands: vec![
                                    TimeControlCommand::SetPlayState(PlayState::Paused),
                                    TimeControlCommand::SetTime(redo_time.into()),
                                ],
                            });
                    } else {
                        self.command_sender
                            .send_system(SystemCommand::TimeControlCommands {
                                store_id: blueprint_id,
                                time_commands: vec![TimeControlCommand::SetPlayState(
                                    PlayState::Following,
                                )],
                            });
                    }
                }
            }

            SystemCommand::DropEntity(blueprint_id, entity_path) => {
                let blueprint_db = store_hub.entity_db_entry(&blueprint_id);
                blueprint_db.drop_entity_path_recursive(&entity_path);
            }

            #[cfg(debug_assertions)]
            SystemCommand::EnableInspectBlueprintTimeline(show) => {
                self.app_options_mut().inspect_blueprint_timeline = show;
            }

            SystemCommand::SetSelection(set) => {
                if let Some(item) = set.selection.single_item() {
                    // If the selected item has its own page, switch to it.
                    if let Some(route) = Route::from_item(item) {
                        if let Route::LocalRecording { recording_id } = &route {
                            store_hub
                                .load_blueprint_and_caches(recording_id, &self.view_class_registry);
                        }
                        self.state.navigation.replace(route);
                    }
                }

                self.state.selection_state.set_selection(set);
                egui_ctx.request_repaint(); // Make sure we actually see the new selection.
            }

            SystemCommand::SetFocus(item) => {
                self.state.focused_item = Some(item);
            }

            SystemCommand::ShowNotification(notification) => {
                self.notifications.add(notification);
            }

            SystemCommand::ReadbackAndSaveTexture { texture, action } => {
                self.texture_readback.push(texture, action);
            }

            #[cfg(not(target_arch = "wasm32"))]
            SystemCommand::FileSaver(file_saver) => {
                if let Err(err) = self.background_tasks.spawn_file_saver(file_saver) {
                    re_log::error!("Failed to save file: {err}");
                }
            }

            SystemCommand::OnAuthChanged(auth) => {
                self.state.auth_state = auth;
            }

            SystemCommand::SetAuthCredentials {
                access_token,
                email,
            } => {
                let credentials =
                    match re_auth::oauth::Credentials::try_new(access_token, None, email) {
                        Ok(credentials) => credentials,
                        Err(err) => {
                            re_log::error!("Failed to create credentials: {err}");
                            return;
                        }
                    };
                if let Err(err) = credentials.ensure_stored() {
                    re_log::error!("Failed to store credentials: {err}");
                }
            }
            SystemCommand::Logout => {
                let signed_out_url = self
                    .startup_options
                    .login
                    .as_ref()
                    .map(|l| l.signed_out_url.as_str());
                match re_auth::oauth::clear_credentials(signed_out_url) {
                    Ok(Some(outcome)) => {
                        // Open the WorkOS logout URL to also end the browser session.
                        // This opens in a new tab/window so the viewer state is preserved.
                        // WorkOS clears its session cookies and redirects to /signed-out.
                        egui_ctx.open_url(egui::output::OpenUrl {
                            url: outcome.logout_url,
                            new_tab: true,
                        });
                    }
                    Ok(None) => {
                        re_log::debug!("No session to logout from");
                    }
                    Err(err) => {
                        re_log::error!("Failed to logout: {err}");
                    }
                }
                let logged_out_origins = self.state.redap_servers.logout();

                // Close any open recordings that came from the logged-out servers.
                store_hub.retain_recordings(|db| {
                    let Some(data_source) = &db.data_source else {
                        return true;
                    };
                    match data_source {
                        LogSource::RedapGrpcStream { uri, .. } => {
                            !logged_out_origins.contains(&uri.origin)
                        }
                        _ => true,
                    }
                });

                // Also stop receiving data from those servers.
                self.rx_log.retain(|r| match r.source() {
                    LogSource::RedapGrpcStream { uri, .. } => {
                        !logged_out_origins.contains(&uri.origin)
                    }
                    _ => true,
                });
            }
            SystemCommand::SaveScreenshot {
                target,
                view_id,
                notify,
            } => {
                if let Some(view_id) = view_id {
                    // Screenshot a specific view
                    if let Some(view_info) = self.egui_ctx.memory_mut(|mem| {
                        mem.caches
                            .cache::<re_viewer_context::ViewRectPublisher>()
                            .get(&view_id)
                            .cloned()
                    }) {
                        let re_viewer_context::PublishedViewInfo { name, rect } = view_info;
                        let rect = rect.shrink(2.5); // Hacky: Shrink so we don't accidentally include the border of the view.
                        if !rect.is_positive() {
                            re_log::warn!("View too small for a screenshot");
                            return;
                        }

                        self.egui_ctx
                            .send_viewport_cmd(egui::ViewportCommand::Screenshot(
                                egui::UserData::new(re_viewer_context::ScreenshotInfo {
                                    ui_rect: Some(rect),
                                    pixels_per_point: self.egui_ctx.pixels_per_point(),
                                    name,
                                    target,
                                    notify,
                                }),
                            ));
                    } else {
                        re_log::warn!("View {view_id} not found for screenshot");
                    }
                } else {
                    // Screenshot the entire viewer
                    self.egui_ctx
                        .send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(
                            re_viewer_context::ScreenshotInfo {
                                ui_rect: None,
                                pixels_per_point: self.egui_ctx.pixels_per_point(),
                                name: "screenshot".to_owned(),
                                target,
                                notify,
                            },
                        )));
                }

                // Screenshot commands may be triggered from receiving messages over the network, so we may not actually do any painting right now.
                // Make sure we do at least once, so the screenshot gets saved out.
                self.egui_ctx.request_repaint();

                // TODO(#12481): Depending on the platform we a request repaint alone isn't enough to wake up the viewer.
                // For now we do a focus switch but this isn't ideal since it breaks the flow of programmatic screenshot taking.
                self.egui_ctx
                    .send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }
    }

    fn run_ui_command(
        &mut self,
        egui_ctx: &egui::Context,
        app_blueprint: &AppBlueprint<'_>,
        storage_context: &StorageContext<'_>,
        store_context: Option<&ActiveStoreContext<'_>>,
        route: &Route,
        cmd: UICommand,
    ) {
        let mut force_store_info = false;
        let active_store_id = store_context
            .map(|ctx| ctx.recording_store_id().clone())
            // Don't redirect data to the welcome screen.
            .filter(|store_id| store_id.application_id() != StoreHub::welcome_screen_app_id())
            .unwrap_or_else(|| {
                // If we don't have any application ID to recommend (which means we are on the welcome screen),
                // then just generate a new one using a UUID.
                let application_id = ApplicationId::random();

                // NOTE: We don't override blueprints' store IDs anyhow, so it is sound to assume that
                // this can only be a recording.
                let recording_id = RecordingId::random();

                // We're creating a recording just-in-time, directly from the viewer.
                // We need those store infos or the data will just be silently ignored.
                force_store_info = true;

                StoreId::recording(application_id, recording_id)
            });

        match cmd {
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Open => {
                use re_data_source::LogDataSource;
                use re_log_types::FileSource;
                for file_path in open_file_dialog_native(self.main_thread_token) {
                    self.command_sender
                        .send_system(SystemCommand::LoadDataSource(LogDataSource::FilePath {
                            file_source: FileSource::FileDialog {
                                recommended_store_id: None,
                                force_store_info,
                            },
                            path: file_path,
                        }));
                }
            }
            #[cfg(target_arch = "wasm32")]
            UICommand::Open => {
                let egui_ctx = egui_ctx.clone();

                let promise = poll_promise::Promise::spawn_local(async move {
                    let file = async_open_rrd_dialog().await;
                    egui_ctx.request_repaint(); // Wake ui thread
                    file
                });

                self.open_files_promise = Some(super::PendingFilePromise {
                    recommended_store_id: None,
                    force_store_info,
                    promise,
                });
            }

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Import => {
                use re_data_source::LogDataSource;
                use re_log_types::FileSource;
                for file_path in open_file_dialog_native(self.main_thread_token) {
                    self.command_sender
                        .send_system(SystemCommand::LoadDataSource(LogDataSource::FilePath {
                            file_source: FileSource::FileDialog {
                                recommended_store_id: Some(active_store_id.clone()),
                                force_store_info,
                            },
                            path: file_path,
                        }));
                }
            }
            #[cfg(target_arch = "wasm32")]
            UICommand::Import => {
                let egui_ctx = egui_ctx.clone();

                let promise = poll_promise::Promise::spawn_local(async move {
                    let file = async_open_rrd_dialog().await;
                    egui_ctx.request_repaint(); // Wake ui thread
                    file
                });

                self.open_files_promise = Some(super::PendingFilePromise {
                    recommended_store_id: Some(active_store_id.clone()),
                    force_store_info,
                    promise,
                });
            }

            UICommand::OpenUrl => {
                self.state.open_url_modal.open();
            }

            UICommand::CloseAllEntries => {
                self.command_sender
                    .send_system(SystemCommand::CloseAllEntries);
            }

            UICommand::NextRecording => {
                self.state
                    .recording_panel
                    .send_command(re_recording_panel::RecordingPanelCommand::SelectNextRecording);
            }
            UICommand::PreviousRecording => {
                self.state.recording_panel.send_command(
                    re_recording_panel::RecordingPanelCommand::SelectPreviousRecording,
                );
            }

            UICommand::NavigateBack => {
                if let Some(url) = self.state.history.go_back() {
                    url.clone().open(
                        egui_ctx,
                        &OpenUrlOptions {
                            recording_open_behavior: RecordingOpenBehavior::OpenAndSelect,
                            show_loader: true,
                        },
                        &self.command_sender,
                    );
                }
            }
            UICommand::NavigateForward => {
                if let Some(url) = self.state.history.go_forward() {
                    url.clone().open(
                        egui_ctx,
                        &OpenUrlOptions {
                            recording_open_behavior: RecordingOpenBehavior::OpenAndSelect,
                            show_loader: true,
                        },
                        &self.command_sender,
                    );
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::Quit => {
                egui_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }

            UICommand::OpenWebsite => {
                egui_ctx.open_url(egui::output::OpenUrl {
                    url: "https://rerun.io/".to_owned(),
                    new_tab: true,
                });
            }
            UICommand::OpenWebHelp => {
                egui_ctx.open_url(egui::output::OpenUrl {
                    url: "https://rerun.io/docs/getting-started/navigating-the-viewer".to_owned(),
                    new_tab: true,
                });
            }
            UICommand::OpenRerunDiscord => {
                egui_ctx.open_url(egui::output::OpenUrl {
                    url: "https://discord.gg/PXtCgFBSmH".to_owned(),
                    new_tab: true,
                });
            }

            UICommand::ResetViewer => self.command_sender.send_system(SystemCommand::ResetViewer),
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::OpenProfiler => {
                self.profiler.start();
            }

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::CaptureProfileTrace => {
                if self.profile_capture.is_none() {
                    self.profile_capture = Some(re_tracing::ProfileCapture::start(5));
                    egui_ctx.request_repaint();
                }
            }

            UICommand::ToggleDevPanel => {
                self.dev_panel_open ^= true;
            }
            UICommand::TogglePanelStateOverrides => {
                self.panel_state_overrides_active ^= true;
            }
            UICommand::ToggleTopPanel => {
                app_blueprint.toggle_top_panel(&self.command_sender);
            }
            UICommand::ToggleBlueprintPanel => {
                app_blueprint.toggle_blueprint_panel(&self.command_sender);
            }
            UICommand::ExpandBlueprintPanel => {
                if !app_blueprint.blueprint_panel_state().is_expanded() {
                    app_blueprint.toggle_blueprint_panel(&self.command_sender);
                }
            }
            UICommand::ToggleSelectionPanel => {
                app_blueprint.toggle_selection_panel(&self.command_sender);
            }
            UICommand::ExpandSelectionPanel => {
                if !app_blueprint.selection_panel_state().is_expanded() {
                    app_blueprint.toggle_selection_panel(&self.command_sender);
                }
            }
            #[cfg(debug_assertions)]
            UICommand::ToggleEguiDebugPanel => {
                self.egui_debug_panel_open ^= true;
            }

            UICommand::ToggleFullscreen => {
                self.toggle_fullscreen();
            }

            UICommand::Settings => {
                self.command_sender.send_system(SystemCommand::OpenSettings);
            }

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ZoomIn => {
                let mut zoom_factor = egui_ctx.zoom_factor();
                zoom_factor += 0.1;
                zoom_factor = zoom_factor.clamp(MIN_ZOOM_FACTOR, MAX_ZOOM_FACTOR);
                zoom_factor = (zoom_factor * 10.).round() / 10.;
                egui_ctx.set_zoom_factor(zoom_factor);
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ZoomOut => {
                let mut zoom_factor = egui_ctx.zoom_factor();
                zoom_factor -= 0.1;
                zoom_factor = zoom_factor.clamp(MIN_ZOOM_FACTOR, MAX_ZOOM_FACTOR);
                zoom_factor = (zoom_factor * 10.).round() / 10.;
                egui_ctx.set_zoom_factor(zoom_factor);
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ZoomReset => {
                egui_ctx.set_zoom_factor(1.0);
            }

            UICommand::ToggleCommandPalette => {
                self.cmd_palette.toggle();
            }

            #[cfg(not(target_arch = "wasm32"))]
            UICommand::ScreenshotWholeApp => {
                self.screenshotter.request_screenshot(egui_ctx);
            }
            #[cfg(debug_assertions)]
            UICommand::ResetEguiMemory => {
                egui_ctx.memory_mut(|mem| *mem = Default::default());

                // re-apply style, which is lost when resetting memory
                re_ui::apply_style_and_install_loaders(egui_ctx);
            }

            UICommand::Share => {
                let selection = self.state.selection_state.selected_items();
                let rec_cfg = route
                    .recording_id()
                    .and_then(|id| self.state.time_controls.get(id));
                if let Err(err) =
                    self.state
                        .share_modal
                        .open(storage_context.hub, route, rec_cfg, selection)
                {
                    re_log::error!("Cannot share link to current screen: {err}");
                }
            }
            UICommand::CopyDirectLink => {
                match ViewerOpenUrl::from_route(storage_context.hub, route) {
                    Ok(url) => self.run_copy_link_command(&url),
                    Err(err) => re_log::error!("{err}"),
                }
            }

            UICommand::CopyTimeSelectionLink => {
                match ViewerOpenUrl::from_route(storage_context.hub, route) {
                    Ok(mut url) => {
                        if let Some(fragment) = url.fragment_mut() {
                            let time_ctrl = route
                                .recording_id()
                                .and_then(|id| self.state.time_control(id));

                            if let Some(time_ctrl) = &time_ctrl
                                && let Some(time_selection) = time_ctrl.time_selection()
                                && let Some(timeline) = time_ctrl.timeline()
                            {
                                fragment.time_selection = Some(re_uri::TimeSelection {
                                    timeline: *timeline,
                                    range: time_selection.to_int(),
                                });
                            } else {
                                re_log::warn!("No timeline selection to copy");
                            }
                        } else {
                            re_log::warn!(
                                "The current recording doesn't support sharing a time range"
                            );
                        }

                        self.run_copy_link_command(&url);
                    }
                    Err(err) => re_log::error!("{err}"),
                }
            }

            #[cfg(target_arch = "wasm32")]
            UICommand::RestartWithWebGl => {
                if crate::web_tools::set_url_parameter_and_refresh("renderer", "webgl").is_err() {
                    re_log::error!("Failed to set URL parameter `renderer=webgl` & refresh page.");
                }
            }

            #[cfg(target_arch = "wasm32")]
            UICommand::RestartWithWebGpu => {
                if crate::web_tools::set_url_parameter_and_refresh("renderer", "webgpu").is_err() {
                    re_log::error!("Failed to set URL parameter `renderer=webgpu` & refresh page.");
                }
            }

            UICommand::CopyEntityHierarchy => {
                self.copy_entity_hierarchy_to_clipboard(egui_ctx, store_context);
            }

            UICommand::AddRedapServer => {
                self.state.redap_servers.open_add_server_modal();
            }
        }
    }

    pub(super) fn run_pending_recording_commands(
        &mut self,
        egui_ctx: &egui::Context,
        app_blueprint: &AppBlueprint<'_>,
        storage_context: &StorageContext<'_>,
        store_context: Option<&ActiveStoreContext<'_>>,
    ) {
        re_tracing::profile_function!();
        while let Some(cmd) = self.command_receiver.recv_recording() {
            self.run_recording_command(
                egui_ctx,
                app_blueprint,
                storage_context,
                store_context,
                cmd,
            );
        }
    }

    #[allow(clippy::allow_attributes, unused_variables)] // some parameters are only used on some platforms
    fn run_recording_command(
        &mut self,
        egui_ctx: &egui::Context,
        app_blueprint: &AppBlueprint<'_>,
        storage_context: &StorageContext<'_>,
        store_context: Option<&ActiveStoreContext<'_>>,
        cmd: RecordingCommand,
    ) {
        use re_ui::RecordingCommandKind;

        let RecordingCommand { recording_id, kind } = cmd;

        match kind {
            RecordingCommandKind::Save => {
                #[cfg(target_arch = "wasm32")] // Web
                {
                    if let Err(err) = save_active_recording(self, store_context) {
                        re_log::error!("Failed to save recording: {err}");
                    }
                }

                #[cfg(not(target_arch = "wasm32"))] // Native
                {
                    let mut selected_stores = vec![];
                    for item in self.state.selection_state.selected_items().iter_items() {
                        use re_viewer_context::Item;

                        match item {
                            Item::AppId(selected_app_id) => {
                                for recording in storage_context.bundle.recordings() {
                                    if recording.application_id() == selected_app_id {
                                        selected_stores.push(recording.store_id().clone());
                                    }
                                }
                            }
                            Item::StoreId(store_id) => {
                                selected_stores.push(store_id.clone());
                            }
                            _ => {}
                        }
                    }

                    let selected_stores = selected_stores
                        .iter()
                        .filter_map(|store_id| storage_context.bundle.get(store_id))
                        .collect_vec();

                    if selected_stores.is_empty() {
                        if let Err(err) = save_active_recording(self, store_context) {
                            re_log::error!("Failed to save recording: {err}");
                        }
                    } else if selected_stores.len() == 1 {
                        // Common case: saving a single recording.
                        // In this case we want the user to be able to pick a file name (not just a folder):
                        if let Err(err) = save_recording(self, selected_stores[0], None) {
                            re_log::error!("Failed to save recording: {err}");
                        }
                    } else {
                        // Save all selected recordings to a folder:
                        if let Some(folder) = rfd::FileDialog::new()
                            .set_title("Save recordings to folder")
                            .pick_folder()
                        {
                            self.save_many_recordings(&selected_stores, &folder);
                        } else {
                            re_log::info!("No folder selected - recordings not saved.");
                        }
                    }
                }
            }
            RecordingCommandKind::SaveTimeSelection => {
                if let Err(err) = save_active_recording(self, store_context) {
                    re_log::error!("Failed to save recording: {err}");
                }
            }

            RecordingCommandKind::SaveBlueprint => {
                if let Err(err) = save_blueprint(self, store_context) {
                    re_log::error!("Failed to save blueprint: {err}");
                }
            }

            RecordingCommandKind::Close => {
                self.command_sender
                    .send_system(SystemCommand::CloseRecordingOrTable(recording_id.into()));
            }
            RecordingCommandKind::Undo => {
                if let Some(store_context) = store_context {
                    let blueprint_id = store_context.blueprint.store_id().clone();
                    self.command_sender
                        .send_system(SystemCommand::UndoBlueprint { blueprint_id });
                }
            }
            RecordingCommandKind::Redo => {
                if let Some(store_context) = store_context {
                    let blueprint_id = store_context.blueprint.store_id().clone();
                    self.command_sender
                        .send_system(SystemCommand::RedoBlueprint { blueprint_id });
                }
            }

            RecordingCommandKind::AddViewOrContainer => {
                if let Some(ctx) = store_context {
                    let blueprint_query =
                        self.state.blueprint_query_for_viewer(Some(ctx.blueprint));
                    let viewport = re_viewport_blueprint::ViewportBlueprint::from_db(
                        ctx.blueprint,
                        &blueprint_query,
                    );

                    // If a single container is selected, we use it as target.
                    // Otherwise, we target the root container.
                    let target_container_id =
                        if let Some(re_viewer_context::Item::Container(container_id)) =
                            self.state.selection_state.selected_items().single_item()
                        {
                            *container_id
                        } else {
                            viewport.root_container
                        };

                    re_viewport_blueprint::ui::show_add_view_or_container_modal(
                        target_container_id,
                    );
                }
            }
            RecordingCommandKind::ClearActiveBlueprint => {
                self.command_sender
                    .send_system(SystemCommand::ClearActiveBlueprint);
            }
            RecordingCommandKind::ClearActiveBlueprintAndEnableHeuristics => {
                self.command_sender
                    .send_system(SystemCommand::ClearActiveBlueprintAndEnableHeuristics);
            }

            RecordingCommandKind::ToggleTimePanel => {
                app_blueprint.toggle_time_panel(&self.command_sender);
            }

            RecordingCommandKind::ToggleChunkStoreBrowser => {
                match self.state.navigation.current() {
                    Route::ChunkStoreBrowser { return_route, .. } => {
                        self.state.navigation.replace((**return_route).clone());
                    }

                    current => {
                        self.state.navigation.replace(Route::ChunkStoreBrowser {
                            store_id: current.recording_id().cloned(),
                            selected_chunk: None,
                            return_route: Box::new(current.clone()),
                        });
                    }
                }
            }

            #[cfg(debug_assertions)]
            RecordingCommandKind::ToggleBlueprintInspectionPanel => {
                self.app_options_mut().inspect_blueprint_timeline ^= true;
            }

            RecordingCommandKind::PlaybackTogglePlayPause
            | RecordingCommandKind::PlaybackFollow
            | RecordingCommandKind::PlaybackStepBack
            | RecordingCommandKind::PlaybackStepForward
            | RecordingCommandKind::PlaybackBack
            | RecordingCommandKind::PlaybackForward
            | RecordingCommandKind::PlaybackBackFast
            | RecordingCommandKind::PlaybackForwardFast
            | RecordingCommandKind::PlaybackBeginning
            | RecordingCommandKind::PlaybackEnd
            | RecordingCommandKind::PlaybackRestart
            | RecordingCommandKind::PlaybackSpeed(_) => {
                if let Some(time_command) = playback_time_command(kind) {
                    self.command_sender
                        .send_system(SystemCommand::TimeControlCommands {
                            store_id: recording_id,
                            time_commands: vec![time_command],
                        });
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            RecordingCommandKind::PrintChunkStore => {
                if let Some(ctx) = store_context {
                    let text = format!("{}", ctx.recording.storage_engine().store());
                    egui_ctx.copy_text(text.clone());
                    println!("{text}");
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            RecordingCommandKind::PrintBlueprintStore => {
                if let Some(ctx) = store_context {
                    let text = format!("{}", ctx.blueprint.storage_engine().store());
                    egui_ctx.copy_text(text.clone());
                    println!("{text}");
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            RecordingCommandKind::PrintPrimaryCache => {
                if let Some(ctx) = store_context {
                    let text = format!("{:?}", ctx.recording.storage_engine().cache());
                    egui_ctx.copy_text(text.clone());
                    println!("{text}");
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_many_recordings(&mut self, stores: &[&EntityDb], folder: &std::path::Path) {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        use itertools::Itertools as _;
        use re_log::ResultExt as _;
        use re_viewer_context::sanitize_file_name;
        use tap::Pipe as _;

        re_tracing::profile_function!();

        let num_stores = stores.len();
        let any_error = Arc::new(AtomicBool::new(false));
        let num_remaining = Arc::new(AtomicUsize::new(stores.len()));

        re_log::info!("Saving {num_stores} recordings to {}…", folder.display());

        for store in stores {
            let messages = store.to_messages(None).collect_vec();

            let file_name = if let Some(rec_name) = store
                .recording_info_property::<re_sdk_types::components::Name>(
                    re_sdk_types::archetypes::RecordingInfo::descriptor_name().component,
                ) {
                rec_name.to_string()
            } else {
                format!("{}-{}", store.application_id(), store.recording_id())
            }
            .pipe(|name| sanitize_file_name(&name))
            .pipe(|stem| format!("{stem}.rrd"));

            let file_path = folder.join(file_name.clone());
            let any_error = any_error.clone();
            let num_remaining = num_remaining.clone();
            let folder = folder.display().to_string();

            self.background_tasks
                .spawn_threaded_promise(file_name, move || {
                    let res = crate::saving::encode_to_file(
                        re_build_info::CrateVersion::LOCAL,
                        &file_path,
                        messages.into_iter(),
                    );

                    if res.is_err() {
                        any_error.store(true, Ordering::Relaxed);
                    }

                    let num_remaining = num_remaining.fetch_sub(1, Ordering::Relaxed) - 1;

                    if num_remaining == 0 {
                        if any_error.load(Ordering::Relaxed) {
                            re_log::error!("Some recordings failed to save.");
                        } else {
                            re_log::info!("{num_stores} recordings successfully saved to {folder}");
                        }
                    }

                    res
                })
                .ok_or_log_error_once();
        }
    }

    fn run_copy_link_command(&mut self, content_url: &ViewerOpenUrl) {
        let base_url = self.startup_options.web_viewer_base_url();

        match content_url.sharable_url(base_url.as_ref()) {
            Ok(url) => {
                self.copy_text(url);
            }
            Err(err) => {
                re_log::error!("{err}");
            }
        }
    }

    /// Copies text to the clipboard, and gives a notification about it.
    fn copy_text(&mut self, url: String) {
        self.notifications
            .success(format!("Copied {url:?} to clipboard"));
        self.egui_ctx.copy_text(url);
    }

    fn copy_entity_hierarchy_to_clipboard(
        &mut self,
        egui_ctx: &egui::Context,
        store_context: Option<&ActiveStoreContext<'_>>,
    ) {
        let Some(entity_db) = store_context.as_ref().map(|ctx| ctx.recording) else {
            re_log::warn!("Could not copy entity hierarchy: No active recording");
            return;
        };

        use std::fmt::Write as _;

        let mut hierarchy_text = String::new();

        // Add application ID and recording ID header
        write!(
            hierarchy_text,
            "Application ID: {}\nRecording ID: {}\n\n",
            entity_db.application_id(),
            entity_db.recording_id()
        )
        .ok();

        hierarchy_text.push_str(&entity_db.format_with_components());

        if hierarchy_text.is_empty() {
            hierarchy_text = "(no entities)".to_owned();
        }

        egui_ctx.copy_text(hierarchy_text.clone());
        self.notifications
            .success("Copied entity hierarchy with schema to clipboard".to_owned());
    }

    /// Reset the viewer to how it looked the first time you ran it.
    fn reset_viewer(&mut self, store_hub: &mut StoreHub, egui_ctx: &egui::Context) {
        self.state = Default::default();

        store_hub.clear_all_cloned_blueprints();

        // Reset egui:
        egui_ctx.memory_mut(|mem| *mem = Default::default());

        // Restore style:
        re_ui::apply_style_and_install_loaders(egui_ctx);

        if let Err(err) = crate::reset_viewer_persistence() {
            re_log::warn!("Failed to reset viewer: {err}");
        }
    }

    pub(crate) fn toggle_fullscreen(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let fullscreen = self
                .egui_ctx
                .input(|i| i.viewport().fullscreen.unwrap_or(false));
            self.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(options) = &self.startup_options.fullscreen_options {
                // Tell JS to toggle fullscreen.
                if let Err(err) = options.on_toggle.call0() {
                    re_log::error!("{}", crate::web_tools::string_from_js_value(err));
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn is_fullscreen_allowed(&self) -> bool {
        self.startup_options.fullscreen_options.is_some()
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn is_fullscreen_mode(&self) -> bool {
        if let Some(options) = &self.startup_options.fullscreen_options {
            // Ask JS if fullscreen is on or not.
            match options.get_state.call0() {
                Ok(v) => return v.is_truthy(),
                Err(err) => re_log::error_once!("{}", crate::web_tools::string_from_js_value(err)),
            }
        }

        false
    }

    /// Opens `url` through the normal navigation flow.
    fn navigate_to(&self, egui_ctx: &egui::Context, url: &ViewerOpenUrl) {
        url.clone().open(
            egui_ctx,
            &OpenUrlOptions {
                recording_open_behavior: RecordingOpenBehavior::OpenAndSelect,
                show_loader: true,
            },
            &self.command_sender,
        );
    }

    /// Whether navigating back to `url` still leads to an open route.
    ///
    /// `closing` is the recording being closed, if any.
    fn is_back_destination_open(
        &self,
        store_hub: &StoreHub,
        url: &ViewerOpenUrl,
        closing: Option<&StoreId>,
    ) -> bool {
        // A redap origin is reachable as long as its server is still registered.
        let origin_reachable = |origin: &re_uri::Origin| {
            origin == &*re_redap_browser::EXAMPLES_ORIGIN
                || self.state.redap_servers.has_server(origin)
        };

        // Whether some still-open recording, other than the one we're closing, was loaded from
        // `url`.
        let recording_loaded_from = |url: &ViewerOpenUrl| {
            store_hub.store_bundle().recordings().any(|db| {
                closing.is_none_or(|closing| db.store_id() != closing)
                    && store_hub.is_opened(db.store_id())
                    && db.data_source.as_ref().is_some_and(|source| {
                        ViewerOpenUrl::from_data_source(source).is_ok_and(|loaded| &loaded == url)
                    })
            })
        };

        match url {
            // Points into the recording we're closing, so there's nothing left to step back to.
            ViewerOpenUrl::IntraRecordingSelection(_) => false,

            ViewerOpenUrl::WebViewerUrl { url_parameters, .. } => {
                if url_parameters.len() == 1 {
                    self.is_back_destination_open(store_hub, url_parameters.first(), closing)
                } else {
                    false
                }
            }

            // Not a place to land on after closing a recording.
            ViewerOpenUrl::Settings => false,

            // Resolve to a recording loaded from that source: only step back while it's still open.
            ViewerOpenUrl::HttpUrl(_) | ViewerOpenUrl::WebEventListener => {
                recording_loaded_from(url)
            }
            #[cfg(not(target_arch = "wasm32"))]
            ViewerOpenUrl::FilePath(_) => recording_loaded_from(url),

            // Resolves to a recording: it must still be loaded, and not be the one we're closing.
            // We don't re-stream a closed recording, that would land us on a blank, unloaded one.
            ViewerOpenUrl::RedapDatasetSegment(uri) => {
                let store_id = uri.store_id();
                closing.is_none_or(|closing| &store_id != closing) && store_hub.is_opened(&store_id)
            }

            // Redap destinations are reachable while their server is still registered.
            ViewerOpenUrl::RedapProxy(uri) => origin_reachable(&uri.origin),
            ViewerOpenUrl::RedapCatalog(uri) => origin_reachable(&uri.origin),
            ViewerOpenUrl::RedapEntry(uri) => origin_reachable(&uri.origin),
            ViewerOpenUrl::RedapFolder(uri) => origin_reachable(&uri.origin),

            // Tied to a specific store: it must exist, still be opened, and not be the one we're
            // closing. Without an explicit store it falls back to the active recording, which is the
            // one we're closing, so there's nowhere to step back to.
            ViewerOpenUrl::ChunkStoreBrowser { recording_id, .. } => {
                recording_id.as_ref().is_some_and(|id| {
                    closing.is_none_or(|closing| id != closing) && store_hub.is_opened(id)
                })
            }
        }
    }

    /// Where to navigate after closing the active recording `closing`, when there's no usable
    /// history entry to step back to.
    ///
    /// - If viewing a recording in a dataset, go to said dataset.
    /// - Otherwise, if in a redap server, go to said redap server.
    /// - Otherwise go to the start page.
    fn fallback_route_after_close(store_hub: &StoreHub, closing: &StoreId) -> Route {
        let redap_uri = store_hub
            .entity_db(closing)
            .and_then(|db| db.data_source.as_ref())
            .and_then(|source| source.redap_uri());

        match redap_uri {
            Some(re_uri::RedapUri::DatasetData(uri)) => Route::RedapEntry {
                origin: uri.origin,
                kind: re_viewer_context::RedapEntryKind::Entry(uri.dataset_id.into()),
            },

            Some(uri) if !matches!(uri, re_uri::RedapUri::Proxy(_)) => {
                Route::RedapServer(uri.origin().clone())
            }

            _ => Route::welcome_page(),
        }
    }

    fn close_recording(
        &self,
        store_hub: &mut StoreHub,
        entry: &RecordingOrTable,
        mode: CloseRecording,
    ) {
        if let RecordingOrTable::Recording { store_id } = entry {
            store_hub.set_opened(store_id, false);

            // A recording that's still rendered as a preview should stay loaded and streaming, just
            // no longer in the recording list. Removing it would make the preview re-download it.
            // `Force` overrides this and removes it regardless.
            if mode != CloseRecording::Force && store_hub.was_preview(store_id) {
                return;
            }
        }

        let data_source = match entry {
            RecordingOrTable::Recording { store_id } => {
                store_hub.entity_db_entry(store_id).data_source.clone()
            }
            RecordingOrTable::Table { .. } => None,
        };
        if let Some(data_source) = data_source {
            // Only certain sources should be closed.
            #[expect(clippy::match_same_arms)]
            let should_close = match &data_source {
                // Specific files should stop streaming when closing them.
                LogSource::File { .. } => true,

                // Specific HTTP streams should stop streaming when closing them.
                LogSource::HttpStream { .. } => true,

                // Specific GRPC streams should stop streaming when closing them.
                // TODO(#10967): We still stream in some data after that.
                LogSource::RedapGrpcStream { .. } => true,

                // Don't close generic connections (like to an SDK) that may feed in different recordings over time.
                LogSource::RrdWebEvent
                | LogSource::JsChannel { .. }
                | LogSource::Sdk
                | LogSource::Stdin
                | LogSource::MessageProxy(_) => false,
            };

            if should_close {
                self.rx_log.retain(|r| r.source() != &data_source);
            }
        }

        store_hub.remove(entry);
    }
}

/// Propagates [`re_viewer_context::TimeControlResponse`] to [`ViewerEventDispatcher`].
pub(super) fn handle_time_ctrl_event(
    recording: &EntityDb,
    events: Option<&ViewerEventDispatcher>,
    response: &re_viewer_context::TimeControlResponse,
) {
    let Some(events) = events else {
        return;
    };

    if let Some(playing) = response.playing_change {
        events.on_play_state_change(recording, playing);
    }

    if let Some((timeline, time)) = response.timeline_change {
        events.on_timeline_change(recording, timeline, time);
    }

    if let Some(time) = response.time_change {
        events.on_time_update(recording, time);
    }
}

/// [This may only be called on the main thread](https://docs.rs/rfd/latest/rfd/#macos-non-windowed-applications-async-and-threading).
#[cfg(not(target_arch = "wasm32"))]
fn open_file_dialog_native(_: crate::MainThreadToken) -> Vec<std::path::PathBuf> {
    re_tracing::profile_function!();

    let supported: Vec<_> = if re_importer::iter_external_importers().len() == 0 {
        re_importer::supported_extensions().collect()
    } else {
        vec![]
    };

    let mut dialog = rfd::FileDialog::new();

    // If there's at least one external loader registered, then literally anything goes!
    if !supported.is_empty() {
        dialog = dialog.add_filter("Supported files", &supported);
    }

    dialog.pick_files().unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
async fn async_open_rrd_dialog() -> Vec<re_data_source::FileContents> {
    let supported: Vec<_> = re_importer::supported_extensions().collect();

    let files = rfd::AsyncFileDialog::new()
        .add_filter("Supported files", &supported)
        .pick_files()
        .await
        .unwrap_or_default();

    let mut file_contents = Vec::with_capacity(files.len());

    for file in files {
        let file_name = file.file_name();
        re_log::debug!("Reading {file_name}…");
        let bytes = file.read().await;
        re_log::debug!(
            "{file_name} was {}",
            re_format::format_bytes(bytes.len() as _)
        );
        file_contents.push(re_data_source::FileContents {
            name: file_name,
            bytes: bytes.into(),
        });
    }

    file_contents
}

/// The time-control command a playback [`re_ui::RecordingCommandKind`] maps to.
///
/// Returns `None` for non-playback kinds.
fn playback_time_command(kind: re_ui::RecordingCommandKind) -> Option<TimeControlCommand> {
    use re_ui::RecordingCommandKind;
    Some(match kind {
        RecordingCommandKind::PlaybackTogglePlayPause => TimeControlCommand::TogglePlayPause,
        RecordingCommandKind::PlaybackFollow => {
            TimeControlCommand::SetPlayState(PlayState::Following)
        }
        RecordingCommandKind::PlaybackStepBack => TimeControlCommand::StepTimeBack,
        RecordingCommandKind::PlaybackStepForward => TimeControlCommand::StepTimeForward,
        RecordingCommandKind::PlaybackBack => TimeControlCommand::Move {
            direction: MoveDirection::Back,
            speed: MoveSpeed::Normal,
        },
        RecordingCommandKind::PlaybackForward => TimeControlCommand::Move {
            direction: MoveDirection::Forward,
            speed: MoveSpeed::Normal,
        },
        RecordingCommandKind::PlaybackBackFast => TimeControlCommand::Move {
            direction: MoveDirection::Back,
            speed: MoveSpeed::Fast,
        },
        RecordingCommandKind::PlaybackForwardFast => TimeControlCommand::Move {
            direction: MoveDirection::Forward,
            speed: MoveSpeed::Fast,
        },
        RecordingCommandKind::PlaybackBeginning => TimeControlCommand::MoveBeginning,
        RecordingCommandKind::PlaybackEnd => TimeControlCommand::MoveEnd,
        RecordingCommandKind::PlaybackRestart => TimeControlCommand::Restart,
        RecordingCommandKind::PlaybackSpeed(speed) => TimeControlCommand::SetSpeed(speed.0.0),
        _ => return None,
    })
}

fn save_active_recording(
    app: &mut App,
    store_context: Option<&ActiveStoreContext<'_>>,
) -> anyhow::Result<()> {
    let Some(store_context) = store_context else {
        // NOTE: Can only happen if saving through the command palette.
        anyhow::bail!("No recording data to save");
    };

    save_recording(app, store_context.recording, store_context.loop_selection())
}

fn save_recording(
    app: &mut App,
    entity_db: &EntityDb,
    loop_selection: Option<(TimelineName, re_log_types::AbsoluteTimeRangeF)>,
) -> anyhow::Result<()> {
    let rrd_version = entity_db
        .store_info()
        .and_then(|info| info.store_version)
        .unwrap_or(re_build_info::CrateVersion::LOCAL);

    let file_name = if let Some(recording_name) = entity_db
        .recording_info_property::<re_sdk_types::components::Name>(
            re_sdk_types::archetypes::RecordingInfo::descriptor_name().component,
        ) {
        format!("{}.rrd", sanitize_file_name(&recording_name))
    } else {
        "data.rrd".to_owned()
    };

    let title = if loop_selection.is_some() {
        "Save loop selection"
    } else {
        "Save recording"
    };

    save_entity_db(
        app,
        rrd_version,
        file_name,
        title.to_owned(),
        entity_db.to_messages(loop_selection),
    )
}

fn save_blueprint(
    app: &mut App,
    store_context: Option<&ActiveStoreContext<'_>>,
) -> anyhow::Result<()> {
    let Some(store_context) = store_context else {
        anyhow::bail!("No blueprint to save");
    };

    re_tracing::profile_function!();

    let rrd_version = store_context
        .blueprint
        .store_info()
        .and_then(|info| info.store_version)
        .unwrap_or(re_build_info::CrateVersion::LOCAL);

    // We change the recording id to a new random one,
    // otherwise when saving and loading a blueprint file, we can end up
    // in a situation where the store_id we're loading is the same as the currently active one,
    // which mean they will merge in a strange way.
    // This is also related to https://github.com/rerun-io/rerun/issues/5295
    let new_store_id = store_context
        .blueprint
        .store_id()
        .clone()
        .with_recording_id(RecordingId::random());

    let mut saved_blueprint = store_context
        .blueprint
        .clone_with_new_id(new_store_id)
        .context("Cloning current blueprint")?;

    if let Some(undo_state) = app
        .state
        .blueprint_undo_state
        .get(store_context.blueprint.store_id())
    {
        // We don't actually want to edit the undo state when saving,
        // just clear the redo-buffer section of what we save.
        undo_state.clone().clear_redo_buffer(&mut saved_blueprint);
    }

    let messages = saved_blueprint.to_messages(None);

    let file_name = format!(
        "{}.rbl",
        crate::saving::sanitize_app_id(store_context.application_id())
    );
    let title = "Save blueprint";

    save_entity_db(app, rrd_version, file_name, title.to_owned(), messages)
}

// TODO(emilk): unify this with `ViewerContext::save_file_dialog`
#[allow(clippy::allow_attributes, clippy::needless_pass_by_ref_mut)] // `app` is only used on native
#[allow(clippy::unnecessary_wraps)] // cannot return error on web
fn save_entity_db(
    #[allow(clippy::allow_attributes, unused_variables)] app: &mut App, // only used on native
    rrd_version: CrateVersion,
    file_name: String,
    title: String,
    messages: impl Iterator<Item = re_chunk::ChunkResult<LogMsg>>,
) -> anyhow::Result<()> {
    re_tracing::profile_function!();

    // TODO(#6984): Ideally we wouldn't collect at all and just stream straight to the
    // encoder from the store.
    //
    // From a memory usage perspective this isn't too bad though: the data within is still
    // refcounted straight from the store in any case.
    //
    // It just sucks latency-wise.
    let messages = messages.collect::<Vec<_>>();

    // Web
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(err) =
                async_save_dialog(rrd_version, &file_name, &title, messages.into_iter()).await
            {
                re_log::error!("File saving failed: {err}");
            }
        });
    }

    // Native
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = {
            re_tracing::profile_scope!("file_dialog");
            rfd::FileDialog::new()
                .set_file_name(file_name)
                .set_title(title)
                .save_file()
        };
        if let Some(path) = path {
            app.background_tasks.spawn_file_saver(move || {
                crate::saving::encode_to_file(rrd_version, &path, messages.into_iter())?;
                Ok(path)
            })?;
        }
    }

    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn async_save_dialog(
    rrd_version: CrateVersion,
    file_name: &str,
    title: &str,
    messages: impl Iterator<Item = re_chunk::ChunkResult<LogMsg>>,
) -> anyhow::Result<()> {
    use anyhow::Context as _;

    let file_handle = rfd::AsyncFileDialog::new()
        .set_file_name(file_name)
        .set_title(title)
        .save_file()
        .await;

    let Some(file_handle) = file_handle else {
        return Ok(()); // aborted
    };

    let options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
    let mut bytes = Vec::new();
    re_log_encoding::Encoder::encode_into(rrd_version, options, messages, &mut bytes)?;
    file_handle.write(&bytes).await.context("Failed to save")
}
