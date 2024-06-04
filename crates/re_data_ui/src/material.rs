use re_types::components::{Color, Material};
use re_viewer_context::{UiLayout, ViewerContext};

use crate::DataUi;

impl DataUi for Material {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store2::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        let show_optional_albedo_factor = |ui: &mut egui::Ui| {
            if let Some(albedo_factor) = self.albedo_factor {
                Color(albedo_factor).data_ui(ctx, ui, ui_layout, query, db);
            } else {
                ui.weak("(empty)");
            }
        };

        match ui_layout {
            UiLayout::List | UiLayout::Tooltip => {
                show_optional_albedo_factor(ui);
            }
            UiLayout::SelectionPanelFull | UiLayout::SelectionPanelLimitHeight => {
                egui::Grid::new("material").num_columns(2).show(ui, |ui| {
                    ui.label("albedo_factor");
                    show_optional_albedo_factor(ui);
                    ui.end_row();
                });
            }
        }
    }
}
