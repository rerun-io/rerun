use re_data_store::InstancePath;
use re_viewer_context::{UiVerbosity, ViewerContext};

use super::DataUi;

impl DataUi for re_data_store::EntityPath {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        InstancePath::entity_splat(self.clone()).data_ui(ctx, ui, verbosity, query);
    }
}
