use re_data_source::{LogDataSource, LogDataSourceAnalytics};
use re_entity_db::LogSource;
use re_log_channel::{LogReceiver, RecordingOpenBehavior};
use re_log_encoding::RrdMetadata;
use re_log_types::StoreId;
use re_viewer_context::{StoreHub, SystemCommand, SystemCommandSender as _};

use super::App;

use std::path::Path;

use anyhow::Context as _;
use re_protos::cloud::v1alpha1::ext::DataSource;
use re_protos::common::v1alpha1::ext::{IfDuplicateBehavior, SegmentId};

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
            LogDataSource::HttpUrl { url } => {
                let new_source = LogSource::HttpStream {
                    url: url.to_string(),
                };

                if all_sources.any(|source| source.is_same_ignoring_uri_fragments(&new_source)) {
                    if let Some(entity_db) = store_hub.find_recording_store_by_source(&new_source) {
                        let store_id = entity_db.store_id().clone();
                        re_log::debug_assert!(store_id.is_recording()); // `find_recording_store_by_source` should have filtered for recordings rather than blueprints.
                        drop(all_sources);
                        self.make_store_active_and_highlight(store_hub, egui_ctx, &store_id);
                    }
                    return;
                }
            }

            LogDataSource::File {
                path,
                #[cfg(target_arch = "wasm32")]
                file,
                ..
            } => {
                if self.should_register_via_internal_catalog(path) {
                    self.register_via_internal_catalog(
                        path,
                        data_source.analytics(),
                        #[cfg(target_arch = "wasm32")]
                        file.clone(),
                    );
                    return;
                }

                #[cfg(target_arch = "wasm32")]
                if file.size() > f64::from(u32::MAX) {
                    if path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("rrd"))
                    {
                        // TODO(RR-5258): Remove this hint when the Viewer catalog is enabled by default.
                        re_log::error!(
                            "Failed to load file: this file is larger than the web Viewer's 4 GiB direct-load limit. Enable \"Load files via Viewer catalog\" in Settings, then open the file again.\nFile path: {}",
                            path.display()
                        );
                    } else {
                        re_log::error!(
                            "Failed to load file: this file is larger than the web Viewer's 4 GiB direct-load limit.\nFile path: {}",
                            path.display()
                        );
                    }
                    return;
                }

                // We use the file identity to check if we need to load a recording, or if we have
                // loaded it already. In the latter case, we do not load it again, and instead
                // switch to it instead.
                //
                // On web, `path` is only a display name, which is not a reliable file identity.
                //
                // TODO(grtlr): Maybe we can use the fingerprinting mechanism for this? It would
                // work on the web, and be a more reliable source for making a decision, i.e.
                // we can't know from the file path alone that a file is the same, it could have
                // changed in the meantime.
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let new_source = LogSource::File { path: path.clone() };
                    if all_sources.any(|source| source.is_same_ignoring_uri_fragments(&new_source))
                    {
                        drop(all_sources);
                        self.try_make_recording_from_source_active(
                            egui_ctx,
                            store_hub,
                            &new_source,
                        );
                        return;
                    }
                }
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
            &self.async_runtime,
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

        if !matches!(
            data_source,
            LogDataSource::RedapDatasetSegment { uri, .. }
                if self.connection_registry.is_internal_origin(&uri.origin)
        ) {
            record_catalog_load_analytics(data_source.analytics(), None, stream.is_ok());
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
            &self.async_runtime,
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

    fn should_register_via_internal_catalog(&self, path: &Path) -> bool {
        // TODO(RR-5309): Keep `.rbl` files on the legacy importer until the server supports
        // blueprint management and catalog registration can preserve `ApplicationId` retargeting.
        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("rrd"))
            && self.app_options().experimental.use_internal_catalog
            && self.connection_registry.internal_origin().is_some()
    }

    /// Registers a file with the internal catalog, then opens the segment it produced.
    fn register_via_internal_catalog(
        &self,
        path: &Path,
        data_source_analytics: LogDataSourceAnalytics,
        #[cfg(target_arch = "wasm32")] file: web_sys::File,
    ) {
        let connection_registry = self.connection_registry.clone();
        let sender = self.command_sender.clone();
        let path = path.to_owned();
        self.async_runtime.spawn_future(async move {
            let registration = register_file(
                &connection_registry,
                &path,
                #[cfg(target_arch = "wasm32")]
                file,
            )
            .await;
            match registration {
                Ok(uri) => {
                    record_catalog_load_analytics(data_source_analytics, Some("internal"), true);
                    // Refresh the dataset if it is open.
                    sender.send_system(SystemCommand::RefreshRedapEntry {
                        origin: uri.origin.clone(),
                        entry_id: uri.dataset_id.into(),
                    });
                    sender.send_system(SystemCommand::LoadDataSource(
                        LogDataSource::RedapDatasetSegment {
                            uri,
                            open_behavior: RecordingOpenBehavior::OpenAndSelect,
                        },
                    ));
                }
                Err(err) => {
                    record_catalog_load_analytics(data_source_analytics, Some("internal"), false);
                    re_log::error!(
                        "Failed to load file via the Viewer catalog: {}\nFile path: {}",
                        re_error::format(err),
                        path.display(),
                    );
                }
            }
        });
    }
}

fn record_catalog_load_analytics(
    data_source: LogDataSourceAnalytics,
    catalog_kind: Option<&'static str>,
    started_successfully: bool,
) {
    #[cfg(feature = "analytics")]
    if let Some(analytics) = re_analytics::Analytics::global_or_init() {
        analytics.record(re_analytics::event::LoadDataSource {
            source_type: data_source.source_type,
            file_extension: data_source.file_extension,
            file_source: data_source.file_source,
            catalog_kind,
            started_successfully,
        });
    }

    #[cfg(not(feature = "analytics"))]
    let _ = (data_source, catalog_kind, started_successfully);
}

/// Register an `.rrd` file the user picked with the internal catalog.
///
/// The server reads the file itself, so we only read what we need to name the dataset and to hand
/// the server a `file://` URL for the same bytes.
async fn register_file(
    connection_registry: &re_redap_client::ConnectionRegistryHandle,
    path: &Path,
    #[cfg(target_arch = "wasm32")] file: web_sys::File,
) -> anyhow::Result<re_uri::DatasetSegmentUri> {
    #[cfg(not(target_arch = "wasm32"))]
    let (reader, abs_path) = {
        let abs_path = std::path::absolute(path).with_context(|| {
            format!(
                "failed to resolve absolute path\nFile path: {}",
                path.display()
            )
        })?;
        // TODO(tokio-rs/tokio#1529): positional reads block the reactor; use `std::fs::File` until
        // an async positional file API lands (or push reads to `spawn_blocking`).
        let reader = std::fs::File::open(&abs_path)
            .with_context(|| format!("failed to open RRD\nFile path: {}", abs_path.display()))?;
        (reader, abs_path)
    };
    #[cfg(target_arch = "wasm32")]
    let reader = re_web::fs::File::from(file.clone());

    let rrd_metadata = read_rrd_metadata(&reader)
        .await
        .with_context(|| format!("failed to read RRD metadata\nFile path: {}", path.display()))?;
    rrd_metadata.store_ids.first().with_context(|| {
        format!(
            "no application id found in RRD\nFile path: {}",
            path.display()
        )
    })?;

    #[cfg(not(target_arch = "wasm32"))]
    let file_url = url::Url::from_file_path(&abs_path).map_err(|()| {
        anyhow::anyhow!(
            "not an absolute file path\nFile path: {}",
            abs_path.display()
        )
    })?;
    #[cfg(target_arch = "wasm32")]
    let file_url = copy_to_opfs(&reader, path, file).await?;

    register_rrd_file_url(connection_registry, file_url, rrd_metadata).await
}

/// Copy a browser file into OPFS and return the `file://` URL the server can read it from.
///
/// The OPFS path is content-addressed, so re-opening the same file reuses the existing copy.
#[cfg(target_arch = "wasm32")]
async fn copy_to_opfs(
    reader: &impl re_async::AsyncReadAt,
    path: &Path,
    file: web_sys::File,
) -> anyhow::Result<url::Url> {
    let file_size = reader.size().await.with_context(|| {
        format!(
            "failed to read RRD file size\nFile path: {}",
            path.display(),
        )
    })?;
    let fingerprint = re_log_encoding::RrdFingerprint::compute_for_rrd(reader)
        .await
        .with_context(|| format!("failed to fingerprint RRD\nFile path: {}", path.display()))?
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let file_name = path
        .file_name()
        .filter(|file_name| !file_name.is_empty())
        .context("OPFS upload path has no file name")?
        .to_str()
        .context("OPFS upload file name is not UTF-8")?;

    let opfs_path = std::path::PathBuf::from("/uploads")
        .join(&fingerprint)
        .join(file_name);
    if !opfs_upload_matches(&opfs_path, file_size).await?
        && let Err(err) = re_web::fs::write_file(&opfs_path, file).await
    {
        if err.kind() == std::io::ErrorKind::StorageFull {
            anyhow::bail!(
                "Viewer catalog storage quota exceeded. In Settings, under Origin private filesystem, select \"Request persistence\", then try again."
            );
        }
        return Err(err).context("failed to copy file to Viewer catalog storage");
    }

    // `Url::from_file_path` is unavailable on `wasm32-unknown-unknown`.
    let mut file_url = url::Url::parse("file:///").expect("`file:///` is a valid base URL");
    file_url
        .path_segments_mut()
        .expect("`file:///` is a base URL")
        .extend(["uploads", &fingerprint, file_name]);
    Ok(file_url)
}

/// Makes use of the fact that we don't need to scan for `default_blueprint_by_app_id`,
/// if we don't have blueprints in the RRD.
async fn read_rrd_metadata(reader: &impl re_async::AsyncReadAt) -> anyhow::Result<RrdMetadata> {
    if let Some(footer) = re_log_encoding::read_rrd_footer(reader).await?
        && footer
            .manifests
            .keys()
            .all(|store_id| !store_id.is_blueprint())
    {
        Ok(RrdMetadata {
            store_ids: footer.manifests.into_keys().collect(),
            default_blueprint_by_app_id: Default::default(),
        })
    } else {
        Ok(re_log_encoding::enumerate_legacy_metadata(reader).await?)
    }
}

#[cfg(target_arch = "wasm32")]
async fn opfs_upload_matches(path: &Path, expected_size: u64) -> anyhow::Result<bool> {
    match re_web::fs::metadata(path).await {
        Ok(metadata) => Ok(metadata.is_file() && metadata.len() == expected_size),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).context("failed to inspect Viewer catalog storage"),
    }
}

/// Register a `file://` URL the server can read with the internal catalog.
async fn register_rrd_file_url(
    connection_registry: &re_redap_client::ConnectionRegistryHandle,
    file_url: url::Url,
    rrd_metadata: re_log_encoding::RrdMetadata,
) -> anyhow::Result<re_uri::DatasetSegmentUri> {
    let application_id = rrd_metadata
        .store_ids
        .first()
        .map(StoreId::application_id)
        .context("no application id found in RRD")?;

    if rrd_metadata
        .store_ids
        .iter()
        .any(|store_id| store_id.application_id() != application_id)
    {
        re_log::warn!(
            "RRD contains multiple application ids; using the first as the dataset name: {application_id}"
        );
    }

    let origin = connection_registry
        .internal_origin()
        .context("internal catalog is not running")?;
    let mut client = connection_registry.client(origin.clone()).await?;
    let data_source = DataSource::new_rrd_url(file_url);
    let dataset_name = re_log_types::EntryName::from(application_id.clone());

    // TODO(RR-5309): Handle RRDs without recording stores as standalone blueprints.
    let (dataset_id, segment_id) = client
        .ensure_dataset_and_register(
            &dataset_name,
            vec![data_source.clone()],
            IfDuplicateBehavior::Overwrite,
        )
        .await?;

    if let Err(err) = update_default_blueprint(
        &mut client,
        dataset_id,
        &segment_id,
        data_source,
        &rrd_metadata,
    )
    .await
    {
        re_log::warn!("Failed to update default blueprint for catalog RRD load: {err:#}");
    }

    Ok(re_uri::DatasetSegmentUri {
        origin,
        dataset_id: dataset_id.id,
        segment_id,
        fragment: Default::default(),
    })
}

/// Registers the recording's embedded default blueprint (if any) into the dataset's hidden
/// blueprint dataset and records it as the dataset's default blueprint segment.
///
/// The blueprint lives in the same RRD, so we register the same `file://` URL into the blueprint
/// dataset; the server picks out the blueprint store and serves it lazily.
async fn update_default_blueprint(
    client: &mut re_redap_client::ConnectionClient,
    dataset_id: re_log_types::EntryId,
    segment_id: &SegmentId,
    data_source: DataSource,
    rrd_metadata: &re_log_encoding::RrdMetadata,
) -> anyhow::Result<()> {
    // TODO(RR-5309): Register embedded blueprints that lack a `make_default` command once the
    // server supports blueprint management.
    if rrd_metadata.default_blueprint_by_app_id.is_empty() {
        return Ok(());
    }

    let Some(recording_store_id) = rrd_metadata.store_ids.iter().find(|store_id| {
        store_id.is_recording() && SegmentId::from(store_id.recording_id()) == *segment_id
    }) else {
        re_log::warn!("Could not match registered segment {segment_id} to an RRD recording store");
        return Ok(());
    };

    let Some(default_blueprint_store_id) = rrd_metadata
        .default_blueprint_by_app_id
        .get(recording_store_id.application_id())
    else {
        return Ok(());
    };

    let mut dataset_details = client.read_dataset_entry(dataset_id).await?.dataset_details;
    let Some(blueprint_dataset_id) = dataset_details.blueprint_dataset else {
        re_log::warn!(
            "Dataset {dataset_id} has no hidden blueprint dataset; cannot set default blueprint"
        );
        return Ok(());
    };

    let expected_blueprint_segment_id = SegmentId::from(default_blueprint_store_id.recording_id());
    let (_trace_id, tasks) = client
        .register_with_dataset(
            blueprint_dataset_id,
            vec![data_source],
            IfDuplicateBehavior::Overwrite,
        )
        .await?;

    if !tasks
        .iter()
        .any(|task| task.segment_id == expected_blueprint_segment_id)
    {
        re_log::warn!(
            "Registered RRD into the blueprint dataset, but default blueprint segment \
             {expected_blueprint_segment_id} was not returned; keeping the existing default blueprint"
        );
        return Ok(());
    }

    dataset_details.default_blueprint_segment = Some(expected_blueprint_segment_id);

    client
        .update_dataset_entry(dataset_id, dataset_details)
        .await?;

    Ok(())
}
