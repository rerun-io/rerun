use re_byte_size::SizeBytes;
use re_chunk_store::ChunkStoreConfig;
use re_entity_db::EntityDb;
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
            // TODO(emilk): standardize this formatting with that in `entity_db_button_ui`
            let mut string = self.store_id().to_string();
            if let Some(data_source) = &self.data_source {
                string += &format!(", {data_source}");
            }
            if let Some(store_info) = self.store_info() {
                string += &format!(", {}", store_info.application_id);
            }
            ui.label(string);
            return;
        }

        egui::Grid::new("entity_db").num_columns(2).show(ui, |ui| {
            {
                ui.grid_left_hand_label(&format!("{} ID", self.store_id().kind));
                ui.label(self.store_id().to_string());
                ui.end_row();
            }

            if let Some(store_info) = self.store_info() {
                let re_log_types::StoreInfo {
                    application_id,
                    store_id,
                    cloned_from,
                    is_official_example: _,
                    started,
                    store_source,
                    store_version,
                } = store_info;

                if let Some(cloned_from) = cloned_from {
                    ui.grid_left_hand_label("Clone of");
                    crate::item_ui::store_id_button_ui(ctx, ui, cloned_from);
                    ui.end_row();
                }

                ui.grid_left_hand_label("Application ID");
                app_id_button_ui(ctx, ui, application_id);
                ui.end_row();

                ui.grid_left_hand_label("Source");
                ui.label(store_source.to_string());
                ui.end_row();

                if let Some(store_version) = store_version {
                    ui.grid_left_hand_label("Source RRD version");
                    ui.label(store_version.to_string());
                    ui.end_row();
                } else {
                    re_log::debug_once!(
                        "store version is undefined for this recording, this is a bug"
                    );
                }

                ui.grid_left_hand_label("Kind");
                ui.label(store_id.kind.to_string());
                ui.end_row();

                ui.grid_left_hand_label("Created");
                ui.label(started.format(ctx.app_options.time_zone));
                ui.end_row();
            }

            if let Some(latest_row_id) = self.latest_row_id() {
                if let Ok(nanos_since_epoch) =
                    i64::try_from(latest_row_id.nanoseconds_since_epoch())
                {
                    let time = re_log_types::Time::from_ns_since_epoch(nanos_since_epoch);
                    ui.grid_left_hand_label("Modified");
                    ui.label(time.format(ctx.app_options.time_zone));
                    ui.end_row();
                }
            }

            {
                ui.grid_left_hand_label("Size");
                ui.label(re_format::format_bytes(self.total_size_bytes() as _))
                    .on_hover_text(
                        "Approximate size in RAM (decompressed).\n\
                         If you hover an entity in the streams view (bottom panel) you can see the \
                         size of individual entities.",
                    );
                ui.end_row();
            }

            {
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

            if let Some(data_source) = &self.data_source {
                ui.grid_left_hand_label("Data source");
                data_source_button_ui(ctx, ui, data_source);
                ui.end_row();
            }
        });

        let hub = ctx.store_context.hub;
        let store_id = Some(self.store_id());

        match self.store_kind() {
            StoreKind::Recording => {
                if store_id.as_ref() == hub.active_recording_id() {
                    ui.add_space(8.0);
                    ui.label("This is the active recording");
                }
            }
            StoreKind::Blueprint => {
                let active_app_id = &ctx.store_context.app_id;
                let is_active_app_id = self.app_id() == Some(active_app_id);

                if is_active_app_id {
                    let is_default =
                        hub.default_blueprint_id_for_app(active_app_id) == store_id.as_ref();
                    let is_active =
                        hub.active_blueprint_id_for_app(active_app_id) == store_id.as_ref();

                    match (is_default, is_active) {
                        (false, false) => {}
                        (true, false) => {
                            ui.add_space(8.0);
                            ui.label("This is the default blueprint for the current application.");

                            if let Some(active_blueprint) =
                                hub.active_blueprint_for_app(active_app_id)
                            {
                                if active_blueprint.cloned_from() == Some(self.store_id()).as_ref()
                                {
                                    // The active blueprint is a clone of the selected blueprint.
                                    if self.latest_row_id() == active_blueprint.latest_row_id() {
                                        ui.label(
                                            "The active blueprint is a clone of this blueprint.",
                                        );
                                    } else {
                                        ui.label("The active blueprint is a modified clone of this blueprint.");
                                    }
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
