use re_data_source::LogDataSource;
use re_entity_db::LogSource;
use re_log_channel::{LogReceiver, RecordingOpenBehavior};
use re_sdk_types::blueprint::components::PlayState;
use re_viewer_context::{StoreHub, SystemCommand, SystemCommandSender as _, TimeControlCommand};

use super::App;

impl App {
    #[expect(clippy::needless_pass_by_ref_mut)]
    pub fn add_log_receiver(&mut self, rx: LogReceiver) {
        re_log::debug!("Adding new log receiver: {}", rx.source());

        // Make sure we wake up when a new message is available:
        rx.set_waker({
            let egui_ctx = self.egui_ctx.clone();
            move || {
                // Spend a few more milliseconds decoding incoming messages,
                // then trigger a repaint (https://github.com/rerun-io/rerun/issues/963):
                egui_ctx.request_repaint_after(std::time::Duration::from_millis(10));
            }
        });

        // Add unknown redap servers.
        //
        // Otherwise we end up in a situation where we have a data from an unknown server,
        // which is unnecessary and can get us into a strange ui state.
        if let LogSource::RedapGrpcStream { uri, .. } = rx.source() {
            self.command_sender
                .send_system(SystemCommand::AddRedapServer(uri.origin.clone()));
        }

        self.rx_log.add(rx);
    }

    /// Add a tracker for memory external to the viewer but in the same process.
    pub fn add_external_memory_user(&mut self, user: Box<dyn crate::ExternalMemoryUser>) {
        self.external_memory_users.add(user);
    }

    /// Loads a data source into the viewer.
    ///
    /// Tries to detect whether the datasource is already present (either still streaming in or already loaded),
    /// and if so, will not load the data again.
    /// Instead, it will only perform any kind of selection/mode-switching operations associated with loading the given data source.
    ///
    /// Note that we *do not* change the route here _unconditionally_.
    /// For instance if the datasource is a blueprint for a dataset that may be loaded later,
    /// we don't want to switch out to it while the user browses a server.
    pub(super) fn load_data_source(
        &mut self,
        store_hub: &mut StoreHub,
        egui_ctx: &egui::Context,
        data_source: &LogDataSource,
    ) {
        re_tracing::profile_function!();

        // Check if we've already loaded this data source and should just switch to it.
        //
        // Go through all sources that are still loading and those that are already in the store_hub.
        // (if we look only at the one from the store_hub, we might miss those that haven't hit it yet)
        let active_sources = self.rx_log.sources();
        // Only consider recordings for dedup, not blueprints.
        // Blueprints loaded alongside a recording share the same `data_source`,
        // but they should not prevent re-opening a closed recording.
        let store_sources = store_hub
            .store_bundle()
            .recordings()
            .filter_map(|db| db.data_source.as_ref());
        let mut all_sources =
            std::iter::chain(store_sources, active_sources.iter().map(|s| s.as_ref()));

        match data_source {
            LogDataSource::HttpUrl { url, follow } => {
                let new_source = LogSource::HttpStream {
                    url: url.to_string(),
                    follow: *follow,
                };

                if all_sources.any(|source| source.is_same_ignoring_uri_fragments(&new_source)) {
                    if let Some(entity_db) = store_hub.find_recording_store_by_source(&new_source) {
                        if *follow {
                            self.command_sender
                                .send_system(SystemCommand::TimeControlCommands {
                                    store_id: entity_db.store_id().clone(),
                                    time_commands: vec![TimeControlCommand::SetPlayState(
                                        PlayState::Following,
                                    )],
                                });
                        }

                        let store_id = entity_db.store_id().clone();
                        re_log::debug_assert!(store_id.is_recording()); // `find_recording_store_by_source` should have filtered for recordings rather than blueprints.
                        drop(all_sources);
                        self.make_store_active_and_highlight(store_hub, egui_ctx, &store_id);
                    }
                    return;
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            LogDataSource::FilePath { path, follow, .. } => {
                let new_source = LogSource::File {
                    path: path.clone(),
                    follow: *follow,
                };
                if all_sources.any(|source| source.is_same_ignoring_uri_fragments(&new_source)) {
                    drop(all_sources);
                    self.try_make_recording_from_source_active(egui_ctx, store_hub, &new_source);
                    return;
                }
            }

            LogDataSource::FileContents(_file_source, _file_contents) => {
                // For raw file contents we currently can't determine whether we're already receiving them.
            }

            #[cfg(not(target_arch = "wasm32"))]
            LogDataSource::Stdin => {
                let new_source = LogSource::Stdin;
                if all_sources.any(|source| source.is_same_ignoring_uri_fragments(&new_source)) {
                    drop(all_sources);
                    self.try_make_recording_from_source_active(egui_ctx, store_hub, &new_source);
                    return;
                }
            }

            LogDataSource::RedapDatasetSegment { uri, open_behavior } => {
                let new_source = LogSource::RedapGrpcStream {
                    uri: uri.clone(),
                    open_behavior: *open_behavior,
                    table_blueprint: None,
                };
                if all_sources.any(|source| source.is_same_ignoring_uri_fragments(&new_source)) {
                    // We're already receiving from the exact same data source!
                    // But we still should navigate if requested according to the fragments if any.
                    drop(all_sources);
                    match *open_behavior {
                        RecordingOpenBehavior::Background => {}
                        RecordingOpenBehavior::Open => {
                            store_hub.set_opened(&uri.store_id(), true);
                        }
                        RecordingOpenBehavior::OpenAndSelect => {
                            // First make the recording itself active.
                            // `go_to_dataset_data` may override the selection again, but this is important regardless,
                            // since `go_to_dataset_data` does not change the active recording.
                            self.make_store_active_and_highlight(
                                store_hub,
                                egui_ctx,
                                &uri.store_id(),
                            );
                        }
                    }

                    // Note that applying the fragment changes the per-recording settings like the active time cursor.
                    // Therefore, we apply it even when open_behavior is Background.
                    self.go_to_dataset_data(uri.store_id(), uri.fragment.clone());

                    return;
                }
            }

            LogDataSource::RedapProxy(uri) => {
                let new_source = LogSource::MessageProxy(uri.clone());
                if all_sources.any(|source| source.is_same_ignoring_uri_fragments(&new_source)) {
                    drop(all_sources);
                    self.try_make_recording_from_source_active(egui_ctx, store_hub, &new_source);
                    return;
                }
            }
        }

        let sender = self.command_sender.clone();
        let stream = data_source
            .clone()
            .stream(Self::auth_error_handler(sender), &self.connection_registry);

        #[cfg(feature = "analytics")]
        if let Some(analytics) = re_analytics::Analytics::global_or_init() {
            let data_source_analytics = data_source.analytics();
            analytics.record(re_analytics::event::LoadDataSource {
                source_type: data_source_analytics.source_type,
                file_extension: data_source_analytics.file_extension,
                file_source: data_source_analytics.file_source,
                started_successfully: stream.is_ok(),
            });
        }

        match stream {
            Ok(rx) => self.add_log_receiver(rx),
            Err(err) => {
                re_log::error!("Failed to open data source: {}", re_error::format(err));
            }
        }
    }

    /// Makes the first recording store active that is found for a given data source if any.
    fn try_make_recording_from_source_active(
        &mut self,
        egui_ctx: &egui::Context,
        store_hub: &mut StoreHub,
        new_source: &LogSource,
    ) {
        if let Some(entity_db) = store_hub.find_recording_store_by_source(new_source) {
            let store_id = entity_db.store_id().clone();
            re_log::debug_assert!(store_id.is_recording()); // `find_recording_store_by_source` should have filtered for recordings rather than blueprints.
            self.make_store_active_and_highlight(store_hub, egui_ctx, &store_id);
        }
    }
}
