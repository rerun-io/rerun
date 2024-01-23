use re_types::components::{Color, Material};
use re_viewer_context::{UiVerbosity, ViewerContext};

use crate::DataUi;

impl DataUi for Material {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_data_store::LatestAtQuery,
        store: &re_data_store::DataStore,
    ) {
        let show_optional_albedo_factor = |ui: &mut egui::Ui| {
            if let Some(albedo_factor) = self.albedo_factor {
                Color(albedo_factor).data_ui(ctx, ui, verbosity, query, store);
            } else {
                ui.weak("(empty)");
            }
        };
        let show_optional_albedo_texture = |ui: &mut egui::Ui| {
            if let Some(albedo_texture) = &self.albedo_texture {
                // TODO(andreas): Either we rethink this so that albedo texture is a regular image,
                //                or we pluck apart the image preview code to be re-usable here,
                //                so that we can show an image preview.
                if let Some([height, width, _]) = albedo_texture.image_height_width_channels() {
                    ui.weak(format!("[{height} x {width}]"));
                } else {
                    ui.weak("(not an image)");
                }
            } else {
                ui.weak("(empty)");
            }
        };

        match verbosity {
            UiVerbosity::Small | UiVerbosity::Reduced => {
                show_optional_albedo_factor(ui);
            }
            UiVerbosity::Full | UiVerbosity::LimitHeight => {
                egui::Grid::new("material").num_columns(2).show(ui, |ui| {
                    ui.label("albedo_factor");
                    show_optional_albedo_factor(ui);
                    ui.end_row();
                    ui.label("albedo_texture");
                    show_optional_albedo_texture(ui);
                    ui.end_row();
                });
            }
        }
    }
}
