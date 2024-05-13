use re_log_types::StoreKind;
use re_viewer_context::{UiLayout, ViewerContext};

use crate::item_ui::entity_db_button_ui;

impl crate::DataUi for re_smart_channel::SmartChannelSource {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        ui.label(self.to_string());

        if ui_layout == UiLayout::List {
            return;
        }

        // TODO(emilk): show whether we're still connected to this data source

        // Find all stores from this data source
        // (e.g. find the recordings and blueprint in this .rrd file).
        let mut recordings = vec![];
        let mut blueprints = vec![];

        for other in ctx
            .store_context
            .bundle
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

        recordings.sort_by_key(|entity_db| entity_db.store_info().map(|info| info.started));
        blueprints.sort_by_key(|entity_db| entity_db.store_info().map(|info| info.started));

        //TODO(#6245): we should _not_ use interactive UI in code used for hover tooltip!
        let content_ui = |ui: &mut egui::Ui| {
            if !recordings.is_empty() {
                ui.add_space(8.0);
                ui.strong("Recordings from this data source");
                for entity_db in recordings {
                    entity_db_button_ui(ctx, ui, entity_db, true);
                }
            }

            if !blueprints.is_empty() {
                ui.add_space(8.0);
                ui.strong("Blueprints from this data source");
                for entity_db in blueprints {
                    entity_db_button_ui(ctx, ui, entity_db, true);
                }
            }
        };
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;

            // TODO(#6246): this test is needed because we're called in a context that may or may
            // not have a full span defined.
            if ui_layout == UiLayout::Tooltip {
                // This typically happens in tooltips, so a scope is needed
                //TODO(ab): in the context of tooltips, ui.max_rect() doesn't provide the correct width
                re_ui::full_span::full_span_scope(ui, ui.max_rect().x_range(), content_ui);
            } else {
                // This only happens from the selection panel, so the full span scope is already set.
                content_ui(ui);
            }
        });
    }
}
