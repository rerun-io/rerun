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
        let label = if let Some(pixel_format) = self.pixel_format {
            format!("{} {}×{}", pixel_format, self.width, self.height)
        } else {
            format!(
                "{} {} {}×{}",
                self.color_model(),
                self.datatype(),
                self.width,
                self.height
            )
        };
        ui_layout.data_label(ui, label);
    }
}
