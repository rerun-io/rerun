use re_types::components::PinholeProjection;
use re_viewer_context::{UiLayout, ViewerContext};

use crate::DataUi;

impl DataUi for PinholeProjection {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        if ui_layout.is_single_line() {
            // See if this is a trivial pinhole, and can be displayed as such:
            let fl = self.focal_length_in_pixels();
            let pp = self.principal_point();
            if *self == Self::from_focal_length_and_principal_point(fl, pp) {
                let fl = if fl.x() == fl.y() {
                    fl.x().to_string()
                } else {
                    fl.to_string()
                };

                ui_layout.label(ui, format!("Focal length: {fl}, principal point: {pp}"))
            } else {
                ui_layout.label(ui, "3×3 projection matrix")
            }
            .on_hover_ui(|ui| self.data_ui(ctx, ui, UiLayout::Tooltip, query, db));
        } else {
            self.0.data_ui(ctx, ui, ui_layout, query, db);
        }
    }
}
