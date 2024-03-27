use re_log_types::StoreKind;
use re_viewer_context::{UiVerbosity, ViewerContext};

use crate::item_ui::entity_db_button_ui;

impl crate::DataUi for re_smart_channel::SmartChannelSource {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
        _store: &re_data_store::DataStore,
    ) {
        ui.label(self.to_string());

        if verbosity == UiVerbosity::Small {
            return;
        }

        // TODO(emilk): show if we're still connected to this data source

        // Find all stores from this data source
        // (e.g. find the recordings and blueprint in this .rrd file).
        let mut recordings = vec![];
        let mut blueprints = vec![];

        for other in ctx
            .store_context
            .bundle
            .entity_dbs_from_channel_source(self)
        {
            match other.store_kind() {
                StoreKind::Recording => {
                    recordings.push(other);
                }
                StoreKind::Blueprint => {
                    blueprints.push(other);
                }
            }
        }

        if !recordings.is_empty() {
            ui.add_space(8.0);
            ui.strong("Recordings in this data source");
            ui.indent("recordings", |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for entity_db in recordings {
                    entity_db_button_ui(ctx, ui, entity_db, true);
                }
            });
        }

        if !blueprints.is_empty() {
            ui.add_space(8.0);
            ui.strong("Blueprints in this data source");
            ui.indent("blueprints", |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for entity_db in blueprints {
                    entity_db_button_ui(ctx, ui, entity_db, true);
                }
            });
        }
    }
}
