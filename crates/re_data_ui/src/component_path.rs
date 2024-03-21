use re_log_types::ComponentPath;
use re_viewer_context::{UiVerbosity, ViewerContext};

use super::DataUi;

impl DataUi for ComponentPath {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_data_store::LatestAtQuery,
        store: &re_data_store::DataStore,
    ) {
        let Self {
            entity_path,
            component_name,
        } = self;

        if let Some(archetype_name) = component_name.indicator_component_archetype() {
            ui.label(format!(
                "Indicator component for the {archetype_name} archetype"
            ));
        } else if let Some((_, _, component_data)) =
            re_query::get_component_with_instances(store, query, entity_path, *component_name)
        {
            super::component::EntityComponentWithInstances {
                entity_path: self.entity_path.clone(),
                component_data,
            }
            .data_ui(ctx, ui, verbosity, query, store);
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
