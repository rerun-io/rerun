use re_types::components::{Blob, MediaType};
use re_ui::{list_item::PropertyContent, UiExt};
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

#[allow(clippy::too_many_arguments)]
pub fn blob_preview_and_save_ui(
    ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
    blob_row_id: Option<re_chunk_store::RowId>,
    blob: &re_types::datatypes::Blob,
    media_type: Option<&MediaType>,
) {
    // Try to treat it as an image:
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
    // Try to treat it as a video if treating it as image didn't work:
    else if let Some(render_ctx) = ctx.render_ctx {
        let video_result = blob_row_id.map(|row_id| {
            ctx.cache.entry(|c: &mut re_viewer_context::VideoCache| {
                c.entry(
                    row_id,
                    blob,
                    media_type.as_ref().map(|mt| mt.as_str()),
                    render_ctx,
                )
            })
        });
        if let Some(video_result) = &video_result {
            show_video_blob_info(ui, ui_layout, video_result);
        }
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

fn show_video_blob_info(
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    video_result: &Result<re_renderer::video::Video, re_renderer::video::VideoError>,
) {
    match video_result {
        Ok(video) => {
            if ui_layout.is_single_line() {
                return;
            }

            re_ui::list_item::list_item_scope(ui, "video_blob_info", |ui| {
                ui.list_item_flat_noninteractive(re_ui::list_item::LabelContent::new(
                    "Video properties",
                ));
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Dimensions").value_text(format!(
                        "{}x{}",
                        video.width(),
                        video.height()
                    )),
                );
                ui.list_item_flat_noninteractive(PropertyContent::new("Duration").value_text(
                    format!(
                        "{}",
                        re_log_types::Duration::from_millis(video.duration().as_ms_f64() as _)
                    ),
                ));
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Codec").value_text(video.codec()),
                );

                // TODO(andreas): A mini video player at this point would be awesome!
            });
        }
        Err(err) => {
            if ui_layout.is_single_line() {
                ui.error_label(&format!("Failed to load video: {err}"));
            } else {
                ui.error_label_long(&format!("Failed to load video: {err}"));
            }
        }
    }
}
