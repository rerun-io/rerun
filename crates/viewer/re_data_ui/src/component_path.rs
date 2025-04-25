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
            component_name,
        } = self;

        let engine = db.storage_engine();

        if let Some(archetype_name) = component_name.indicator_component_archetype_short_name() {
            ui.label(format!(
                "Indicator component for the {archetype_name} archetype"
            ));
        } else {
            let results = engine
                .cache()
                .latest_at(query, entity_path, [*component_name]);
            if let Some(unit) = results.components.get(component_name) {
                crate::ComponentPathLatestAtResults {
                    component_path: self.clone(),
                    unit,
                }
                .data_ui(ctx, ui, ui_layout, query, db);
            } else if ctx.recording().tree().subtree(entity_path).is_some() {
                if engine.store().entity_has_component_on_timeline(
                    &query.timeline(),
                    entity_path,
                    component_name,
                ) {
                    ui.label("<unset>");
                } else {
                    ui.label(format!(
                        "Entity {entity_path:?} has no component {component_name:?}"
                    ));
                }
            } else {
                ui.error_label(format!("Unknown component path: {self}"));
            }
        }
    }
}
