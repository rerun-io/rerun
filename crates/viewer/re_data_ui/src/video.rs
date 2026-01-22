use std::sync::Arc;

use egui::NumExt as _;
use egui_extras::Column;
use re_format::time::format_relative_timestamp_secs;
use re_renderer::external::re_video::VideoLoadError;
use re_renderer::resource_managers::SourceImageDataFormat;
use re_renderer::video::VideoFrameTexture;
use re_sdk_types::components::{MediaType, VideoTimestamp};
use re_sdk_types::{Archetype as _, archetypes};
use re_types_core::{ComponentDescriptor, RowId};
use re_ui::UiExt as _;
use re_ui::list_item::{self, PropertyContent};
use re_video::{FrameInfo, VideoDataDescription};
use re_viewer_context::{
    SharablePlayableVideoStream, UiLayout, VideoStreamCache, VideoStreamProcessingError,
    ViewerContext, video_stream_time_from_query,
};

use crate::image::texture_preview_size;

pub fn video_asset_result_ui(
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    video_result: &Result<re_renderer::video::Video, VideoLoadError>,
) {
    re_tracing::profile_function!();

    match video_result {
        Ok(video) => {
            if ui_layout == UiLayout::SelectionPanel {
                let default_open = true;
                // Extra scope needed to ensure right spacing.
                ui.list_item_scope("video_asset", |ui| {
                    ui.list_item_collapsible_noninteractive_label(
                        "Video Asset",
                        default_open,
                        |ui| {
                            video_data_ui(ui, ui_layout, video.data_descr());
                        },
                    );
                });
            }
        }
        Err(err) => {
            let error_message = format!("Failed to play: {err}");
            if ui_layout.is_single_line() {
                ui.error_with_details_on_hover(error_message);
            } else {
                ui.error_label(error_message);
            }
        }
    }
}

pub fn video_stream_result_ui(
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    video_result: &Result<SharablePlayableVideoStream, VideoStreamProcessingError>,
) {
    re_tracing::profile_function!();

    match video_result {
        Ok(video) => {
            if ui_layout == UiLayout::SelectionPanel {
                let default_open = true;
                // Extra scope needed to ensure right spacing.
                ui.list_item_scope("video_stream", |ui| {
                    ui.list_item_collapsible_noninteractive_label(
                        "Video Stream",
                        default_open,
                        |ui| {
                            video_data_ui(ui, ui_layout, video.read().video_descr());
                        },
                    );
                });
            }
        }
        Err(err) => {
            let error_message = format!("Failed to process video stream: {err}");
            if ui_layout.is_single_line() {
                ui.error_with_details_on_hover(error_message);
            } else {
                ui.error_label(error_message);
            }
        }
    }
}

fn video_data_ui(ui: &mut egui::Ui, ui_layout: UiLayout, video_descr: &VideoDataDescription) {
    re_tracing::profile_function!();

    if let Some(encoding_details) = &video_descr.encoding_details {
        let [w, h] = &encoding_details.coded_dimensions;
        ui.list_item_flat_noninteractive(
            PropertyContent::new("Dimensions").value_text(format!("{w}x{h}")),
        );

        if let Some(bit_depth) = encoding_details.bit_depth {
            ui.list_item_flat_noninteractive(PropertyContent::new("Bit depth").value_fn(
                |ui, _| {
                    ui.label(bit_depth.to_string());
                    if 8 < bit_depth {
                        // TODO(#7594): HDR videos
                        ui.warning_label("HDR").on_hover_ui(|ui| {
                            ui.label("High-dynamic-range videos not yet supported by Rerun");
                            ui.hyperlink("https://github.com/rerun-io/rerun/issues/7594");
                        });
                    }
                    if encoding_details.chroma_subsampling
                        == Some(re_video::ChromaSubsamplingModes::Monochrome)
                    {
                        ui.label("(monochrome)");
                    }
                },
            ));
        }
        if let Some(chroma_subsampling) = encoding_details.chroma_subsampling {
            // Don't show subsampling mode for monochrome. Usually we know the bit depth and already shown it there.
            if chroma_subsampling != re_video::ChromaSubsamplingModes::Monochrome {
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Subsampling").value_text(chroma_subsampling.to_string()),
                );
            }
        }
    }

    if let Some(duration) = video_descr.duration() {
        ui.list_item_flat_noninteractive(
            PropertyContent::new("Duration")
                .value_text(format!("{}", re_log_types::Duration::from(duration))),
        );
    }

    ui.list_item_flat_noninteractive(
        PropertyContent::new("Frame count").value_uint(video_descr.num_samples()),
    );

    if let Some(fps) = video_descr.average_fps() {
        ui.list_item_flat_noninteractive(
            PropertyContent::new("Average FPS").value_text(format!("{fps:.2}")),
        )
        .on_hover_text("Average frames per second (FPS) of the video");
    }

    ui.list_item_flat_noninteractive(
        PropertyContent::new("Codec").value_text(video_descr.human_readable_codec_string()),
    );

    if ui_layout != UiLayout::Tooltip && !video_descr.mp4_tracks.is_empty() {
        ui.list_item_collapsible_noninteractive_label("MP4 tracks", false, |ui| {
            for (track_id, track_kind) in &video_descr.mp4_tracks {
                let track_kind_string = match track_kind {
                    Some(re_video::TrackKind::Audio) => "audio",
                    Some(re_video::TrackKind::Subtitle) => "subtitle",
                    Some(re_video::TrackKind::Video) => "video",
                    None => "unknown",
                };
                ui.list_item_flat_noninteractive(
                    PropertyContent::new(format!("Track {track_id}")).value_text(track_kind_string),
                );
            }
        });
    }

    ui.list_item_collapsible_noninteractive_label("More video statistics", false, |ui| {
            ui.list_item_flat_noninteractive(
                PropertyContent::new("Number of keyframes")
                    .value_uint(video_descr.keyframe_indices.len()),
            )
            .on_hover_text("The total number of keyframes in the video.");

            let re_video::SamplesStatistics {dts_always_equal_pts, has_sample_highest_pts_so_far: _} = &video_descr.samples_statistics;

            ui.list_item_flat_noninteractive(
                PropertyContent::new("All PTS equal DTS").value_bool(*dts_always_equal_pts)
            ).on_hover_text("Whether all decode timestamps are equal to presentation timestamps. If true, the video typically has no B-frames.");
        });

    ui.list_item_collapsible_noninteractive_label("Video samples", false, |ui| {
        egui::Resize::default()
            .with_stroke(true)
            .resizable([false, true])
            .max_height(611.0) // Odd value so the user can see half-hidden rows
            .show(ui, |ui| {
                samples_table_ui(ui, video_descr);
            });
    });
}

fn samples_table_ui(ui: &mut egui::Ui, video_descr: &VideoDataDescription) {
    re_tracing::profile_function!();
    let tokens = ui.tokens();
    let table_style = re_ui::TableStyle::Dense;

    egui_extras::TableBuilder::new(ui)
        .auto_shrink([false, true])
        .vscroll(true)
        .max_scroll_height(611.0) // Odd value so the user can see half-hidden rows
        .columns(Column::auto(), 8)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .header(tokens.deprecated_table_header_height(), |mut header| {
            re_ui::DesignTokens::setup_table_header(&mut header);
            header.col(|ui| {
                ui.strong("Sample");
            });
            header.col(|ui| {
                ui.strong("Frame");
            });
            header.col(|ui| {
                ui.strong("GOP");
            });
            header.col(|ui| {
                ui.strong("Sync");
            });
            header.col(|ui| {
                ui.strong("DTS").on_hover_text("Decode timestamp");
            });
            header.col(|ui| {
                ui.strong("PTS").on_hover_text("Presentation timestamp");
            });
            header.col(|ui| {
                ui.strong("Duration");
            });
            header.col(|ui| {
                ui.strong("Size");
            });
        })
        .body(|mut body| {
            tokens.setup_table_body(&mut body, table_style);

            body.rows(
                tokens.table_row_height(table_style),
                video_descr.samples.num_elements(),
                |mut row| {
                    let sample_idx = row.index() + video_descr.samples.min_index();
                    let Some(sample) = video_descr.samples[sample_idx].sample() else {
                        return;
                    };
                    let re_video::SampleMetadata {
                        is_sync,
                        frame_nr,
                        decode_timestamp,
                        presentation_timestamp,
                        duration,
                        source_id: _,
                        byte_span,
                    } = *sample;

                    row.col(|ui| {
                        ui.monospace(re_format::format_uint(sample_idx));
                    });
                    row.col(|ui| {
                        ui.monospace(re_format::format_uint(frame_nr));
                    });
                    row.col(|ui| {
                        if let Some(keyframe_index) =
                            video_descr.presentation_time_keyframe_index(presentation_timestamp)
                        {
                            ui.monospace(re_format::format_uint(keyframe_index));
                        }
                    });
                    row.col(|ui| {
                        if is_sync {
                            ui.label("sync");
                        }
                    });
                    row.col(|ui| {
                        timestamp_ui(ui, video_descr.timescale, decode_timestamp);
                    });
                    row.col(|ui| {
                        timestamp_ui(ui, video_descr.timescale, presentation_timestamp);
                    });

                    row.col(|ui| {
                        if let (Some(duration), Some(timescale)) = (duration, video_descr.timescale)
                        {
                            ui.monospace(
                                re_log_types::Duration::from(duration.duration(timescale))
                                    .to_string(),
                            );
                        } else {
                            ui.monospace("unknown");
                        }
                    });
                    row.col(|ui| {
                        ui.monospace(re_format::format_bytes(byte_span.len as _));
                    });
                },
            );
        });
}

fn timestamp_ui(
    ui: &mut egui::Ui,
    timescale: Option<re_video::Timescale>,
    timestamp: re_video::Time,
) {
    let response = ui.monospace(re_format::format_int(timestamp.0));
    if let Some(timescale) = timescale {
        response.on_hover_ui(|ui| {
            ui.monospace(format_relative_timestamp_secs(
                timestamp.into_secs(timescale),
            ));
        });
    }
}

fn decoded_frame_ui<'a>(
    ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    video: &re_renderer::video::Video,
    video_time: re_video::Time,
    get_video_buffer: &dyn Fn(re_log_types::external::re_tuid::Tuid) -> &'a [u8],
) {
    let player_stream_id =
        re_renderer::video::VideoPlayerStreamId(ui.id().with("video_player").value());

    match video.frame_at(
        ctx.render_ctx(),
        player_stream_id,
        video_time,
        get_video_buffer,
    ) {
        Ok(VideoFrameTexture {
            texture,
            decoder_delay_state,
            show_spinner,
            frame_info,
            source_pixel_format,
        }) => {
            if let Some(frame_info) = frame_info
                && ui_layout == UiLayout::SelectionPanel
            {
                re_ui::list_item::list_item_scope(ui, "decoded_frame_ui", |ui| {
                    let id = ui.id().with("decoded_frame_collapsible");
                    let default_open = false;
                    let label = if let Some(frame_nr) = frame_info.frame_nr {
                        format!("Decoded frame #{}", re_format::format_uint(frame_nr))
                    } else {
                        "Current decoded frame".to_owned()
                    };
                    ui.list_item()
                        .interactive(false)
                        .show_hierarchical_with_children(
                            ui,
                            id,
                            default_open,
                            list_item::LabelContent::new(label),
                            |ui| {
                                list_item::list_item_scope(ui, id, |ui| {
                                    frame_info_ui(ui, &frame_info, video.data_descr());
                                    source_image_data_format_ui(ui, &source_pixel_format);
                                });
                            },
                        )
                });
            }

            let preview_size = if let Some(texture) = &texture {
                let [w, h] = texture.width_height();
                texture_preview_size(ui, ui_layout, [w, h])
            } else if let Some([w, h]) = video.dimensions() {
                texture_preview_size(ui, ui_layout, [w as _, h as _])
            } else {
                egui::Vec2::splat(ui.available_width().at_most(64.0))
            };

            let response = if let Some(texture) = texture {
                crate::image::texture_preview_ui(
                    ctx.render_ctx(),
                    ui,
                    ui_layout,
                    "video_preview",
                    re_renderer::renderer::ColormappedTexture::from_unorm_rgba(texture),
                    preview_size,
                )
            } else {
                ui.allocate_response(preview_size, egui::Sense::hover())
            };

            if decoder_delay_state.should_request_more_frames() {
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
        }

        Err(err) => {
            ui.error_label(err.to_string());

            #[cfg(not(target_arch = "wasm32"))]
            if let re_renderer::video::VideoPlayerError::Decoding(re_video::DecodeError::Ffmpeg(
                err,
            )) = &err
            {
                match err.as_ref() {
                    re_video::FFmpegError::UnsupportedFFmpegVersion { .. }
                    | re_video::FFmpegError::FailedToDetermineFFmpegVersion(_)
                    | re_video::FFmpegError::FFmpegNotInstalled => {
                        if let Some(download_url) = re_video::ffmpeg_download_url() {
                            ui.markdown_ui(&format!("You can download a build of `FFmpeg` [here]({download_url}). For Rerun to be able to use it, its binaries need to be reachable from `PATH`."));
                        }
                    }

                    _ => {}
                }
            }
        }
    }
}

fn frame_info_ui(
    ui: &mut egui::Ui,
    frame_info: &FrameInfo,
    video_descr: &re_video::VideoDataDescription,
) {
    let FrameInfo {
        is_sync,
        sample_idx,
        frame_nr,
        presentation_timestamp,
        duration: _,
        latest_decode_timestamp,
    } = *frame_info;

    if let Some(is_sync) = is_sync {
        ui.list_item_flat_noninteractive(PropertyContent::new("Sync").value_bool(is_sync))
            .on_hover_text(
                "The start of a new GOP (Group of Frames)?\n\
                If true, it likely means the frame is a keyframe.",
            );
    }

    let presentation_time_range = frame_info.presentation_time_range();
    if let Some(timescale) = video_descr.timescale {
        ui.list_item_flat_noninteractive(PropertyContent::new("Time range").value_text(format!(
            "{} - {}",
            format_relative_timestamp_secs(presentation_time_range.start.into_secs(timescale)),
            format_relative_timestamp_secs(presentation_time_range.end.into_secs(timescale)),
        )))
    } else {
        ui.list_item_flat_noninteractive(PropertyContent::new("Time range").value_text(format!(
            "{} - {}",
            presentation_time_range.start.0, presentation_time_range.end.0,
        )))
    }
    .on_hover_text("Time range in which this frame is valid.");

    fn value_fn_for_time(
        time: re_video::Time,
        video_descr: &re_video::VideoDataDescription,
    ) -> impl FnOnce(&mut egui::Ui, list_item::ListVisuals) + '_ {
        move |ui, _| {
            timestamp_ui(ui, video_descr.timescale, time);
        }
    }

    if let Some(sample_idx) = sample_idx {
        ui.list_item_flat_noninteractive(PropertyContent::new("Sample").value_fn(move |ui, _| {
            ui.monospace(re_format::format_uint(sample_idx));
        }))
        .on_hover_text(
            "The sample number of this frame in the video. In MP4, one sample is one frame, but not necessareily in the same order!",
        );
    }

    if let Some(frame_nr) = frame_nr {
        ui.list_item_flat_noninteractive(PropertyContent::new("Frame").value_fn(move |ui, _| {
            ui.monospace(re_format::format_uint(frame_nr));
        }))
        .on_hover_text("The frame number, as ordered by presentation time");
    }

    if let Some(dts) = latest_decode_timestamp {
        ui.list_item_flat_noninteractive(
            PropertyContent::new("DTS").value_fn(value_fn_for_time(dts, video_descr)),
        )
        .on_hover_text("Raw decode timestamp prior to applying the timescale.\n\
                        If a frame is made up of multiple chunks, this is the last decode timestamp that was needed to decode the frame.");
    }

    ui.list_item_flat_noninteractive(
        PropertyContent::new("PTS").value_fn(value_fn_for_time(presentation_timestamp, video_descr)),
    )
    .on_hover_text("Raw presentation timestamp prior to applying the timescale.\n\
                    This specifies the time at which the frame should be shown relative to the start of a video stream.");

    // Judging the following to be a bit too obscure to be of relevance outside of debugging Rerun itself.
    #[cfg(debug_assertions)]
    {
        if let Some(has_sample_highest_pts_so_far) = video_descr
            .samples_statistics
            .has_sample_highest_pts_so_far
            .as_ref()
            && let Some(sample_idx) =
                video_descr.latest_sample_index_at_presentation_timestamp(presentation_timestamp)
        {
            ui.list_item_flat_noninteractive(
                PropertyContent::new("Highest PTS so far").value_bool(has_sample_highest_pts_so_far[sample_idx])
            ).on_hover_text("Whether the presentation timestamp (PTS) at the this frame is the highest encountered so far. If false there are lower PTS values prior in the list.");
        }
    }

    // Information about the current group of pictures this frame is part of.
    // Lookup via decode timestamp is faster, but it may not always be available.
    if let Some(keyframe_idx) = video_descr.presentation_time_keyframe_index(presentation_timestamp)
    {
        ui.list_item_flat_noninteractive(
            PropertyContent::new("keyframe index").value_text(keyframe_idx.to_string()),
        )
        .on_hover_text("The index of the keyframe that this sample belongs to.");

        if let Some(sample_range) = video_descr.gop_sample_range_for_keyframe(keyframe_idx) {
            let first_sample = video_descr.samples.get(sample_range.start);
            let last_sample = video_descr.samples.get(sample_range.end.saturating_sub(1));

            if let Some((first_sample, last_sample)) = first_sample
                .and_then(|s| s.sample())
                .zip(last_sample.and_then(|s| s.sample()))
            {
                ui.list_item_flat_noninteractive(PropertyContent::new("GOP DTS range").value_text(
                    format!("{} - {}", re_format::format_int(first_sample.decode_timestamp.0), re_format::format_int(last_sample.decode_timestamp.0))
                ))
                .on_hover_text(
                    "The range of decode timestamps in the currently active group of picture (GOP).",
                );
            } else {
                ui.error_label("GOP has invalid sample range"); // Should never happen.
            }
        } else {
            ui.error_label("Invalid GOP index"); // Should never happen.
        }
    }
}

fn source_image_data_format_ui(ui: &mut egui::Ui, format: &SourceImageDataFormat) {
    let label = "Decoder output format";

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
    }
}

pub enum VideoUi {
    Stream(Result<SharablePlayableVideoStream, VideoStreamProcessingError>),
    Asset(
        Arc<Result<re_renderer::video::Video, VideoLoadError>>,
        Option<VideoTimestamp>,
        re_sdk_types::datatypes::Blob,
    ),
}

impl VideoUi {
    pub fn from_blob(
        ctx: &ViewerContext<'_>,
        entity_path: &re_log_types::EntityPath,
        blob_row_id: RowId,
        blob_component_descriptor: &ComponentDescriptor,
        blob: &re_sdk_types::datatypes::Blob,
        media_type: Option<&MediaType>,
        video_timestamp: Option<VideoTimestamp>,
    ) -> Option<Self> {
        let result =
            ctx.store_context
                .caches
                .entry(|c: &mut re_viewer_context::VideoAssetCache| {
                    let debug_name = entity_path.to_string();
                    c.entry(
                        debug_name,
                        blob_row_id,
                        blob_component_descriptor.component,
                        blob,
                        media_type,
                        ctx.app_options().video_decoder_settings(),
                    )
                });

        let certain_this_is_a_video =
            blob_component_descriptor.archetype == Some(archetypes::AssetVideo::name());

        if let Err(err) = &*result
            && !certain_this_is_a_video
            && matches!(
                err,
                VideoLoadError::MimeTypeIsNotAVideo { .. } | VideoLoadError::UnrecognizedMimeType
            )
        {
            // Don't show an error if we weren't certain that this was a video and it turned out not to be one.
            return None;
        }

        Some(Self::Asset(result, video_timestamp, blob.clone()))
    }

    pub fn from_components(
        ctx: &ViewerContext<'_>,
        query: &re_chunk_store::LatestAtQuery,
        entity_path: &re_log_types::EntityPath,
        descr: &ComponentDescriptor,
    ) -> Option<Self> {
        if descr != &archetypes::VideoStream::descriptor_sample() {
            return None;
        }

        let video_stream_result = ctx.store_context.caches.entry(|c: &mut VideoStreamCache| {
            c.entry(
                ctx.recording(),
                entity_path,
                query.timeline(),
                ctx.app_options().video_decoder_settings(),
            )
        });

        Some(Self::Stream(video_stream_result))
    }

    pub fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
    ) {
        match self {
            Self::Stream(video_stream_result) => {
                video_stream_result_ui(ui, ui_layout, video_stream_result);

                let storage_engine = ctx.store_context.recording.storage_engine();
                let get_chunk_array = |id| {
                    storage_engine.store().insert_missing_chunk_id(id);
                    let chunk = storage_engine.store().physical_chunk(&id)?;

                    let sample_component = archetypes::VideoStream::descriptor_sample().component;

                    let (_, buffer) = re_arrow_util::blob_arrays_offsets_and_buffer(
                        chunk.raw_component_array(sample_component)?,
                    )?;

                    Some(buffer)
                };

                if let Ok(video) = video_stream_result {
                    let video = video.read();
                    let time = video_stream_time_from_query(query);
                    decoded_frame_ui(ctx, ui, ui_layout, &video.video_renderer, time, &|id| {
                        let buffer = get_chunk_array(re_sdk_types::ChunkId::from_tuid(id));

                        buffer.map(|b| b.as_slice()).unwrap_or(&[])
                    });
                }
            }
            Self::Asset(video_result, timestamp, blob) => {
                video_asset_result_ui(ui, ui_layout, video_result);

                // Show a mini video player for video blobs:
                if let Ok(video) = video_result.as_ref() {
                    let video_timestamp = timestamp.unwrap_or_else(|| {
                        // TODO(emilk): Some time controls would be nice,
                        // but the point here is not to have a nice viewer,
                        // but to show the user what they have selected
                        ui.ctx().request_repaint(); // TODO(emilk): schedule a repaint just in time for the next frame of video
                        let time = ui.input(|i| i.time);

                        if let Some(duration) = video.data_descr().duration() {
                            VideoTimestamp::from_secs(time % duration.as_secs_f64())
                        } else {
                            // Invalid video or unknown timescale.
                            VideoTimestamp::from_nanos(0)
                        }
                    });
                    let video_time = re_viewer_context::video_timestamp_component_to_video_time(
                        ctx,
                        video_timestamp,
                        video.data_descr().timescale,
                    );

                    decoded_frame_ui(ctx, ui, ui_layout, video, video_time, &|_| blob);
                }
            }
        }
    }
}
