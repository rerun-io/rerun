use re_data_source::LogDataSource;
use re_entity_db::LogSource;
use re_log_channel::{LogReceiver, RecordingOpenBehavior};
use re_log_types::StoreId;
use re_sdk_types::blueprint::components::PlayState;
use re_viewer_context::{StoreHub, SystemCommand, SystemCommandSender as _, TimeControlCommand};

use super::App;

#[cfg(all(feature = "internal_catalog", not(target_arch = "wasm32")))]
use {
    anyhow::Context as _, re_protos::cloud::v1alpha1::ext::DataSource,
    re_protos::common::v1alpha1::ext::IfDuplicateBehavior, std::path::Path,
};

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
            if self.connection_registry.is_internal_origin(&uri.origin) {
                self.rx_log.add(rx);
                return;
            }

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
                #[cfg(all(feature = "internal_catalog", not(target_arch = "wasm32")))]
                {
                    // If the internal catalog is enabled, route `.rrd` files through it.
                    //
                    // TODO(RR-5039): `follow` (tailing a growing file) is incompatible with
                    // this, followed files keep the direct loading path.
                    if !*follow
                        && path.extension().is_some_and(|ext| ext == "rrd")
                        && self.app_options().experimental.use_internal_catalog
                        && self.connection_registry.internal_origin().is_some()
                    {
                        let path = path.clone();
                        let connection_registry = self.connection_registry.clone();
                        let sender = self.command_sender.clone();
                        self.async_runtime.spawn_future(async move {
                            match register_local_file(&connection_registry, &path).await {
                                Ok(uri) => {
                                    sender.send_system(SystemCommand::LoadDataSource(
                                        LogDataSource::RedapDatasetSegment {
                                            uri,
                                            open_behavior: RecordingOpenBehavior::OpenAndSelect,
                                        },
                                    ));
                                }
                                Err(err) => {
                                    re_log::error!(
                                        "Failed to load file via the Viewer catalog: {err}\nFile path: {}",
                                        path.display(),
                                    );
                                }
                            }
                        });
                        return;
                    }
                }

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
                            // `make_store_active_and_highlight` also fetches the blueprint we skipped
                            // while this was a preview.
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

        let stream = data_source.clone().stream_with_options(
            Self::auth_error_handler(self.command_sender.clone()),
            &self.connection_registry,
            if let LogDataSource::RedapDatasetSegment { open_behavior, .. } = &data_source
                && matches!(open_behavior, RecordingOpenBehavior::Background)
            {
                // Previews skip the blueprint; we fetch it later if the user opens the recording for real.
                re_redap_client::StreamingOptions {
                    download: re_redap_client::SegmentDownload::SEGMENT,
                    ..Default::default()
                }
            } else {
                Default::default()
            },
        );

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

    /// Fetch the server blueprint for a recording that was streamed as a preview, which skips it.
    ///
    /// Does nothing unless the recording hasn't fetched its blueprint.
    pub(super) fn fetch_pending_blueprint(&mut self, store_hub: &mut StoreHub, store_id: &StoreId) {
        if !store_hub.is_blueprint_pending(store_id) {
            return;
        }

        let Some(LogSource::RedapGrpcStream { uri, .. }) = store_hub
            .entity_db(store_id)
            .and_then(|db| db.data_source.clone())
        else {
            return;
        };
        let data_source = LogDataSource::RedapDatasetSegment {
            uri: uri.without_fragment(),
            open_behavior: RecordingOpenBehavior::Background,
        };
        match data_source.stream_with_options(
            Self::auth_error_handler(self.command_sender.clone()),
            &self.connection_registry,
            re_redap_client::StreamingOptions {
                download: re_redap_client::SegmentDownload::BLUEPRINT,
                ..Default::default()
            },
        ) {
            Ok(rx) => {
                store_hub.set_blueprint_pending(store_id, false);
                self.add_log_receiver(rx);
            }
            Err(err) => {
                re_log::error!("Failed to fetch blueprint: {}", re_error::format(err));
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

/// Register a local `.rrd` file with the catalog server.
#[cfg(all(feature = "internal_catalog", not(target_arch = "wasm32")))]
async fn register_local_file(
    connection_registry: &re_redap_client::ConnectionRegistryHandle,
    path: &Path,
) -> anyhow::Result<re_uri::DatasetSegmentUri> {
    let origin = connection_registry
        .internal_origin()
        .context("internal catalog is not running")?;
    let mut client = connection_registry.client(origin.clone()).await?;

    let abs_path = std::path::absolute(path).with_context(|| {
        format!(
            "failed to resolve absolute path\nFile path: {}",
            path.display()
        )
    })?;
    let file_url = url::Url::from_file_path(&abs_path).map_err(|()| {
        anyhow::anyhow!(
            "not an absolute file path\nFile path: {}",
            abs_path.display()
        )
    })?;

    let dataset_name = std::fs::File::open(&abs_path)
        .with_context(|| {
            format!(
                "failed to open RRD for application id extraction\nFile path: {}",
                abs_path.display(),
            )
        })
        .and_then(|mut file| {
            let file: &mut std::fs::File = &mut file;
            let store_ids = re_log_encoding::enumerate_rrd_stores(file)?;
            let first_application_id = store_ids
                .first()
                .map(re_log_types::StoreId::application_id)
                .context("no application id found in RRD")?;

            if store_ids
                .iter()
                .any(|store_id| store_id.application_id() != first_application_id)
            {
                re_log::warn!(
                    "RRD contains multiple application ids; using the first as the dataset name: {first_application_id}"
                );
            }

            Ok(first_application_id.to_string())
        })
        .unwrap_or_else(|err| {
            re_log::warn!(
                "Failed to read application id from RRD: {err}\nFile path: {}",
                abs_path.display(),
            );
            let path: &Path = &abs_path;
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("recording")
                .to_owned()
        });

    let data_source = DataSource::new_rrd(file_url.as_str())?;

    let (dataset_id, segment_id) = client
        .ensure_dataset_and_register(
            &dataset_name,
            vec![data_source],
            IfDuplicateBehavior::Overwrite,
        )
        .await?;

    Ok(re_uri::DatasetSegmentUri {
        origin,
        dataset_id: dataset_id.id,
        segment_id,
        fragment: Default::default(),
    })
}
