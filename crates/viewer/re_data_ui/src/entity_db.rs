use egui::NumExt as _;
use jiff::SignedDuration;
use jiff::fmt::friendly::{FractionalUnit, SpanPrinter};
use re_byte_size::SizeBytes as _;
use re_chunk_store::ChunkStoreConfig;
use re_entity_db::EntityDb;
use re_log_channel::LogSource;
use re_log_types::StoreKind;
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
            {
                ui.grid_left_hand_label(&format!("{} ID", self.store_id().kind()));
                ui.label(self.store_id().recording_id().to_string());
                ui.end_row();
            }

            if let Some(LogSource::RedapGrpcStream { uri: re_uri::DatasetSegmentUri { segment_id, .. }, .. }) = &self.data_source {
                ui.grid_left_hand_label("Segment ID");
                ui.label(segment_id);
                ui.end_row();
            }

            if let Some(store_info) = self.store_info() && ui_layout.is_selection_panel() {
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
                    re_log::trace_once!(
                        "store version is undefined for this recording, this is a bug"
                    );
                }

                ui.grid_left_hand_label("Kind");
                ui.label(store_id.kind().to_string());
                ui.end_row();
            }

            let show_last_modified_time = !ctx.global_context.is_test; // Hide in tests because it is non-deterministic (it's based on `RowId`).
            if show_last_modified_time
                && let Some(latest_row_id) = self.latest_row_id()
                && let Ok(nanos_since_epoch) =
                    i64::try_from(latest_row_id.nanos_since_epoch())
            {
                let time = re_log_types::Timestamp::from_nanos_since_epoch(nanos_since_epoch);
                ui.grid_left_hand_label("Modified");
                ui.label(time.format(ctx.app_options().timestamp_format));
                ui.end_row();
            }

            if let Some(tl_name) = self.timelines().keys()
                .find(|k| **k == re_log_types::TimelineName::log_time())
                && let Some(range) = self.time_range_for(tl_name)
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

                let total_size_bytes = self.total_size_bytes();
                let static_size = if self.rrd_manifest_index().has_manifest() {
                    self.rrd_manifest_index().full_uncompressed_size().at_least(total_size_bytes)
                } else {
                    total_size_bytes
                };

                ui.label(re_format::format_bytes(static_size as _))
                    .on_hover_text(
                        "Approximate size in RAM (decompressed).\n\
                         If you hover an entity in the streams view (bottom panel) you can see the \
                         size of individual entities.",
                    );
                ui.end_row();

                if self.rrd_manifest_index().has_manifest() {
                    ui.grid_left_hand_label("Downloaded");


                    let (memory_limit, max_downloaded) = if let Some(limit) = ctx.global_context.memory_limit.max_bytes && limit < static_size {
                        (true, limit)
                    } else {
                        (false, static_size)
                    };

                    let current = re_format::format_bytes(total_size_bytes as _);
                    let max_downloaded =  re_format::format_bytes(max_downloaded as _);

                    ui.horizontal(|ui| {
                        ui.label(format!("{current} / {max_downloaded}"));

                        if memory_limit {
                            let rect = ui.small_icon(&re_ui::icons::INFO, Some(ui.visuals().text_color()));

                            ui.allocate_rect(rect, egui::Sense::hover()).on_hover_text(format!("Download limited to {max_downloaded} memory budget"));
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
                } = self.storage_engine().store().config();

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
                        reach either a maximum of {} rows ({} if unsorted) or {}, whichever comes first.

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
                        * {}
                        * {}
                        * {}

                        This compaction process is an ephemeral, in-memory optimization of the Rerun viewer.\
                        It will not modify the recording itself: use the `Save` command of the viewer, or the \
                        `rerun rrd compact` CLI tool if you wish to persist the compacted results, which will \
                        make future runs cheaper.
                        ",
                                                    re_format::format_uint(chunk_max_rows),
                                                    re_format::format_uint(chunk_max_rows_if_unsorted),
                                                    re_format::format_bytes(chunk_max_bytes as _),
                                                    ChunkStoreConfig::ENV_CHUNK_MAX_ROWS,
                                                    ChunkStoreConfig::ENV_CHUNK_MAX_ROWS_IF_UNSORTED,
                                                    ChunkStoreConfig::ENV_CHUNK_MAX_BYTES,
                        )),
                    );
                ui.end_row();
            }

            if let Some(data_source) = &self.data_source && ui_layout.is_selection_panel() {
                ui.grid_left_hand_label("Data source");
                data_source_button_ui(ctx, ui, data_source);
                ui.end_row();
            }
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
    }
}
