use re_types::components::{PinholeProjection, Resolution};
use re_viewer_context::{UiLayout, ViewerContext};

use crate::{label_for_ui_layout, DataUi};

impl DataUi for PinholeProjection {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        match ui_layout {
            UiLayout::List => {
                // See if this is a trivial pinhole, and can be displayed as such:
                let fl = self.focal_length_in_pixels();
                let pp = self.principal_point();
                if *self == Self::from_focal_length_and_principal_point(fl, pp) {
                    let fl = if fl.x() == fl.y() {
                        fl.x().to_string()
                    } else {
                        fl.to_string()
                    };

                    label_for_ui_layout(
                        ui,
                        ui_layout,
                        format!("Focal length: {fl}, principal point: {pp}"),
                    )
                } else {
                    label_for_ui_layout(ui, ui_layout, "3x3 projection matrix")
                }
                .on_hover_ui(|ui| self.data_ui(ctx, ui, UiLayout::Tooltip, query, db));
            }
            _ => {
                self.0.data_ui(ctx, ui, ui_layout, query, db);
            }
        }
    }
}

impl DataUi for Resolution {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        let [x, y] = self.0 .0;
        label_for_ui_layout(ui, ui_layout, format!("{x}x{y}"));
    }
}
