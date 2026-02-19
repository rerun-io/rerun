use std::collections::BTreeSet;

use egui::NumExt as _;
use jiff::SignedDuration;
use jiff::fmt::friendly::{FractionalUnit, SpanPrinter};
use re_byte_size::SizeBytes as _;
use re_chunk_store::Chunk;
use re_chunk_store::ChunkStoreConfig;
use re_entity_db::{EntityDb, RrdManifestIndex};
use re_format::{format_bytes, format_uint};
use re_log_channel::LogSource;
use re_log_types::{EntityPath, StoreKind};
use re_ui::UiExt as _;
use re_viewer_context::{UiLayout, ViewerContext};

use crate::item_ui::{app_id_button_ui, data_source_button_ui};

impl crate::DataUi for EntityDb {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        re_tracing::profile_function!();

        if ui_layout.is_single_line() {
            // TODO(emilk): standardize this formatting with that in `entity_db_button_ui` (this is
            // probably dead code, as `entity_db_button_ui` is actually used in all single line
            // contexts).
            let mut string = self.store_id().recording_id().to_string();
            if let Some(data_source) = &self.data_source {
                string += &format!(", {data_source}");
            }
            string += &format!(", {}", self.store_id().application_id());

            ui.label(string);
            return;
        }

        egui::Grid::new("entity_db").num_columns(2).show(ui, |ui| {
            grid_content_ui(self, ctx, ui, ui_layout);
        });

        let hub = ctx.storage_context.hub;
        let store_id = Some(self.store_id());

        match self.store_kind() {
            StoreKind::Recording => {
                if false {
                    // Just confusing and unnecessary to show this.
                    if store_id == hub.active_store_id() {
                        ui.add_space(8.0);
                        ui.label("This is the active recording.");
                    }
                }
            }
            StoreKind::Blueprint => {
                let active_app_id = ctx.store_context.application_id();
                let is_active_app_id = self.application_id() == active_app_id;

                if is_active_app_id {
                    let is_default = hub.default_blueprint_id_for_app(active_app_id) == store_id;
                    let is_active = hub.active_blueprint_id_for_app(active_app_id) == store_id;

                    match (is_default, is_active) {
                        (false, false) => {}
                        (true, false) => {
                            ui.add_space(8.0);
                            ui.label("This is the default blueprint for the current application.");

                            if let Some(active_blueprint) =
                                hub.active_blueprint_for_app(active_app_id)
                                && active_blueprint.cloned_from() == Some(self.store_id())
                            {
                                // The active blueprint is a clone of the selected blueprint.
                                if self.latest_row_id() == active_blueprint.latest_row_id() {
                                    ui.label("The active blueprint is a clone of this blueprint.");
                                } else {
                                    ui.label("The active blueprint is a modified clone of this blueprint.");
                                }
                            }
                        }
                        (false, true) => {
                            ui.add_space(8.0);
                            ui.label(format!("This is the active blueprint for the current application, '{active_app_id}'"));
                        }
                        (true, true) => {
                            ui.add_space(8.0);
                            ui.label(format!("This is both the active and default blueprint for the current application, '{active_app_id}'"));
                        }
                    }
                } else {
                    ui.add_space(8.0);
                    ui.label("This blueprint is not for the active application");
                }
            }
        }

        if ctx.app_options().show_metrics
            && self.can_fetch_chunks_from_redap()
            && ui_layout.is_selection_panel()
        {
            ui.add_space(4.0);
            ui.collapsing_header("In-flight chunk requests", false, |ui| {
                chunk_requests_ui(ui, self.rrd_manifest_index());
            });
        }

        if cfg!(debug_assertions) && !ctx.app_ctx.is_test {
            ui.collapsing_header("Debug info", true, |ui| {
                debug_ui(ui, self);
            });
        }
    }
}

fn grid_content_ui(db: &EntityDb, ctx: &ViewerContext<'_>, ui: &mut egui::Ui, ui_layout: UiLayout) {
    {
        ui.grid_left_hand_label(&format!("{} ID", db.store_id().kind()));
        ui.label(db.store_id().recording_id().to_string());
        ui.end_row();
    }

    if let Some(LogSource::RedapGrpcStream {
        uri: re_uri::DatasetSegmentUri { segment_id, .. },
        ..
    }) = &db.data_source
    {
        ui.grid_left_hand_label("Segment ID");
        ui.label(segment_id);
        ui.end_row();
    }

    if let Some(store_info) = db.store_info()
        && ui_layout.is_selection_panel()
    {
        let re_log_types::StoreInfo {
            store_id,
            cloned_from,
            store_source,
            store_version,
        } = store_info;

        if let Some(cloned_from) = cloned_from {
            ui.grid_left_hand_label("Clone of");
            crate::item_ui::store_id_button_ui(ctx, ui, cloned_from, ui_layout);
            ui.end_row();
        }

        ui.grid_left_hand_label("Application ID");
        app_id_button_ui(ctx, ui, store_id.application_id());
        ui.end_row();

        ui.grid_left_hand_label("Source");
        ui.label(store_source.to_string());
        ui.end_row();

        if let Some(store_version) = store_version {
            ui.grid_left_hand_label("Source RRD version");
            ui.label(store_version.to_string());
            ui.end_row();
        } else {
            re_log::trace_once!("store version is undefined for this recording, this is a bug");
        }

        ui.grid_left_hand_label("Kind");
        ui.label(store_id.kind().to_string());
        ui.end_row();
    }

    let show_last_modified_time = !ctx.app_ctx.is_test;
    // Hide in tests because it is non-deterministic (it's based on `RowId`).
    if show_last_modified_time
        && let Some(latest_row_id) = db.latest_row_id()
        && let Ok(nanos_since_epoch) = i64::try_from(latest_row_id.nanos_since_epoch())
    {
        let time = re_log_types::Timestamp::from_nanos_since_epoch(nanos_since_epoch);
        ui.grid_left_hand_label("Modified");
        ui.label(time.format(ctx.app_options().timestamp_format));
        ui.end_row();
    }

    if let Some(tl_name) = db
        .timelines()
        .keys()
        .find(|k| **k == re_log_types::TimelineName::log_time())
        && let Some(range) = db.time_range_for(tl_name)
        && let delta_ns = (range.max() - range.min()).as_i64()
        && delta_ns > 0
    {
        let duration = SignedDuration::from_nanos(delta_ns);

        let printer = SpanPrinter::new()
            .fractional(Some(FractionalUnit::Second))
            .precision(Some(2));

        let pretty = printer.duration_to_string(&duration);

        ui.grid_left_hand_label("Duration");
        ui.label(pretty)
            .on_hover_text("Duration between earliest and latest log_time.");
        ui.end_row();
    }

    {
        ui.grid_left_hand_label("Size");

        let current_size_bytes = db.byte_size_of_physical_chunks();
        let full_size_bytes = if db.rrd_manifest_index().has_manifest() {
            db.rrd_manifest_index()
                .full_uncompressed_size()
                .at_least(current_size_bytes)
        } else {
            current_size_bytes
        };

        ui.label(format_bytes(full_size_bytes as _)).on_hover_text(
            "Approximate size in RAM (decompressed).\n\
            If you hover an entity in the streams view (bottom panel) you can see the \
            size of individual entities.",
        );
        ui.end_row();

        if db.rrd_manifest_index().has_manifest() {
            ui.grid_left_hand_label("Downloaded");

            let memory_limit = ctx.app_ctx.memory_limit;
            let max_downloaded_bytes = if db.rrd_manifest_index().is_fully_loaded() {
                full_size_bytes
            } else {
                u64::min(full_size_bytes, memory_limit.as_bytes())
            };

            let current_size = format_bytes(current_size_bytes as _);
            let max_downloaded = format_bytes(max_downloaded_bytes as _);

            ui.horizontal(|ui| {
                let mut num_root_chunks = 0_usize;
                let mut num_fully_loaded = 0_usize;
                for info in db.rrd_manifest_index().root_chunks() {
                    num_root_chunks += 1;
                    if info.is_fully_loaded() {
                        num_fully_loaded += 1;
                    }
                }

                if num_fully_loaded == num_root_chunks {
                    ui.label("100%");
                } else {
                    ui.label(format!("{current_size} / {max_downloaded}"));

                    if max_downloaded_bytes < full_size_bytes {
                        let rect =
                            ui.small_icon(&re_ui::icons::INFO, Some(ui.visuals().text_color()));

                        ui.allocate_rect(rect, egui::Sense::hover())
                            .on_hover_text(format!(
                                "Download limited to {memory_limit} memory budget"
                            ));
                    }

                    ui.label(format!(
                        "({} / {} chunks)",
                        format_uint(num_fully_loaded),
                        format_uint(num_root_chunks)
                    ));
                    ui.end_row();
                }
            });

            ui.end_row();
        }
    }

    if ui_layout.is_selection_panel() {
        let &ChunkStoreConfig {
            enable_changelog: _,
            chunk_max_bytes,
            chunk_max_rows,
            chunk_max_rows_if_unsorted,
        } = db.storage_engine().store().config();

        ui.grid_left_hand_label("Compaction");
        ui.label(format!(
            "{} rows ({} if unsorted) or {}",
            re_format::format_uint(chunk_max_rows),
            re_format::format_uint(chunk_max_rows_if_unsorted),
            re_format::format_bytes(chunk_max_bytes as _),
        ))
            .on_hover_text(
                unindent::unindent(&format!("\
                    The current compaction configuration for this recording is to merge chunks until they \
                    reach either a maximum of {chunk_max_rows} rows ({chunk_max_rows_if_unsorted} if unsorted) or {chunk_max_bytes}, whichever comes first.

                    The viewer compacts chunks together as they come in, in order to find the right \
                    balance between space and compute overhead.
                    This is not to be confused with the SDK's batcher, which does a similar job, with \
                    different goals and constraints, on the logging side (SDK).
                    These two functions (SDK batcher & viewer compactor) complement each other.

                    Higher thresholds generally translate to better space overhead, but require more compute \
                    for both ingestion and queries.
                    Lower thresholds generally translate to worse space overhead, but faster ingestion times
                    and more responsive queries.
                    This is a broad oversimplification -- use the defaults if unsure, they fit most workfloads well.

                    To modify the current configuration, set these environment variables before starting the viewer:
                    * {ENV_CHUNK_MAX_ROWS}
                    * {ENV_CHUNK_MAX_ROWS_IF_UNSORTED}
                    * {ENV_CHUNK_MAX_BYTES}

                    This compaction process is an ephemeral, in-memory optimization of the Rerun viewer.\
                    It will not modify the recording itself: use the `Save` command of the viewer, or the \
                    `rerun rrd compact` CLI tool if you wish to persist the compacted results, which will \
                    make future runs cheaper.
                    ",
                        chunk_max_rows = re_format::format_uint(chunk_max_rows),
                        chunk_max_rows_if_unsorted = re_format::format_uint(chunk_max_rows_if_unsorted),
                        chunk_max_bytes = re_format::format_bytes(chunk_max_bytes as _),
                        ENV_CHUNK_MAX_ROWS = ChunkStoreConfig::ENV_CHUNK_MAX_ROWS,
                        ENV_CHUNK_MAX_ROWS_IF_UNSORTED = ChunkStoreConfig::ENV_CHUNK_MAX_ROWS_IF_UNSORTED,
                        ENV_CHUNK_MAX_BYTES = ChunkStoreConfig::ENV_CHUNK_MAX_BYTES,
                )),
            );
        ui.end_row();
    }

    if let Some(data_source) = &db.data_source
        && ui_layout.is_selection_panel()
    {
        ui.grid_left_hand_label("Data source");
        data_source_button_ui(ctx, ui, data_source);
        ui.end_row();
    }
}

fn chunk_requests_ui(ui: &mut egui::Ui, rrd_manifest_index: &RrdManifestIndex) {
    let Some(rrd_manifest) = rrd_manifest_index.manifest() else {
        return;
    };

    let chunk_requests = rrd_manifest_index.chunk_requests();
    let requests = chunk_requests.pending_requests();

    let col_chunk_entity_path_raw = rrd_manifest.col_chunk_entity_path_raw();

    let mut entities = BTreeSet::<EntityPath>::new();
    let mut total_in_flight_bytes = 0;
    let mut total_uncompressed_bytes = 0;
    let mut total_chunks = 0;
    for request in &requests {
        total_in_flight_bytes += request.size_bytes_on_wire;
        total_uncompressed_bytes += request.size_bytes_uncompressed;
        total_chunks += request.row_indices.len() as u64;

        for &row_idx in &request.row_indices {
            let path = col_chunk_entity_path_raw.value(row_idx);
            entities.insert(EntityPath::parse_forgiving(path));
        }
    }

    ui.label("Data currently being downloaded from the server");

    egui::Grid::new("chunk-requests").show(ui, |ui| {
        ui.label("Speed");
        if let Some(bytes_per_second) = chunk_requests.bandwidth() {
            ui.label(format!("{}/s", format_bytes(bytes_per_second)));
            if 0.0 < bytes_per_second {
                ui.ctx().request_repaint(); // Show latest estimate
            }
        }
        ui.end_row();

        ui.label("Requests");
        ui.label(format_uint(requests.len()));
        ui.end_row();

        ui.label("Chunks");
        ui.label(format_uint(total_chunks));
        ui.end_row();

        ui.label("Recently canceled");
        ui.label(format_uint(
            chunk_requests
                .recently_canceled
                .iter()
                .map(|(_time, count)| count)
                .sum::<usize>(),
        ));
        ui.end_row();

        ui.label("Bytes (compressed)");
        ui.label(format_bytes(total_in_flight_bytes as _));
        ui.end_row();

        ui.label("Bytes (uncompressed)");
        ui.label(format_bytes(total_uncompressed_bytes as _));
        ui.end_row();

        ui.label("Entities");
        ui.label(format_uint(entities.len()));
        ui.end_row();
    });

    for entity in &entities {
        ui.label(format!("  - {entity}"));
    }
}

fn debug_ui(ui: &mut egui::Ui, db: &EntityDb) {
    ui.weak("(only visible in debug builds)");
    egui::Grid::new("debug-info").show(ui, |ui| {
        ui.label("is_buffering");
        ui.label(db.is_buffering().to_string());
        ui.end_row();

        ui.label("Physical chunks");
        ui.label(format_bytes(db.byte_size_of_physical_chunks() as _));
        ui.end_row();

        ui.label("App overhead");
        if let Some(overhead) = db.estimated_application_overhead_bytes {
            ui.label(format_bytes(overhead as _));
        }
        ui.end_row();
    });

    protected_chunks_ui(ui, db);
}

fn protected_chunks_ui(ui: &mut egui::Ui, db: &EntityDb) {
    #![expect(clippy::iter_over_hash_type)] // just summing sizes, order doesn't matter

    let rrd_manifest_index = db.rrd_manifest_index();
    let protected = rrd_manifest_index.chunk_prioritizer().protected_chunks();

    if protected.roots.is_empty() && protected.physical.is_empty() {
        return;
    }

    let manifest = rrd_manifest_index.manifest();
    let store = db.storage_engine();
    let store = store.store();

    // Compute root (virtual) chunk sizes from the manifest
    let mut roots_total_bytes: u64 = 0;
    if let Some(manifest) = &manifest {
        let col_sizes = manifest.col_chunk_byte_size_uncompressed();
        for root_id in &protected.roots {
            if let Some(info) = rrd_manifest_index.root_chunk_info(root_id) {
                roots_total_bytes += col_sizes[info.row_id];
            }
        }
    }

    // Compute physical chunk sizes from the store
    let mut physical_total_bytes: u64 = 0;
    for chunk_id in &protected.physical {
        if let Some(chunk) = store.physical_chunk(chunk_id) {
            physical_total_bytes += Chunk::total_size_bytes(chunk.as_ref());
        }
    }

    ui.add_space(4.0);
    ui.label("Protected chunks");
    egui::Grid::new("protected-chunks").show(ui, |ui| {
        ui.label("Roots");
        ui.label(format!(
            "{} chunks, {}",
            format_uint(protected.roots.len()),
            format_bytes(roots_total_bytes as _),
        ));
        ui.end_row();

        ui.label("Physical");
        ui.label(format!(
            "{} chunks, {}",
            format_uint(protected.physical.len()),
            format_bytes(physical_total_bytes as _),
        ));
        ui.end_row();
    });
}
