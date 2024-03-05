use re_viewer_context::{
    BlueprintId, BlueprintIdRegistry, DataQueryId, UiVerbosity, ViewerContext,
};

use crate::{item_ui::entity_path_button_to, DataUi};

impl<T: BlueprintIdRegistry> DataUi for BlueprintId<T> {
    #[allow(clippy::only_used_in_recursion)]
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        query: &re_data_store::LatestAtQuery,
        store: &re_data_store::DataStore,
    ) {
        entity_path_button_to(
            ctx,
            query,
            store,
            ui,
            None,
            &self.as_entity_path(),
            self.to_string(),
        );
    }
}
