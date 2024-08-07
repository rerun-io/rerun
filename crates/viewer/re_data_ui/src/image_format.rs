use re_types::components::ImageFormat;
use re_viewer_context::{UiLayout, ViewerContext};

use crate::DataUi;

impl DataUi for ImageFormat {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        if let Some(pixel_format) = self.pixel_format {
            ui_layout.data_label(
                ui,
                format!("{}: {} × {}", pixel_format, self.width, self.height),
            );
        } else {
            ui_layout.data_label(
                ui,
                format!(
                    "{} {}: {} × {}",
                    self.color_model(),
                    self.datatype(),
                    self.width,
                    self.height
                ),
            );
        }
    }
}
