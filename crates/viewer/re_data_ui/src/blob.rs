use re_types::components::{Blob, MediaType};
use re_ui::{list_item::PropertyContent, UiExt as _};
use re_viewer_context::UiLayout;

use crate::{image::image_preview_ui, EntityDataUi};

impl EntityDataUi for Blob {
    fn entity_data_ui(
        &self,
        ctx: &re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        entity_path: &re_log_types::EntityPath,
        row_id: Option<re_chunk_store::RowId>,
        query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        let compact_size_string = re_format::format_bytes(self.len() as _);

        // We show the actual mime of the blob here instead of doing
        // a side-lookup of the sibling `MediaType` component.
        // This is part of "showing the data as it is".
        // If the user clicked on the blob, is because they want to see info about the blob,
        // not about a sibling component.
        // This can also help a user debug if they log the contents of `.png` file with a `image/jpeg` `MediaType`.
        let media_type = MediaType::guess_from_data(self);

        if ui_layout.is_single_line() {
            ui.horizontal(|ui| {
                ui.label(compact_size_string);

                if let Some(media_type) = &media_type {
                    ui.label(media_type.to_string())
                        .on_hover_text("Media type (MIME) based on magic header bytes");
                }

                blob_preview_and_save_ui(
                    ctx,
                    ui,
                    ui_layout,
                    query,
                    entity_path,
                    row_id,
                    self,
                    media_type.as_ref(),
                );
            });
        } else {
            let all_digits_size_string = format!("{} B", re_format::format_uint(self.len()));
            let size_string = if self.len() < 1024 {
                all_digits_size_string
            } else {
                format!("{all_digits_size_string} ({compact_size_string})")
            };

            ui.list_item_flat_noninteractive(PropertyContent::new("Size").value_text(size_string));

            if let Some(media_type) = &media_type {
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Media type").value_text(media_type.as_str()),
                )
                .on_hover_text("Media type (MIME) based on magic header bytes");
            } else {
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Media type").value_text("?"),
                )
                .on_hover_text("Failed to detect media type (Mime) from magic header bytes");
            }

            blob_preview_and_save_ui(
                ctx,
                ui,
                ui_layout,
                query,
                entity_path,
                row_id,
                self,
                media_type.as_ref(),
            );
        }
    }
}

// TODO(jan): this should NOT try to load videos as images
#[allow(clippy::too_many_arguments)]
pub fn blob_preview_and_save_ui(
    ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
    blob_row_id: Option<re_chunk_store::RowId>,
    blob: &[u8],
    media_type: Option<&MediaType>,
) {
    let image = blob_row_id.and_then(|row_id| {
        ctx.cache
            .entry(|c: &mut re_viewer_context::ImageDecodeCache| {
                c.entry(row_id, blob, media_type.as_ref().map(|mt| mt.as_str()))
            })
            .ok()
    });

    if let Some(image) = &image {
        image_preview_ui(ctx, ui, ui_layout, query, entity_path, image);
    }

    if !ui_layout.is_single_line() && ui_layout != UiLayout::Tooltip {
        ui.horizontal(|ui| {
            let text = if cfg!(target_arch = "wasm32") {
                "Download blob…"
            } else {
                "Save blob…"
            };
            if ui.button(text).clicked() {
                let mut file_name = entity_path
                    .last()
                    .map_or("blob", |name| name.unescaped_str())
                    .to_owned();

                if let Some(file_extension) = media_type.as_ref().and_then(|mt| mt.file_extension())
                {
                    file_name.push('.');
                    file_name.push_str(file_extension);
                }

                ctx.save_file_dialog(file_name, "Save blob".to_owned(), blob.to_vec());
            }

            #[cfg(not(target_arch = "wasm32"))]
            if let Some(image) = image {
                let image_stats = ctx
                    .cache
                    .entry(|c: &mut re_viewer_context::ImageStatsCache| c.entry(&image));
                if let Ok(data_range) = re_viewer_context::gpu_bridge::image_data_range_heuristic(
                    &image_stats,
                    &image.format,
                ) {
                    crate::image::copy_image_button_ui(ui, &image, data_range);
                }
            }
        });
    }
}
