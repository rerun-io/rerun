use re_entity_db::EntityDb;
use re_log_types::StoreKind;
use re_types::SizeBytes;
use re_viewer_context::ViewerContext;

use crate::item_ui::{data_source_button_ui, entity_db_button_ui};

impl crate::DataUi for EntityDb {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: re_viewer_context::UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
        _store: &re_data_store::DataStore,
    ) {
        let re_ui = &ctx.re_ui;

        if verbosity == re_viewer_context::UiVerbosity::Small {
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
                    store_id: _,
                    is_official_example: _,
                    started,
                    store_source,
                    store_kind,
                } = store_info;

                re_ui.grid_left_hand_label(ui, "Application ID");
                ui.label(application_id.to_string());
                ui.end_row();

                re_ui.grid_left_hand_label(ui, "Recording started");
                ui.label(started.format(ctx.app_options.time_zone));
                ui.end_row();

                re_ui.grid_left_hand_label(ui, "Source");
                ui.label(store_source.to_string());
                ui.end_row();

                re_ui.grid_left_hand_label(ui, "Kind");
                ui.label(store_kind.to_string());
                ui.end_row();
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

        if ctx.store_context.is_active(self.store_id()) {
            ui.add_space(8.0);
            match self.store_kind() {
                StoreKind::Recording => {
                    ui.label("This is the active recording");
                }
                StoreKind::Blueprint => {
                    ui.label("This is the active blueprint");
                }
            }
        }

        sibling_stores_ui(ctx, ui, self);
    }
}

/// Show the other stores in the same data source.
fn sibling_stores_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui, entity_db: &EntityDb) {
    let Some(data_source) = &entity_db.data_source else {
        return;
    };

    // Find other stores from the same data source
    // (e.g. find the blueprint in this .rrd file, if any).
    let mut other_recordings = vec![];
    let mut other_blueprints = vec![];

    for other in ctx
        .store_context
        .bundle
        .entity_dbs_from_channel_source(data_source)
    {
        if other.store_id() == entity_db.store_id() {
            continue;
        }
        match other.store_kind() {
            StoreKind::Recording => {
                other_recordings.push(other);
            }
            StoreKind::Blueprint => {
                other_blueprints.push(other);
            }
        }
    }

    if !other_recordings.is_empty() {
        ui.add_space(8.0);
        if entity_db.store_kind() == StoreKind::Recording {
            ui.strong("Other recordings in this data source");
        } else {
            ui.strong("Recordings in this data source");
        }
        ui.indent("recordings", |ui| {
            for entity_db in other_recordings {
                entity_db_button_ui(ctx, ui, entity_db, None);
            }
        });
    }
    if !other_blueprints.is_empty() {
        ui.add_space(8.0);
        if entity_db.store_kind() == StoreKind::Blueprint {
            ui.strong("Other blueprints in this data source");
        } else {
            ui.strong("Blueprints in this data source");
        }
        ui.indent("blueprints", |ui| {
            for entity_db in other_blueprints {
                entity_db_button_ui(ctx, ui, entity_db, None);
            }
        });
    }
}
