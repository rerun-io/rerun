use re_types::blueprint::components::IncludedQueries;
use re_viewer_context::{
    BlueprintId, BlueprintIdRegistry, DataQueryId, UiVerbosity, ViewerContext,
};

use crate::{item_ui::entity_path_button_to, DataUi};

impl DataUi for IncludedQueries {
    #[allow(clippy::only_used_in_recursion)]
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        match verbosity {
            UiVerbosity::Small => {
                ui.label(format!("{} Queries", self.0.len()));
            }
            UiVerbosity::Full | UiVerbosity::LimitHeight | UiVerbosity::Reduced => {
                for query in &self.0 {
                    let query: DataQueryId = (*query).into();
                    query.data_ui(_ctx, ui, verbosity, _query);
                    ui.end_row();
                }
            }
        }
    }
}

impl<T: BlueprintIdRegistry> DataUi for BlueprintId<T> {
    #[allow(clippy::only_used_in_recursion)]
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        entity_path_button_to(ctx, ui, None, &self.as_entity_path(), self.to_string());
    }
}
