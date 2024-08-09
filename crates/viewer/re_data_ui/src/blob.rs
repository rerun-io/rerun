use re_types::components::MediaType;

use crate::EntityDataUi;

impl EntityDataUi for re_types::components::Blob {
    fn entity_data_ui(
        &self,
        ctx: &re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: re_viewer_context::UiLayout,
        entity_path: &re_log_types::EntityPath,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        let size_string = format!("{} B", re_format::format_uint(self.len()));
        let guessed_media_type = MediaType::guess_from_data(self);

        ui.horizontal(|ui| {
            ui.label(size_string);

            if let Some(media_type) = guessed_media_type {
                ui.label(media_type.to_string())
                    .on_hover_text("Media type (MIME) based on magic header bytes");
            }
        });
    }
}
