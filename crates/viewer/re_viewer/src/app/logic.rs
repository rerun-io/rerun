use std::str::FromStr as _;

use ahash::HashMap;
use re_entity_db::LogSource;
use re_log_channel::{
    DataSourceMessage, DataSourceUiCommand, RecordingOpenBehavior, SaveScreenshotError,
};
use re_log_types::{LogMsg, StoreId, StoreKind, TableMsg};
use re_sdk_types::external::uuid;
use re_viewer_context::{
    Item, Route, StoreHub, SystemCommand, SystemCommandSender as _, TableStore,
};

use crate::app_blueprint::AppBlueprint;

use super::App;

impl App {
    /// Called before each call to `ui`, but ALSO when the app is
    /// hidden (occluded, minimized, …) if something has called `request_repaint`.
    ///
    /// We put things here that are unrelated to the UI,
    /// and that we still want to happen if the application is hidden.
    pub(super) fn logic_impl(&mut self, egui_ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Temporarily take the `StoreHub` out of the Viewer so it doesn't interfere with mutability
        let mut store_hub = self
            .store_hub
            .take()
            .expect("Failed to take store hub from the Viewer");

        {
            // Respect memory budget:
            self.purge_memory_if_needed(&mut store_hub); // Call BEFORE `begin_frame_caches`

            if self.app_options().blueprint_gc {
                store_hub.gc_blueprints(&self.state.blueprint_undo_state);
            }
        }

        {
            // Download/ingest data:
            self.receive_messages(&mut store_hub, egui_ctx);
            self.receive_fetched_chunks(&mut store_hub);
            self.prefetch_chunks(&mut store_hub);
        }

        self.run_pending_system_commands(&mut store_hub, egui_ctx);

        {
            // We also need to check for Ui commands, especially `UiCommand::Quit`.

            let route = self.state.navigation.current().clone();
            let (storage_context, store_context) = store_hub.read_context(&route);

            let blueprint = store_context.as_ref().map(|ctx| ctx.blueprint);
            let blueprint_query = self.state.blueprint_query_for_viewer(blueprint);

            let app_blueprint = AppBlueprint::new(
                blueprint,
                &blueprint_query,
                egui_ctx,
                self.panel_state_overrides_active
                    .then_some(self.panel_state_overrides),
            );

            self.run_pending_ui_commands(
                egui_ctx,
                &app_blueprint,
                &storage_context,
                store_context.as_ref(),
                &route,
            );
        }

        self.state.cleanup(&store_hub);

        // Return the `StoreHub` to the Viewer so we have it on the next frame
        self.store_hub = Some(store_hub);
    }

    fn receive_messages(&mut self, store_hub: &mut StoreHub, egui_ctx: &egui::Context) {
        re_tracing::profile_function!();

        let start = web_time::Instant::now();

        while let Some((channel_source, msg)) = self.rx_log.try_recv() {
            re_log::trace!("Received a message from {channel_source:?}"); // Used by `test_ui_wakeup` test app!

            if let Some(re_uri::RedapUri::DatasetData(uri)) = channel_source.redap_uri() {
                self.connection_registry.clear_uri_error(uri);
            }

            let msg = match msg.payload {
                re_log_channel::SmartMessagePayload::Msg(msg) => msg,

                re_log_channel::SmartMessagePayload::Flush { on_flush_done } => {
                    re_tracing::profile_scope!("on_flush_done");
                    on_flush_done();
                    continue;
                }

                re_log_channel::SmartMessagePayload::Quit(err) => {
                    if let Some(err) = err {
                        re_log::warn!(
                            "Data source has left unexpectedly: {err}, source: {}",
                            msg.source
                        );
                        if let Some(re_uri::RedapUri::DatasetData(uri)) = channel_source.redap_uri()
                        {
                            self.connection_registry.set_uri_error(uri, err.to_string());
                        }
                    } else {
                        re_log::debug!("Data source {} has finished", msg.source);
                    }
                    continue;
                }
            };

            // We centralize "new store" detection and `data_source` attachment here, so that the `on_new_store`
            // side effects (like `set_opened(true)` for `OpenAndSelect`) fire regardless of which message type
            // happens to come first.
            let msg_store_id = match &msg {
                DataSourceMessage::RrdManifest(store_id, _)
                | DataSourceMessage::RrdManifestComplete(store_id) => Some(store_id.clone()),
                DataSourceMessage::LogMsg(log_msg) => Some(log_msg.store_id().clone()),
                DataSourceMessage::TableMsg(_) | DataSourceMessage::UiCommand(_) => None,
            };

            let maybe_new_store = msg_store_id
                .as_ref()
                .filter(|sid| !store_hub.store_bundle().contains(sid));

            if let Some(sid) = &msg_store_id {
                let entity_db = store_hub.entity_db_entry(sid);
                if entity_db.data_source.is_none() {
                    entity_db.data_source = Some((*channel_source).clone());
                }
            }

            match msg {
                DataSourceMessage::RrdManifest(store_id, rrd_manifest) => {
                    let entity_db = store_hub.entity_db_entry(&store_id);
                    let store_events = entity_db.add_rrd_manifest_message(rrd_manifest);

                    if let Some((entity_db, cache)) =
                        store_hub.entity_db_and_cache(&store_id, &self.view_class_registry)
                    {
                        cache.on_store_events(&store_events, entity_db);
                    }
                }

                DataSourceMessage::RrdManifestComplete(store_id) => {
                    let entity_db = store_hub.entity_db_entry(&store_id);
                    entity_db.mark_rrd_manifest_complete();
                }

                DataSourceMessage::LogMsg(msg) => {
                    self.receive_log_msg(&msg, store_hub, egui_ctx, &channel_source);
                }

                DataSourceMessage::TableMsg(table) => {
                    self.receive_table_msg(store_hub, egui_ctx, table);
                }

                DataSourceMessage::UiCommand(ui_command) => {
                    self.receive_data_source_ui_command(ui_command, &channel_source);
                }
            }

            // Handle any action that is triggered by a new store _after_ processing the message
            // that caused it.
            if let Some(sid) = &maybe_new_store {
                self.on_new_store(egui_ctx, sid, &channel_source, store_hub);
            }

            if start.elapsed() > web_time::Duration::from_millis(10) {
                egui_ctx.request_repaint(); // make sure we keep receiving messages asap
                break; // don't block the main thread for too long
            }
        }

        // Run pending system commands in case any of the messages resulted in additional commands.
        // This avoid further frame delays on these commands.
        self.run_pending_system_commands(store_hub, egui_ctx);
    }

    /// There is logic duplicated between this and [`Self::prefetch_chunks`].
    /// Make sure they are kept in sync!
    fn receive_log_msg(
        &mut self,
        msg: &LogMsg,
        store_hub: &mut StoreHub,
        egui_ctx: &egui::Context,
        channel_source: &LogSource,
    ) {
        re_tracing::profile_function!();

        let store_id = msg.store_id();

        if store_hub.is_active_blueprint(store_id) {
            // TODO(#5514): handle loading of active blueprints.
            re_log::warn_once!(
                "Loading a blueprint {store_id:?} that is active. See https://github.com/rerun-io/rerun/issues/5514 for details."
            );
        }

        // NOTE: store materialization, `data_source` attachment, and the `on_new_store`
        // dispatch are handled in `receive_messages` so that they also fire for stores first
        // introduced by `RrdManifest` / `RrdManifestComplete` messages.
        let entity_db = store_hub.entity_db_entry(store_id);
        let was_empty = entity_db.num_physical_chunks() == 0;
        let entity_db_add_result = entity_db.add_log_msg(msg);

        match entity_db_add_result {
            Ok(store_events) => {
                self.process_store_events_for_db(store_hub, store_id, &store_events);
            }

            Err(err) => {
                re_log::error_once!("Failed to add incoming msg: {err}");
            }
        }

        // Need to reborrow as read-only since we passed store_hub as mutable earlier.
        let entity_db = store_hub
            .entity_db(store_id)
            .expect("Just queried it mutable and that was fine.");

        // Note: some of the logic above is duplicated in `fn prefetch_chunks`.
        // Make sure they are kept in sync!

        let is_empty = entity_db.num_physical_chunks() == 0;
        if was_empty && !is_empty {
            // Hack: we cannot go to a specific timeline or entity until we know about it.
            // Now we _hopefully_ do. The `LogMsg` could also belong to the blueprint, so
            // we need to check for that as well.
            if let LogSource::RedapGrpcStream { uri, .. } = channel_source
                && &uri.store_id() == store_id
            {
                self.go_to_dataset_data(uri.store_id(), uri.fragment.clone());
            }
        }

        #[expect(clippy::match_same_arms)]
        match &msg {
            LogMsg::SetStoreInfo(_) => {
                // Causes a new store typically. But that's handled below via `on_new_store`.
            }

            LogMsg::ArrowMsg(_, _) => {
                // Handled by `EntityDb::add`.
            }

            LogMsg::BlueprintActivationCommand(cmd) => match store_id.kind() {
                StoreKind::Recording => {
                    re_log::debug!(
                        "Unexpected `BlueprintActivationCommand` message for {store_id:?}"
                    );
                }
                StoreKind::Blueprint => {
                    if let Some(info) = entity_db.store_info() {
                        re_log::trace!(
                            "Activating blueprint that was loaded from {channel_source}"
                        );
                        let app_id = info.application_id().clone();
                        if cmd.make_default {
                            store_hub
                                .set_default_blueprint_for_app(store_id)
                                .unwrap_or_else(|err| {
                                    re_log::warn!("Failed to make blueprint default: {err}");
                                });
                        }
                        if cmd.make_active {
                            store_hub
                                .set_cloned_blueprint_active_for_app(store_id)
                                .unwrap_or_else(|err| {
                                    re_log::warn!("Failed to make blueprint active: {err}");
                                });

                            // Switch to this app, e.g. on drag-and-drop of a blueprint file

                            if self.state.navigation.current().app_id() != Some(&app_id) {
                                // Switch to this app:

                                store_hub.load_persisted_blueprints_for_app(&app_id);
                                if let Some(recording_id) =
                                    store_hub.earliest_recording_for_app(&app_id)
                                {
                                    store_hub.load_blueprint_and_caches(
                                        &recording_id,
                                        &self.view_class_registry,
                                    );
                                    self.state
                                        .selection_state
                                        .set_selection(Item::StoreId(recording_id.clone()));
                                    self.state
                                        .navigation
                                        .replace(Route::LocalRecording { recording_id });
                                } else {
                                    // TODO(RR-3713): show a blueprint for it anyway
                                    re_log::debug_once!(
                                        "Received BlueprintActivationCommand for app '{app_id}', but we have no recording for it"
                                    );
                                }
                            }

                            // If the viewer is in the background, tell the user that it has received something new.
                            egui_ctx.send_viewport_cmd(
                                egui::ViewportCommand::RequestUserAttention(
                                    egui::UserAttentionType::Informational,
                                ),
                            );
                        }
                    } else {
                        re_log::warn!(
                            "Got ActivateStore message without first receiving a SetStoreInfo"
                        );
                    }
                }
            },
        }
    }

    fn process_store_events_for_db(
        &self,
        store_hub: &mut StoreHub,
        store_id: &StoreId,
        store_events: &[re_chunk_store::ChunkStoreEvent],
    ) {
        re_tracing::profile_function!();

        // Keep all caches up to date, even if they're in the background.
        // This ensures that when we switch to a different recording, the caches are already valid.
        if let Some((entity_db, cache)) =
            store_hub.entity_db_and_cache(store_id, &self.view_class_registry)
        {
            cache.on_store_events(store_events, entity_db);
        }

        self.validate_loaded_events(store_events);
    }

    fn receive_table_msg(
        &self,
        store_hub: &mut StoreHub,
        egui_ctx: &egui::Context,
        table: TableMsg,
    ) {
        re_tracing::profile_function!();

        let TableMsg { id, data } = table;

        // TODO(grtlr): For now we don't append anything to existing stores and always replace.
        // TODO(ab): When we actually append to existing table, we will have to clear the UI
        // cache by calling `DataFusionTableWidget::clear_state`.
        let store = TableStore::default();
        if let Err(err) = store.add_record_batch(data) {
            re_log::error!("Failed to load table {id}: {err}");
        } else {
            if store_hub.insert_table_store(id.clone(), store).is_some() {
                re_log::debug!("Overwritten table store with id: `{id}`");
            } else {
                re_log::debug!("Inserted table store with id: `{id}`");
            }
            self.command_sender
                .send_system(SystemCommand::set_selection(
                    re_viewer_context::Item::TableId(id),
                ));

            // If the viewer is in the background, tell the user that it has received something new.
            egui_ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Informational,
            ));
        }
    }

    fn on_new_store(
        &mut self,
        egui_ctx: &egui::Context,
        store_id: &StoreId,
        channel_source: &LogSource,
        store_hub: &mut StoreHub,
    ) {
        match channel_source.open_behavior() {
            RecordingOpenBehavior::Background => {}

            RecordingOpenBehavior::Open => {
                if store_id.kind() == StoreKind::Recording {
                    store_hub.set_opened(store_id, true);
                }
            }

            RecordingOpenBehavior::OpenAndSelect => {
                // Set the recording-id after potentially creating the store in the hub.
                // This ordering is important because the `StoreHub` internally
                // updates the app-id when changing the recording.
                match store_id.kind() {
                    StoreKind::Recording => {
                        re_log::trace!("Opening a new recording: '{store_id:?}'");
                        self.make_store_active_and_highlight(store_hub, egui_ctx, store_id);
                    }
                    StoreKind::Blueprint => {
                        // We wait with activating blueprints until they are fully loaded,
                        // so that we don't run heuristics on half-loaded blueprints.
                        // Otherwise on a mixed connection (SDK sending both blueprint and recording)
                        // the blueprint won't be activated until the whole _recording_ has finished loading.
                    }
                }
            }
        }

        let entity_db = store_hub.entity_db_entry(store_id);
        let is_example = entity_db.store_class().is_example();

        if cfg!(target_arch = "wasm32") && !self.startup_options.is_in_notebook && !is_example {
            use std::sync::Once;
            static ONCE: Once = Once::new();
            ONCE.call_once(|| {
                // Tell the user there is a faster native viewer they can use instead of the web viewer:
                let notification = re_ui::notifications::Notification::new(
                    re_ui::notifications::NotificationLevel::Tip, "For better performance, try the native Rerun Viewer!").with_link(
                    re_ui::Link {
                        text: "Install…".into(),
                        url: "https://rerun.io/docs/overview/installing-rerun/viewer#installing-the-viewer".into(),
                    }
                )
                    .no_toast()
                    .permanent_dismiss_id(egui::Id::new("install_native_viewer_prompt"));
                self.command_sender
                    .send_system(SystemCommand::ShowNotification(notification));
            });
        }

        if entity_db.store_kind() == StoreKind::Recording {
            #[cfg(feature = "analytics")]
            if let Some(analytics) = re_analytics::Analytics::global_or_init()
                && let Some(event) =
                    crate::viewer_analytics::event::open_recording(&self.app_env, entity_db)
            {
                analytics.record(event);
            }

            if let Some(event_dispatcher) = self.event_dispatcher.as_ref() {
                event_dispatcher.on_recording_open(entity_db);
            }
        }
    }

    fn receive_data_source_ui_command(
        &mut self,
        ui_command: DataSourceUiCommand,
        channel_source: &LogSource,
    ) {
        re_tracing::profile_function!();
        match ui_command {
            DataSourceUiCommand::SetUrlFragment { store_id, fragment } => {
                match re_uri::Fragment::from_str(&fragment) {
                    Ok(fragment) => {
                        self.command_sender
                            .send_system(SystemCommand::SetUrlFragment { store_id, fragment });
                    }

                    Err(err) => {
                        re_log::warn!(
                            "Failed to parse fragment received from {channel_source:?}: {err}"
                        );
                    }
                }
            }

            DataSourceUiCommand::SaveScreenshot {
                file_path,
                view_id,
                on_done,
            } => {
                let view_id = if let Some(view_id) = view_id {
                    if let Ok(view_id) = uuid::Uuid::parse_str(&view_id) {
                        Some(view_id.into())
                    } else {
                        re_log::error!(
                            "Failed to parse view id from {view_id:?}. Expected a UUID."
                        );
                        if let Some(on_done) = on_done {
                            on_done
                                .unbounded_send(Err(SaveScreenshotError::InvalidViewId { view_id }))
                                .ok();
                        }
                        return;
                    }
                } else {
                    None
                };

                if let Some(on_done) = on_done {
                    self.pending_screenshot_notifiers
                        .insert(file_path.clone(), on_done);
                }

                self.command_sender
                    .send_system(SystemCommand::SaveScreenshot {
                        target: re_viewer_context::ScreenshotTarget::SaveToPath(file_path),
                        view_id,
                    });
            }
        }
    }

    /// Receive in-transit chunks (previously prefetched):
    fn receive_fetched_chunks(&self, store_hub: &mut StoreHub) {
        re_tracing::profile_function!();

        let store_ids: Vec<_> = store_hub
            .store_bundle()
            .recordings()
            .map(|db| db.store_id().clone())
            .collect();

        for store_id in store_ids {
            let db = store_hub.entity_db_entry(&store_id);

            if cfg!(debug_assertions) && db.can_fetch_chunks_from_redap() {
                re_tracing::profile_scope!("debug-sanity-check");
                let storage_engine = db.storage_engine();
                let store = storage_engine.store();

                #[expect(clippy::iter_over_hash_type)] // sanity checks don't care about order
                for missing_chunk_id in store.tracked_chunk_ids().missing_virtual {
                    let roots = store.find_root_chunks(&missing_chunk_id);
                    re_log::debug_assert!(!roots.is_empty(), "Missing chunk has no roots");

                    let all_roots_are_fully_loaded = roots.iter().all(|root_id| {
                        let root_info = db.rrd_manifest_index().root_chunk_info(root_id);
                        if let Some(root_info) = root_info {
                            root_info.is_fully_loaded()
                        } else {
                            re_log::debug_warn_once!("Failed to find root chunk");
                            false
                        }
                    });

                    if all_roots_are_fully_loaded {
                        re_log::warn_once!(
                            "A chunk was reported missing, but all its roots are marked as fully loaded."
                        );
                        re_log::debug_once!(
                            "Missing: {missing_chunk_id}, roots: {roots:?}, Chunk lineage: {}",
                            store.format_lineage(&missing_chunk_id)
                        );
                    }
                }
            }

            if db.can_fetch_chunks_from_redap() {
                re_tracing::profile_scope!("recording");

                let mut store_events = Vec::new();
                for chunk in db
                    .rrd_manifest_index_mut()
                    .chunk_requests_mut()
                    .receive_finished(self.egui_ctx.time())
                {
                    match db.add_chunk(&std::sync::Arc::new(chunk)) {
                        Ok(events) => {
                            store_events.extend(events);
                        }
                        Err(err) => {
                            re_log::warn_once!("add_chunk failed: {err}");
                        }
                    }
                }

                self.process_store_events_for_db(store_hub, &store_id, &store_events);

                // Need to reborrow since we pass `&mut store_hub` above.
                let db = store_hub.entity_db_entry(&store_id);

                // Note: some of the logic above is duplicated in `fn receive_log_msg`.
                // Make sure they are kept in sync!

                // We cancel right after resoliving (above), so that
                // we give each fetch as much time as possible to finish.
                db.rrd_manifest_index_mut()
                    .cancel_outdated_requests(self.egui_ctx.time());

                if db.rrd_manifest_index_mut().chunk_requests().has_pending() {
                    self.egui_ctx.request_repaint(); // check back for more
                }
            }
        }
    }

    /// Makes the given store active and request user attention if Rerun in the background.
    pub(super) fn make_store_active_and_highlight(
        &mut self,
        store_hub: &mut StoreHub,
        egui_ctx: &egui::Context,
        store_id: &StoreId,
    ) {
        if store_id.is_blueprint() {
            re_log::warn!(
                "Can't make a blueprint active: {store_id:?}. This is likely a bug in Rerun."
            );
            return;
        }

        store_hub.set_opened(store_id, true);
        store_hub.load_blueprint_and_caches(store_id, &self.view_class_registry);
        self.state.navigation.replace(Route::LocalRecording {
            recording_id: store_id.clone(),
        });

        // Also select the new recording:
        self.command_sender
            .send_system(SystemCommand::set_selection(
                re_viewer_context::Item::StoreId(store_id.clone()),
            ));

        // If the viewer is in the background, tell the user that it has received something new.
        egui_ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
            egui::UserAttentionType::Informational,
        ));
    }

    /// After loading some data; check if the loaded data makes sense.
    fn validate_loaded_events(&self, store_events: &[re_chunk_store::ChunkStoreEvent]) {
        re_tracing::profile_function!();

        for event in store_events {
            let Some(chunk) = event.delta_chunk() else {
                continue;
            };

            // For speed, we don't care about the order of the following log statements, so we silence this warning
            for component_descr in chunk.components().component_descriptors() {
                if let Some(archetype_name) = component_descr.archetype {
                    if let Some(archetype) = self.reflection.archetypes.get(&archetype_name) {
                        for &view_type in archetype.view_types {
                            if !cfg!(feature = "map_view") && view_type == "MapView" {
                                re_log::warn_once!(
                                    "Found map-related archetype, but viewer was not compiled with the `map_view` feature."
                                );
                            }
                        }
                    } else {
                        re_log::trace_once!("Unknown archetype: {archetype_name}");
                    }
                }
            }
        }
    }

    pub(super) fn purge_memory_if_needed(&mut self, store_hub: &mut StoreHub) {
        re_tracing::profile_function!();

        use re_format::format_bytes;
        use re_memory::MemoryUse;

        let limit = self.app_options().memory_limit;
        let mut mem_use_before = MemoryUse::capture();

        let default_limit = re_memory::MemoryLimit::default_for_current_platform();

        // If we are at the default limit, which is derived from system memory,
        // we actually do want to count external to OOM.
        let external_mem = if limit.as_bytes() >= default_limit.as_bytes()
            || default_limit.is_exceeded_by(&mem_use_before).is_some()
        {
            0
        } else {
            let external_mem = self.external_memory_users.total_external_memory();

            if let Some(counted) = &mut mem_use_before.counted {
                *counted -= external_mem;
            }

            if let Some(resident) = &mut mem_use_before.resident {
                *resident -= external_mem;
            }

            external_mem
        };

        if let Some(minimum_fraction_to_purge) = limit.is_exceeded_by(&mem_use_before) {
            re_log::info_once!("Reached memory limit of {limit}. Freeing up data…");

            let fraction_to_purge = (minimum_fraction_to_purge + 0.2).clamp(0.25, 1.0);

            re_log::trace!("RAM limit: {limit}");
            if let Some(resident) = mem_use_before.resident {
                re_log::trace!("Resident: {}", format_bytes(resident as _),);
            }
            if let Some(counted) = mem_use_before.counted {
                re_log::trace!("Counted: {}", format_bytes(counted as _));
            }
            if external_mem > 0 {
                re_log::trace!("External: {}", format_bytes(external_mem as _));
            }

            re_tracing::profile_scope!("pruning");
            if let Some(counted) = mem_use_before.counted {
                re_log::trace!(
                    "Attempting to purge {:.1}% of used RAM ({})…",
                    100.0 * fraction_to_purge,
                    format_bytes(counted as f64 * fraction_to_purge as f64)
                );
            }

            store_hub.purge_fraction_of_ram(
                fraction_to_purge,
                self.active_recording_id(),
                &|store_id| self.state.time_cursor_for(store_id).map(|t| t.time_cursor),
            );

            let mem_use_after = MemoryUse::capture();

            let freed_memory = mem_use_before - mem_use_after;

            if let (Some(counted_before), Some(counted_diff)) =
                (mem_use_before.counted, freed_memory.counted)
                && 0 < counted_diff
            {
                re_log::debug!(
                    "GC result: -{} (-{:.1}%).",
                    format_bytes(counted_diff as _),
                    100.0 * counted_diff as f32 / counted_before as f32
                );
            }

            // Cache app overhead = total memory use minus all recording chunk data.
            // This captures fonts, UI state, indices, and other unevictable memory.
            if let Some(current_mem_use) = mem_use_after.counted.or(mem_use_after.resident) {
                let total_chunk_bytes: u64 = store_hub
                    .store_bundle()
                    .recordings()
                    .map(|r| r.byte_size_of_physical_chunks())
                    .sum();
                self.cached_app_overhead_bytes =
                    Some(current_mem_use.saturating_sub(total_chunk_bytes));
            }

            self.memory_panel.note_memory_purge();
        }
    }

    /// Prefetch chunks for the open recording (stream from server)
    ///
    /// There is logic duplicated between this and [`Self::receive_log_msg`].
    /// Make sure they are kept in sync!
    fn prefetch_chunks(&self, store_hub: &mut StoreHub) {
        re_tracing::profile_function!();

        use crate::prefetch_chunks::{RecordingOpenKind, RecordingPrefetchInfo};
        use re_entity_db::ChunkPrefetchOptions;

        let active_recording_id = self.active_recording_id();

        // Fixed overhead for the app (fonts, icons, caches, etc.) that we cannot purge.
        // We also want some headroom for spikes.
        const APP_OVERHEAD_BYTES: u64 = 300_000_000;

        // When we have a measured overhead we need less extra headroom.
        // When we don't, use a larger fraction to be safe.
        const FIXED_FRACTION_OVERHEAD: f32 = 0.10;
        const FALLBACK_FIXED_FRACTION_OVERHEAD: f32 = 0.20;

        let overhead = self.cached_app_overhead_bytes.unwrap_or(APP_OVERHEAD_BYTES);
        let fixed_fraction_overhead = if self.cached_app_overhead_bytes.is_some() {
            FIXED_FRACTION_OVERHEAD
        } else {
            FALLBACK_FIXED_FRACTION_OVERHEAD
        };

        let memory_limit = self
            .app_options()
            .memory_limit
            .saturating_sub(overhead)
            .split(fixed_fraction_overhead)
            .1;

        if memory_limit == re_memory::MemoryLimit::ZERO {
            re_log::warn_once!("Very little memory budget left for prefetching recordings.");
        }

        let mut recordings_info: HashMap<StoreId, RecordingPrefetchInfo> = HashMap::default();

        for recording in store_hub.store_bundle().recordings() {
            if recording.is_downloading_first_part_of_manifest() {
                // We need at least ONE part of the manifest before prefetching chunks.
                continue;
            }

            let is_active = Some(recording.store_id()) == active_recording_id;
            let usage = store_hub.usage(recording.store_id());

            let open_kind = if is_active {
                RecordingOpenKind::Active
            } else if usage.was_preview() {
                RecordingOpenKind::Preview
            } else if usage.opened {
                RecordingOpenKind::Inactive
            } else {
                continue;
            };

            let time_cursor = match open_kind {
                RecordingOpenKind::Active => self.state.time_cursor_for(recording.store_id()),
                RecordingOpenKind::Preview => {
                    let timelines = recording.timelines();
                    let timeline =
                        re_chunk::Timeline::pick_best_timeline(timelines.values(), |t| {
                            recording.num_temporal_rows_on_timeline(t.name())
                        });

                    Some(re_entity_db::PrefetchTimeCursor {
                        time_cursor: re_log_types::TimelinePoint {
                            name: *timeline.name(),
                            typ: timeline.typ(),
                            // TODO(RR-4257): Don't hack mid-point time
                            time: recording
                                .rrd_manifest_index()
                                .timeline_range(timeline.name())
                                .map(|r| r.center())
                                .unwrap_or(re_chunk::TimeInt::ZERO),
                        },
                        speed_if_unpaused: 1.0,
                        loop_range: None,
                    })
                }
                RecordingOpenKind::Inactive => None,
            };
            if let Some(redap_uri) = recording.redap_uri() {
                let store_id = recording.store_id().clone();
                recordings_info.insert(
                    store_id.clone(),
                    RecordingPrefetchInfo {
                        store_id,
                        open_kind,
                        time_cursor,
                        origin: redap_uri.origin.clone(),
                    },
                );
            }
        }

        let total_bytes_in_memory = memory_limit.at_least(100_000_000).as_bytes();

        crate::prefetch_chunks::prefetch_chunks_for_recordings(
            &self.egui_ctx,
            store_hub.store_bundle_mut(),
            &recordings_info,
            total_bytes_in_memory,
            self.connection_registry(),
            &ChunkPrefetchOptions {
                max_fetch_stage: self.app_options().max_fetch_stage,
                ..ChunkPrefetchOptions::default()
            },
        );
    }
}
