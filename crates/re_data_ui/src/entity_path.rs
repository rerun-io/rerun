use re_entity_db::InstancePath;
use re_viewer_context::{UiVerbosity, ViewerContext};

use super::DataUi;

impl DataUi for re_entity_db::EntityPath {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_data_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        InstancePath::entity_splat(self.clone()).data_ui(ctx, ui, verbosity, query, db);
    }
}
