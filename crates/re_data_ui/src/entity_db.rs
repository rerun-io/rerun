use re_entity_db::EntityDb;
use re_log_types::StoreKind;
use re_types::SizeBytes;
use re_viewer_context::{UiVerbosity, ViewerContext};

use crate::item_ui::{app_id_button_ui, data_source_button_ui};

impl crate::DataUi for EntityDb {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        let re_ui = &ctx.re_ui;

        if verbosity == UiVerbosity::Small {
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
                re_ui.grid_left_hand_label(ui, &format!("{} ID", self.store_id().kind));
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
                } = store_info;

                if let Some(cloned_from) =  cloned_from {
                    re_ui.grid_left_hand_label(ui, "Clone of");
                    crate::item_ui::store_id_button_ui(ctx, ui, cloned_from);
                    ui.end_row();
                }

                re_ui.grid_left_hand_label(ui, "Application ID");
                app_id_button_ui(ctx, ui, application_id);
                ui.end_row();

                re_ui.grid_left_hand_label(ui, "Source");
                ui.label(store_source.to_string());
                ui.end_row();

                re_ui.grid_left_hand_label(ui, "Kind");
                ui.label(store_id.kind.to_string());
                ui.end_row();

                re_ui.grid_left_hand_label(ui, "Created");
                ui.label(started.format(ctx.app_options.time_zone));
                ui.end_row();
            }

            if let Some(latest_row_id) = self.latest_row_id() {
                if let Ok(nanos_since_epoch) = i64::try_from(latest_row_id.nanoseconds_since_epoch()) {
                    let time = re_log_types::Time::from_ns_since_epoch(nanos_since_epoch);
                    re_ui.grid_left_hand_label(ui, "Modified");
                    ui.label(time.format(ctx.app_options.time_zone));
                    ui.end_row();
                }
            }

            {
                re_ui.grid_left_hand_label(ui, "Size");
                ui.label(re_format::format_bytes(self.total_size_bytes() as _))
                    .on_hover_text(
                        "Approximate size in RAM (decompressed).\n\
                         If you hover an entity in the streams view (bottom panel) you can see the size of individual entities.");
                ui.end_row();
            }

            if let Some(data_source) = &self.data_source {
                re_ui.grid_left_hand_label(ui, "Data source");
                data_source_button_ui(ctx, ui, data_source);
                ui.end_row();
            }
        });

        let hub = ctx.store_context.hub;

        match self.store_kind() {
            StoreKind::Recording => {
                if Some(self.store_id()) == hub.active_recording_id() {
                    ui.add_space(8.0);
                    ui.label("This is the active recording");
                }
            }
            StoreKind::Blueprint => {
                let active_app_id = &ctx.store_context.app_id;
                let is_active_app_id = self.app_id() == Some(active_app_id);

                if is_active_app_id {
                    let is_default =
                        hub.default_blueprint_id_for_app(active_app_id) == Some(self.store_id());
                    let is_active =
                        hub.active_blueprint_id_for_app(active_app_id) == Some(self.store_id());

                    match (is_default, is_active) {
                        (false, false) => {}
                        (true, false) => {
                            ui.add_space(8.0);
                            ui.label("This is the default blueprint for the current application.");

                            if let Some(active_blueprint) =
                                hub.active_blueprint_for_app(active_app_id)
                            {
                                if active_blueprint.cloned_from() == Some(self.store_id()) {
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
