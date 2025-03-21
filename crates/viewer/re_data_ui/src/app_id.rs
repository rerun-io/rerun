use itertools::Itertools as _;

use re_entity_db::EntityDb;
use re_log_types::ApplicationId;
use re_types::components::Timestamp;
use re_viewer_context::{UiLayout, ViewerContext};

use crate::item_ui::entity_db_button_ui;

impl crate::DataUi for ApplicationId {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        egui::Grid::new("application_id")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Application ID");

                let mut label = self.to_string();
                if self == &ctx.store_context.app_id {
                    label.push_str(" (active)");
                }
                UiLayout::List.label(ui, label);
                ui.end_row();
            });

        if ui_layout.is_single_line() {
            return;
        }

        // ---------------------------------------------------------------------

        // Find all recordings with this app id
        let recordings: Vec<&EntityDb> = ctx
            .store_context
            .bundle
            .recordings()
            .filter(|db| db.app_id() == Some(self))
            .sorted_by_key(|entity_db| entity_db.recording_property::<Timestamp>())
            .collect();

        // Using the same content ui also for tooltips even if it can't be interacted with.
        // (still displays the content we want)
        if !recordings.is_empty() {
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;

                ui.add_space(8.0);
                ui.strong("Loaded recordings for this app");
                for entity_db in recordings {
                    entity_db_button_ui(ctx, ui, entity_db, ui_layout, true);
                }
            });
        }
    }
}
