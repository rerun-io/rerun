use std::sync::Arc;

use re_log_types::ComponentPath;
use re_viewer_context::{UiLayout, ViewerContext};

use super::DataUi;

impl DataUi for ComponentPath {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        let Self {
            entity_path,
            component_name,
        } = self;

        if let Some(archetype_name) = component_name.indicator_component_archetype() {
            ui.label(format!(
                "Indicator component for the {archetype_name} archetype"
            ));
        } else {
            let results =
                db.query_caches()
                    .latest_at(db.store(), query, entity_path, [*component_name]);
            if let Some(results) = results.components.get(component_name) {
                crate::EntityLatestAtResults {
                    entity_path: entity_path.clone(),
                    component_name: *component_name,
                    results: Arc::clone(results),
                }
                .data_ui(ctx, ui, ui_layout, query, db);
            } else if let Some(entity_tree) = ctx.recording().tree().subtree(entity_path) {
                if entity_tree.entity.components.contains_key(component_name) {
                    ui.label("<unset>");
                } else {
                    ui.label(format!(
                        "Entity {entity_path:?} has no component {component_name:?}"
                    ));
                }
            } else {
                ui.label(
                    ctx.re_ui
                        .error_text(format!("Unknown component path: {self}")),
                );
            }
        }
    }
}
