use re_types::blueprint::components::IncludedQuery;
use re_viewer_context::{
    BlueprintId, BlueprintIdRegistry, DataQueryId, UiVerbosity, ViewerContext,
};

use crate::{item_ui::entity_path_button_to, DataUi};

impl DataUi for IncludedQuery {
    #[allow(clippy::only_used_in_recursion)]
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_data_store::LatestAtQuery,
        store: &re_data_store::DataStore,
    ) {
        let data_query: DataQueryId = self.0.into();
        data_query.data_ui(_ctx, ui, verbosity, query, store);
        ui.end_row();
    }
}

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
