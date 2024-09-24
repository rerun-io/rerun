use re_renderer::{external::re_video::VideoLoadError, video::FrameDecodingResult};
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
    else if let Some(blob_row_id) = blob_row_id {
        let video_result = ctx.cache.entry(|c: &mut re_viewer_context::VideoCache| {
            c.entry(blob_row_id, blob, media_type.as_ref().map(|mt| mt.as_str()))
        });
        show_video_blob_info(ctx.render_ctx, ui, ui_layout, &video_result);
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
    render_ctx: Option<&re_renderer::RenderContext>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    video_result: &Result<re_renderer::video::Video, VideoLoadError>,
) {
    match video_result {
        Ok(video) => {
            if ui_layout.is_single_line() {
                return;
            }

            let data = video.data();

            re_ui::list_item::list_item_scope(ui, "video_blob_info", |ui| {
                ui.list_item_flat_noninteractive(re_ui::list_item::LabelContent::new(
                    "Video properties",
                ));
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Dimensions").value_text(format!(
                        "{}x{}",
                        data.width(),
                        data.height()
                    )),
                );
                ui.list_item_flat_noninteractive(PropertyContent::new("Duration").value_text(
                    format!(
                        "{}",
                        re_log_types::Duration::from_millis(data.duration_ms() as i64)
                    ),
                ));
                // Some people may think that num_frames / duration = fps, but that's not true, videos may have variable frame rate.
                // At the same time, we don't want to overload users with video codec/container specific stuff that they have to understand,
                // and for all intents and purposes one sample = one frame.
                // So the compromise is that we truthfully show the number of *samples* here and don't talk about frames.
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Sample count")
                        .value_text(format!("{}", data.num_samples())),
                );
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Codec").value_text(data.codec()),
                );

                ui.list_item_collapsible_noninteractive_label("MP4 tracks", true, |ui| {
                    for (track_id, track_kind) in &data.mp4_tracks {
                        let track_kind_string = match track_kind {
                            Some(re_video::TrackKind::Audio) => "audio",
                            Some(re_video::TrackKind::Subtitle) => "subtitle",
                            Some(re_video::TrackKind::Video) => "video",
                            None => "unknown",
                        };
                        ui.list_item_flat_noninteractive(
                            PropertyContent::new(format!("Track {track_id}"))
                                .value_text(track_kind_string),
                        );
                    }
                });

                if let Some(render_ctx) = render_ctx {
                    // Show a mini-player for the video:

                    // TODO(emilk): Some time controls would be nice,
                    // but the point here is not to have a nice viewer,
                    // but to show the user what they have selected
                    let timestamp_in_seconds = ui.input(|i| i.time) % video.data().duration_sec();
                    ui.ctx().request_repaint(); // TODO(emilk): schedule a repaint just in time for the next frame of video

                    let decode_stream_id = re_renderer::video::VideoDecodingStreamId(
                        egui::Id::new("video_miniplayer").value(),
                    );

                    if let Some(texture) =
                        match video.frame_at(render_ctx, decode_stream_id, timestamp_in_seconds) {
                            FrameDecodingResult::Ready(texture)
                            | FrameDecodingResult::Pending(texture) => Some(texture),

                            FrameDecodingResult::Error(err) => {
                                ui.error_label(&err.to_string());
                                None
                            }
                        }
                    {
                        crate::image::texture_preview_ui(
                            render_ctx,
                            ui,
                            ui_layout,
                            "video_preview",
                            re_renderer::renderer::ColormappedTexture::from_unorm_rgba(texture),
                        );
                    }
                }
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
