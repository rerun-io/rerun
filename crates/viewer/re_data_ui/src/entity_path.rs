use re_entity_db::InstancePath;
use re_viewer_context::{UiLayout, ViewerContext};

use super::DataUi;

impl DataUi for re_entity_db::EntityPath {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        InstancePath::entity_all(self.clone()).data_ui(ctx, ui, ui_layout, query, db);
    }
}
