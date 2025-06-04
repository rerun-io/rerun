use std::sync::Arc;

use re_log_types::EntityPath;
use re_types::{
    ComponentDescriptor, RowId,
    components::{Blob, MediaType, VideoTimestamp},
};
use re_ui::{
    UiExt as _, icons,
    list_item::{self, PropertyContent},
};
use re_viewer_context::{StoredBlobCacheKey, UiLayout, ViewerContext};

use crate::{
    EntityDataUi,
    image::image_preview_ui,
    video::{show_decoded_frame_info, video_result_ui},
};

impl EntityDataUi for Blob {
    fn entity_data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        entity_path: &EntityPath,
        component_descriptor: &ComponentDescriptor,
        row_id: Option<RowId>,
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
                blob_preview_and_save_ui(
                    ctx,
                    ui,
                    ui_layout,
                    query,
                    entity_path,
                    component_descriptor,
                    row_id,
                    self,
                    media_type.as_ref(),
                    None,
                );

                ui.label(compact_size_string);

                if let Some(media_type) = &media_type {
                    ui.label(media_type.to_string())
                        .on_hover_text("Media type (MIME) based on magic header bytes");
                }
            });
        } else {
            let all_digits_size_string = format!("{} B", re_format::format_uint(self.len()));
            let size_string = if self.len() < 1024 {
                all_digits_size_string
            } else {
                format!("{all_digits_size_string} ({compact_size_string})")
            };

            re_ui::list_item::list_item_scope(ui, "blob_info", |ui| {
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Size").value_text(size_string),
                );

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
                    component_descriptor,
                    row_id,
                    self,
                    media_type.as_ref(),
                    None,
                );
            });
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
    blob_component_descriptor: &ComponentDescriptor,
    blob_row_id: Option<RowId>,
    blob: &re_types::datatypes::Blob,
    media_type: Option<&MediaType>,
    video_timestamp: Option<VideoTimestamp>,
) {
    #[allow(unused_assignments)] // Not used when targeting web.
    let mut image = None;
    let mut video_result_for_frame_preview = None;

    if let Some(blob_row_id) = blob_row_id {
        if !ui_layout.is_single_line() && ui_layout != UiLayout::Tooltip {
            exif_ui(
                ui,
                StoredBlobCacheKey::new(blob_row_id, blob_component_descriptor),
                blob,
            );
        }

        // Try to treat it as an image:
        image = ctx
            .store_context
            .caches
            .entry(|c: &mut re_viewer_context::ImageDecodeCache| {
                c.entry(blob_row_id, blob_component_descriptor, blob, media_type)
            })
            .ok();

        if let Some(image) = &image {
            if !ui_layout.is_single_line() {
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Image format").value_text(image.format.to_string()),
                );
            }

            let colormap = None; // TODO(andreas): Rely on default here for now.
            image_preview_ui(ctx, ui, ui_layout, query, entity_path, image, colormap);
        } else {
            // Try to treat it as a video.
            let video_result =
                ctx.store_context
                    .caches
                    .entry(|c: &mut re_viewer_context::VideoCache| {
                        let debug_name = entity_path.to_string();
                        c.entry(
                            debug_name,
                            blob_row_id,
                            blob_component_descriptor,
                            blob,
                            media_type,
                            ctx.app_options().video_decoder_settings(),
                        )
                    });
            video_result_ui(ui, ui_layout, &video_result);
            video_result_for_frame_preview = Some(video_result);
        }
    }

    if !ui_layout.is_single_line() && ui_layout != UiLayout::Tooltip {
        ui.horizontal(|ui| {
            let text = if cfg!(target_arch = "wasm32") {
                "Download blob…"
            } else {
                "Save blob…"
            };
            if ui
                .add(egui::Button::image_and_text(
                    icons::DOWNLOAD.as_image(),
                    text,
                ))
                .clicked()
            {
                let mut file_name = entity_path
                    .last()
                    .map_or("blob", |name| name.unescaped_str())
                    .to_owned();

                if let Some(file_extension) = media_type.as_ref().and_then(|mt| mt.file_extension())
                {
                    file_name.push('.');
                    file_name.push_str(file_extension);
                }

                ctx.command_sender().save_file_dialog(
                    re_capabilities::MainThreadToken::from_egui_ui(ui),
                    &file_name,
                    "Save blob".to_owned(),
                    blob.to_vec(),
                );
            }

            if let Some(image) = image {
                let image_stats = ctx
                    .store_context
                    .caches
                    .entry(|c: &mut re_viewer_context::ImageStatsCache| c.entry(&image));
                let data_range = re_viewer_context::gpu_bridge::image_data_range_heuristic(
                    &image_stats,
                    &image.format,
                );
                crate::image::copy_image_button_ui(ui, &image, data_range);
            }
        });

        // Show a mini video player for video blobs:
        if let Some(video_result) = &video_result_for_frame_preview {
            if let Ok(video) = video_result.as_ref() {
                ui.separator();

                show_decoded_frame_info(
                    ctx.render_ctx(),
                    ui,
                    ui_layout,
                    video,
                    video_timestamp,
                    blob,
                );
            }
        }
    }
}

/// Show EXIF data about the given blob (image), if possible.
fn exif_ui(ui: &mut egui::Ui, key: StoredBlobCacheKey, blob: &re_types::datatypes::Blob) {
    let exif_result = ui.ctx().memory_mut(|mem| {
        // Cache EXIF parsing to avoid re-parsing every frame.
        // The parsing is really fast, so this is not really needed.
        let cache = mem
            .caches
            .cache::<egui::cache::FramePublisher<StoredBlobCacheKey, Arc<rexif::ExifResult>>>();
        cache.get(&key).cloned().unwrap_or_else(|| {
            re_tracing::profile_scope!("exif-parse");
            let (result, _warnings) = rexif::parse_buffer_quiet(blob);
            let result = Arc::new(result);
            cache.set(key, result.clone());
            result
        })
    });

    if let Ok(exif) = &*exif_result {
        ui.list_item_collapsible_noninteractive_label("EXIF", false, |ui| {
            list_item::list_item_scope(ui, "exif", |ui| {
                for entry in &exif.entries {
                    let tag_string = if entry.tag == rexif::ExifTag::UnknownToMe {
                        "<Unknown tag>".to_owned()
                    } else {
                        entry.tag.to_string()
                    };
                    ui.list_item_flat_noninteractive(
                        list_item::PropertyContent::new(tag_string)
                            .value_text(entry.value_more_readable.to_string()),
                    );
                }
            });
        });
    }
}
