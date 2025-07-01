use egui::NumExt as _;
use egui_extras::Column;
use re_renderer::{
    external::re_video::VideoLoadError, resource_managers::SourceImageDataFormat,
    video::VideoFrameTexture,
};
use re_ui::{
    UiExt as _,
    list_item::{self, PropertyContent},
};
use re_video::{FrameInfo, StableIndexDeque, VideoDataDescription};
use re_viewer_context::{SharablePlayableVideoStream, UiLayout, VideoStreamProcessingError};

pub fn video_asset_result_ui(
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    video_result: &Result<re_renderer::video::Video, VideoLoadError>,
) {
    re_tracing::profile_function!();

    #[allow(clippy::match_same_arms)]
    match video_result {
        Ok(video) => {
            if !ui_layout.is_single_line() {
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
            let error_message = format!("Failed to load video: {err}");
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

    #[allow(clippy::match_same_arms)]
    match video_result {
        Ok(video) => {
            if !ui_layout.is_single_line() {
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

    // Some people may think that num_frames / duration = fps, but that's not true, videos may have variable frame rate.
    // Video containers and codecs like talking about samples or chunks rather than frames, but for how we define a chunk today,
    // a frame is always a single chunk of data is always a single sample, see [`re_video::decode::Chunk`].
    // So for all practical purposes the sample count _is_ the number of frames, at least how we use it today.
    ui.list_item_flat_noninteractive(
        PropertyContent::new("Frame count")
            .value_text(re_format::format_uint(video_descr.num_samples())),
    );
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
                PropertyContent::new("Number of GOPs")
                    .value_text(video_descr.gops.num_elements().to_string()),
            )
            .on_hover_text("The total number of Group of Pictures (GOPs) in the video.");

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
            tokens.setup_table_body(&mut body);

            body.rows(
                tokens.deprecated_table_line_height(),
                video_descr.samples.num_elements(),
                |mut row| {
                    let sample_idx = row.index() + video_descr.samples.min_index();
                    let sample = &video_descr.samples[sample_idx];
                    let re_video::SampleMetadata {
                        is_sync,
                        frame_nr,
                        decode_timestamp,
                        presentation_timestamp,
                        duration,
                        buffer_index: _,
                        byte_span,
                    } = *sample;

                    row.col(|ui| {
                        ui.monospace(re_format::format_uint(sample_idx));
                    });
                    row.col(|ui| {
                        ui.monospace(re_format::format_uint(frame_nr));
                    });
                    row.col(|ui| {
                        if let Some(gop_index) = video_descr
                            .gop_index_containing_presentation_timestamp(presentation_timestamp)
                        {
                            ui.monospace(re_format::format_uint(gop_index));
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
            ui.monospace(re_format::format_timestamp_secs(
                timestamp.into_secs(timescale),
            ));
        });
    }
}

pub fn show_decoded_frame_info(
    ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    video: &re_renderer::video::Video,
    video_time: re_video::Time,
    video_buffers: &StableIndexDeque<&[u8]>,
) {
    let player_stream_id =
        re_renderer::video::VideoPlayerStreamId(ui.id().with("video_player").value());

    match video.frame_at(
        ctx.render_ctx(),
        player_stream_id,
        video_time,
        video_buffers,
    ) {
        Ok(VideoFrameTexture {
            texture,
            decoder_delay_state,
            show_spinner,
            frame_info,
            source_pixel_format,
        }) => {
            if let Some(frame_info) = frame_info {
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

            let response = if let Some(texture) = texture {
                crate::image::texture_preview_ui(
                    ctx.render_ctx(),
                    ui,
                    ui_layout,
                    "video_preview",
                    re_renderer::renderer::ColormappedTexture::from_unorm_rgba(texture),
                )
            } else {
                ui.allocate_response(
                    egui::Vec2::splat(ui.available_width().at_most(128.0)),
                    egui::Sense::hover(),
                )
            };

            if decoder_delay_state.should_rerequest_frame() {
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
            re_format::format_timestamp_secs(presentation_time_range.start.into_secs(timescale)),
            re_format::format_timestamp_secs(presentation_time_range.end.into_secs(timescale)),
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
        {
            if let Some(sample_idx) =
                video_descr.latest_sample_index_at_presentation_timestamp(presentation_timestamp)
            {
                ui.list_item_flat_noninteractive(
                    PropertyContent::new("Highest PTS so far").value_bool(has_sample_highest_pts_so_far[sample_idx])
                ).on_hover_text("Whether the presentation timestamp (PTS) at the this frame is the highest encountered so far. If false there are lower PTS values prior in the list.");
            }
        }
    }

    // Information about the current group of pictures this frame is part of.
    // Lookup via decode timestamp is faster, but it may not always be available.
    if let Some(gop_index) =
        video_descr.gop_index_containing_presentation_timestamp(presentation_timestamp)
    {
        ui.list_item_flat_noninteractive(
            PropertyContent::new("GOP index").value_text(gop_index.to_string()),
        )
        .on_hover_text("The index of the group of picture (GOP) that this sample belongs to.");

        if let Some(gop) = video_descr.gops.get(gop_index) {
            let first_sample = video_descr.samples.get(gop.sample_range.start);
            let last_sample = video_descr
                .samples
                .get(gop.sample_range.end.saturating_sub(1));

            if let (Some(first_sample), Some(last_sample)) = (first_sample, last_sample) {
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
    };
}
