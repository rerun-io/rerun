use re_log_types::ComponentPath;
use re_viewer_context::{UiVerbosity, ViewerContext};

use super::DataUi;

impl DataUi for ComponentPath {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        let Self {
            entity_path,
            component_name,
        } = self;

        let store = &ctx.store_db.entity_db.data_store;

        if let Some(archetype_name) = crate::indicator_component_archetype(component_name) {
            ui.label(format!(
                "Indicator component for the {archetype_name} archetype"
            ));
        } else if let Some((_, component_data)) =
            re_query::get_component_with_instances(store, query, entity_path, *component_name)
        {
            super::component::EntityComponentWithInstances {
                entity_path: self.entity_path.clone(),
                component_data,
            }
            .data_ui(ctx, ui, verbosity, query);
        } else if let Some(entity_tree) = ctx.store_db.entity_db.tree.subtree(entity_path) {
            if entity_tree.components.contains_key(component_name) {
                ui.label("<unset>");
            } else {
                ui.label(format!(
                    "Entity {entity_path:?} has no component {component_name:?}"
                ));
            }
        } else {
            ui.label(
                ctx.re_ui
                    .error_text(format!("Unknown entity: {entity_path:?}")),
            );
        }
    }
}
