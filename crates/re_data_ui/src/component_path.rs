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
        let store = &ctx.log_db.entity_db.data_store;

        if let Some((_, component_data)) = re_query::get_component_with_instances(
            store,
            query,
            self.entity_path(),
            self.component_name,
        ) {
            super::component::EntityComponentWithInstances {
                entity_path: self.entity_path.clone(),
                component_data,
            }
            .data_ui(ctx, ui, verbosity, query);
        } else {
            ui.label("<unset>");
        }
    }
}
