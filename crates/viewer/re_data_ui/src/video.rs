use re_renderer::{
    external::re_video::VideoLoadError, resource_managers::SourceImageDataFormat,
    video::VideoFrameTexture,
};
use re_types::components::VideoTimestamp;
use re_ui::{list_item::PropertyContent, UiExt};
use re_video::decode::FrameInfo;
use re_viewer_context::UiLayout;

pub fn show_video_blob_info(
    render_ctx: Option<&re_renderer::RenderContext>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    video_result: &Result<re_renderer::video::Video, VideoLoadError>,
    video_timestamp: Option<VideoTimestamp>,
    blob: &re_types::datatypes::Blob,
) {
    #[allow(clippy::match_same_arms)]
    match video_result {
        Ok(video) => {
            if ui_layout.is_single_line() {
                return;
            }

            let data = video.data();

            re_ui::list_item::list_item_scope(ui, "video_blob_info", |ui| {
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Dimensions").value_text(format!(
                        "{}x{}",
                        data.width(),
                        data.height()
                    )),
                );
                if let Some(bit_depth) = data.config.stsd.contents.bit_depth() {
                    let mut bit_depth = bit_depth.to_string();
                    if data.is_monochrome() == Some(true) {
                        bit_depth = format!("{bit_depth} (monochrome)");
                    }

                    ui.list_item_flat_noninteractive(
                        PropertyContent::new("Bit depth").value_text(bit_depth),
                    );
                }
                if let Some(subsampling_mode) = data.subsampling_mode() {
                    // Don't show subsampling mode for monochrome, doesn't make sense usually.
                    if data.is_monochrome() != Some(true) {
                        ui.list_item_flat_noninteractive(
                            PropertyContent::new("Subsampling")
                                .value_text(subsampling_mode.to_string()),
                        );
                    }
                }
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Duration")
                        .value_text(format!("{}", re_log_types::Duration::from(data.duration()))),
                );
                // Some people may think that num_frames / duration = fps, but that's not true, videos may have variable frame rate.
                // At the same time, we don't want to overload users with video codec/container specific stuff that they have to understand,
                // and for all intents and purposes one sample = one frame.
                // So the compromise is that we truthfully show the number of *samples* here and don't talk about frames.
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Sample count")
                        .value_text(re_format::format_uint(data.num_samples())),
                );
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Codec").value_text(data.human_readable_codec_string()),
                );

                if ui_layout != UiLayout::Tooltip {
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
                }

                if let Some(render_ctx) = render_ctx {
                    // Show a mini-player for the video:

                    let timestamp_in_seconds = if let Some(video_timestamp) = video_timestamp {
                        video_timestamp.as_seconds()
                    } else {
                        // TODO(emilk): Some time controls would be nice,
                        // but the point here is not to have a nice viewer,
                        // but to show the user what they have selected
                        ui.ctx().request_repaint(); // TODO(emilk): schedule a repaint just in time for the next frame of video
                        ui.input(|i| i.time) % video.data().duration().as_secs_f64()
                    };

                    let player_stream_id = re_renderer::video::VideoPlayerStreamId(
                        ui.id().with("video_player").value(),
                    );

                    match video.frame_at(
                        render_ctx,
                        player_stream_id,
                        timestamp_in_seconds,
                        blob.as_slice(),
                    ) {
                        Ok(VideoFrameTexture {
                            texture,
                            is_pending,
                            show_spinner,
                            frame_info,
                            source_pixel_format,
                        }) => {
                            let response = crate::image::texture_preview_ui(
                                render_ctx,
                                ui,
                                ui_layout,
                                "video_preview",
                                re_renderer::renderer::ColormappedTexture::from_unorm_rgba(texture),
                            );

                            if is_pending {
                                ui.ctx().request_repaint(); // Keep polling for an up-to-date texture
                            }

                            if show_spinner {
                                // Shrink slightly:
                                let smaller_rect = egui::Rect::from_center_size(
                                    response.rect.center(),
                                    0.75 * response.rect.size(),
                                );
                                egui::Spinner::new().paint_at(ui, smaller_rect);
                            }

                            decoded_frame_ui(
                                ui,
                                &frame_info,
                                video.data().timescale,
                                &source_pixel_format,
                            );
                        }

                        Err(err) => {
                            ui.error_label_long(&err.to_string());
                        }
                    }
                }
            });
        }
        Err(VideoLoadError::MimeTypeIsNotAVideo { .. }) => {
            // Don't show an error if this wasn't a video in the first place.
            // Unfortunately we can't easily detect here if the Blob was _supposed_ to be a video, for that we'd need tagged components!
            // (User may have confidently logged a non-video format as Video, we should tell them that!)
        }
        Err(VideoLoadError::UnrecognizedMimeType) => {
            // If we couldn't detect the media type,
            // we can't show an error for unrecognized formats since maybe this wasn't a video to begin with.
            // See also `MediaTypeIsNotAVideo` case above.
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

fn decoded_frame_ui(
    ui: &mut egui::Ui,
    frame_info: &FrameInfo,
    timescale: re_video::Timescale,
    source_image_format: &SourceImageDataFormat,
) {
    re_ui::list_item::list_item_scope(ui, "decoded_frame_ui", |ui| {
        let default_open = false;
        ui.list_item_collapsible_noninteractive_label("Decoded frame info", default_open, |ui| {
            frame_info_ui(ui, frame_info, timescale);
            source_image_data_format_ui(ui, source_image_format);
        });
    });
}

fn frame_info_ui(ui: &mut egui::Ui, frame_info: &FrameInfo, timescale: re_video::Timescale) {
    let time_range = frame_info.time_range();
    ui.list_item_flat_noninteractive(PropertyContent::new("Time range").value_text(format!(
        "{} - {}",
        re_format::format_timestamp_seconds(time_range.start.into_secs(timescale),),
        re_format::format_timestamp_seconds(time_range.end.into_secs(timescale),),
    )))
    .on_hover_text("Time range in which this frame is valid.");

    ui.list_item_flat_noninteractive(
        PropertyContent::new("PTS").value_text(format!("{}", frame_info.presentation_timestamp.0)),
    )
    .on_hover_text("Raw presentation timestamp prior to applying the timescale.\n\
                    This specifies the time at which the frame should be shown relative to the start of a video stream.");
}

fn source_image_data_format_ui(ui: &mut egui::Ui, format: &SourceImageDataFormat) {
    let label = "Output format";

    match format {
        SourceImageDataFormat::WgpuCompatible(format) => {
            ui.list_item_flat_noninteractive(PropertyContent::new(label).value_text(format!("{format:?}")))
                // This is true for YUV outputs as well, but for RGB/RGBA there was almost certainly some postprocessing involved,
                // whereas it would very surprising for YUV.
                .on_hover_text("Pixel format as returned from the decoder.\n\
                                Decoders may do arbitrary post processing, so this is not necessarily the format that is actually encoded in the video data!"
            );
        }

        SourceImageDataFormat::Yuv {
            layout,
            range,
            coefficients,
        } => {
            let default_open = true;
            ui.list_item_collapsible_noninteractive_label(label, default_open, |ui| {
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Data layout").value_text(layout.to_string()),
                )
                .on_hover_text("Subsampling ratio & layout of the pixel data.");
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Color range").value_text(range.to_string()),
                )
                .on_hover_text("Valid range of the pixel data values.");
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Yuv Coefficients").value_text(coefficients.to_string()),
                )
                .on_hover_text("Matrix coefficients used to convert the pixel data to RGB.");
            });
        }
    };
}
