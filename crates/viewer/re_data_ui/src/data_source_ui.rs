use re_log_types::StoreKind;
use re_sdk_types::archetypes::RecordingInfo;
use re_sdk_types::components::Timestamp;
use re_viewer_context::{UiLayout, ViewerContext};

use crate::item_ui::entity_db_button_ui;

impl crate::DataUi for re_log_channel::LogSource {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        ui.label(self.to_string());

        if ui_layout.is_single_line() {
            return;
        }

        // TODO(emilk): show whether we're still connected to this data source

        // Find all stores from this data source
        // (e.g. find the recordings and blueprint in this .rrd file).
        let mut recordings = vec![];
        let mut blueprints = vec![];

        for other in ctx
            .store_bundle()
            .entity_dbs()
            .filter(|db| db.data_source.as_ref() == Some(self))
        {
            let is_clone = other.cloned_from().is_some();
            if is_clone {
                // Clones are not really from this data source (e.g. a cloned blueprint
                continue;
            }

            match other.store_kind() {
                StoreKind::Recording => {
                    recordings.push(other);
                }
                StoreKind::Blueprint => {
                    blueprints.push(other);
                }
            }
        }

        let start_time_component = RecordingInfo::descriptor_start_time().component;
        recordings.sort_by_key(|entity_db| {
            entity_db.recording_info_property::<Timestamp>(start_time_component)
        });
        // TODO(grtlr): Blueprints don't have a time yet. But do we even need that?
        blueprints.sort_by_key(|entity_db| {
            entity_db.recording_info_property::<Timestamp>(start_time_component)
        });

        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if !recordings.is_empty() {
                ui.add_space(8.0);
                ui.strong("Recordings from this data source");
                for entity_db in recordings {
                    entity_db_button_ui(ctx, ui, entity_db, ui_layout, true);
                }
            }

            if !blueprints.is_empty() {
                ui.add_space(8.0);
                ui.strong("Blueprints from this data source");
                for entity_db in blueprints {
                    entity_db_button_ui(ctx, ui, entity_db, ui_layout, true);
                }
            }
        });
    }
}
