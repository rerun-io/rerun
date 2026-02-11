use re_log_types::ComponentPath;
use re_ui::UiExt as _;
use re_viewer_context::{UiLayout, ViewerContext};

use super::DataUi;

impl DataUi for ComponentPath {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        let Self {
            entity_path,
            component,
        } = self;

        let engine = db.storage_engine();

        let results = engine.cache().latest_at(query, entity_path, [*component]);
        if let Some(unit) = results.components.get(component) {
            crate::ComponentPathLatestAtResults {
                component_path: self.clone(),
                unit,
            }
            .data_ui(ctx, ui, ui_layout, query, db);
        } else if db.tree().subtree(entity_path).is_some() {
            let any_missing_chunks = !results.missing_virtual.is_empty();

            // TODO(RR-3670): figure out how to handle missing chunks
            if any_missing_chunks && db.can_fetch_chunks_from_redap() {
                ui.loading_indicator();
            } else if engine.store().entity_has_component_on_timeline(
                &query.timeline(),
                entity_path,
                *component,
            ) {
                ui.label("<unset>");
            } else {
                ui.warning_label(format!(
                    "Entity {entity_path:?} has no component {component:?}"
                ));
            }
        } else {
            ui.error_label(format!("Unknown component path: {self}"));
        }
    }
}
