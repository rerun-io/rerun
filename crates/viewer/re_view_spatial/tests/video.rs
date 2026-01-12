#![expect(clippy::unwrap_used)] // It's a test!

use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::archetypes::{AssetVideo, VideoFrameReference, VideoStream};
use re_sdk_types::components::{self, MediaType, VideoTimestamp};
use re_sdk_types::datatypes;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotOptions;
use re_test_viewport::TestContextExt as _;
use re_video::{VideoCodec, VideoDataDescription};
use re_viewer_context::{TimeControlCommand, ViewClass as _};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

fn workspace_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf()
}

fn pixi_ffmpeg_path() -> std::path::PathBuf {
    workspace_dir().join(if cfg!(target_os = "windows") {
        ".pixi/envs/default/Library/bin/ffmpeg.exe"
    } else {
        ".pixi/envs/default/Library/bin/ffmpeg"
    })
}

fn video_test_file_mp4(codec: VideoCodec, need_dts_equal_pts: bool) -> std::path::PathBuf {
    let codec_str = match codec {
        VideoCodec::H264 => "h264",
        VideoCodec::H265 => "h265",
        VideoCodec::VP9 => "vp9",
        VideoCodec::VP8 => {
            panic!("We don't have test data for vp8, because Mp4 doesn't support vp8.")
        }
        VideoCodec::AV1 => "av1",
    };

    if need_dts_equal_pts && (codec == VideoCodec::H264 || codec == VideoCodec::H265) {
        // Only H264 and H265 have DTS != PTS when b-frames are present.
        workspace_dir().join(format!(
            "tests/assets/video/Big_Buck_Bunny_1080_1s_{codec_str}_nobframes.mp4",
        ))
    } else {
        workspace_dir().join(format!(
            "tests/assets/video/Big_Buck_Bunny_1080_1s_{codec_str}.mp4",
        ))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VideoTestSeekLocation {
    BeforeStart,
    Start,
    NotOnFrameboundary,
    BeyondEnd,
}

impl VideoTestSeekLocation {
    const ALL: [Self; 4] = [
        Self::BeforeStart,
        Self::Start,
        Self::NotOnFrameboundary,
        Self::BeyondEnd,
    ];

    fn get_time_ns(&self, frame_timestamps_nanos: &[i64]) -> i64 {
        match self {
            Self::BeforeStart => frame_timestamps_nanos[0] - 1_000,
            Self::Start => frame_timestamps_nanos[0],
            Self::NotOnFrameboundary => {
                // Videos with large GOPs cause a lot of decoding work on seek.
                // For software decoders this can take longer than we can bear in our debug test builds.
                // Therefore, pick a timestamp very close to the start of the video!
                frame_timestamps_nanos[4] + 10
            }
            Self::BeyondEnd => frame_timestamps_nanos.last().unwrap() + 1_000,
        }
    }

    fn get_label(&self) -> &str {
        match self {
            Self::BeforeStart => "before_start",
            Self::Start => "start",
            Self::NotOnFrameboundary => "not_on_frame_boundary",
            Self::BeyondEnd => "beyond_end",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VideoType {
    AssetVideo,
    VideoStream,
}

impl std::fmt::Display for VideoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AssetVideo => write!(f, "asset"),
            Self::VideoStream => write!(f, "stream"),
        }
    }
}

fn image_diff_threshold(codec: VideoCodec) -> f32 {
    match codec {
        // Despite version pinning, ffmpeg's results are quite different depending on the platform
        // and seemingly even between runs!
        VideoCodec::H264 | VideoCodec::H265 => 2.2,
        // AV1 has this problem as well but to a lesser extent.
        VideoCodec::AV1 => 1.2,

        _ => SnapshotOptions::default().threshold,
    }
}

fn image_failed_pixel_count_threshold(codec: VideoCodec) -> usize {
    match codec {
        // Despite version pinning, ffmpeg's results are quite different depending on the platform
        // and seemingly even between runs!
        VideoCodec::H264 | VideoCodec::H265 => 300,
        // AV1 has this problem as well but to a lesser extent.
        VideoCodec::AV1 => 100,

        _ => SnapshotOptions::default().failed_pixel_count_threshold,
    }
}

fn test_video(video_type: VideoType, codec: VideoCodec) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();

    // Use pixi ffmpeg install if available.
    let pixi_ffmpeg_path = pixi_ffmpeg_path();
    if pixi_ffmpeg_path.exists() {
        test_context.app_options.video.override_ffmpeg_path = true;
        test_context.app_options.video.ffmpeg_path = pixi_ffmpeg_path.to_str().unwrap().to_owned();

        re_log::info!("Using pixi ffmpeg at {pixi_ffmpeg_path:?}");
    } else {
        // End up using system install. Fine usually, no need to force a pixi environment here.
        re_log::info!("Pixi ffmpeg not found at {pixi_ffmpeg_path:?}");
    }

    let need_dts_equal_pts = video_type == VideoType::VideoStream; // TODO(#10090): Video stream doesn't support bframes
    let video_path = video_test_file_mp4(codec, need_dts_equal_pts);

    let video_asset = AssetVideo::from_file_path(&video_path).unwrap();
    let frame_timestamps_nanos = video_asset.read_frame_timestamps_nanos().unwrap();
    let timeline = test_context
        .active_timeline()
        .expect("should have an active timeline");

    match video_type {
        VideoType::AssetVideo => {
            test_context.log_entity("video", |builder| {
                builder.with_archetype(RowId::new(), TimePoint::default(), &video_asset)
            });

            test_context.log_entity("video", |mut builder| {
                for nanos in &frame_timestamps_nanos {
                    builder = builder.with_archetype(
                        RowId::new(),
                        [(timeline, *nanos)],
                        &VideoFrameReference::new(VideoTimestamp::from_nanos(*nanos)),
                    );
                }
                builder
            });
        }

        VideoType::VideoStream => {
            // Pretend the file is a video stream.
            let blob_bytes =
                datatypes::Blob::serialized_blob_as_slice(video_asset.blob.as_ref().unwrap())
                    .unwrap();
            let video_data_description = VideoDataDescription::load_from_bytes(
                blob_bytes,
                MediaType::mp4().as_str(),
                video_path.to_str().unwrap(),
            )
            .unwrap();

            assert!(
                video_data_description
                    .samples_statistics
                    .dts_always_equal_pts,
                "TODO(#10090): Video stream doesn't support bframes"
            );

            let mut annexb_stream_state = re_video::AnnexBStreamState::default();
            let samples_buffers = std::iter::once(blob_bytes).collect();

            for (sample_idx, sample) in video_data_description.samples.iter().enumerate() {
                let (codec, sample_bytes) = match video_data_description.codec {
                    VideoCodec::H264 => {
                        let avcc = video_data_description
                            .encoding_details
                            .as_ref()
                            .and_then(|e| e.stsd.as_ref())
                            .and_then(|stsd| match &stsd.contents {
                                re_mp4::StsdBoxContent::Avc1(avc1) => Some(avc1),
                                _ => None,
                            })
                            .expect("AVCC box should be present for H264 mp4");

                        let mut sample_bytes = Vec::new();
                        re_video::write_avc_chunk_to_nalu_stream(
                            avcc,
                            &mut sample_bytes,
                            &sample.get(&samples_buffers, sample_idx).unwrap(),
                            &mut annexb_stream_state,
                        )
                        .unwrap();

                        (components::VideoCodec::H264, sample_bytes)
                    }
                    VideoCodec::H265 => {
                        let hvcc = video_data_description
                            .encoding_details
                            .as_ref()
                            .and_then(|e| e.stsd.as_ref())
                            .and_then(|stsd| match &stsd.contents {
                                re_mp4::StsdBoxContent::Hev1(hvcc)
                                | re_mp4::StsdBoxContent::Hvc1(hvcc) => Some(hvcc),
                                _ => None,
                            })
                            .expect("HVCC box should be present for H264 mp4");

                        let mut sample_bytes = Vec::new();
                        re_video::write_hevc_chunk_to_nalu_stream(
                            hvcc,
                            &mut sample_bytes,
                            &sample.get(&samples_buffers, sample_idx).unwrap(),
                            &mut annexb_stream_state,
                        )
                        .unwrap();

                        (components::VideoCodec::H265, sample_bytes)
                    }
                    VideoCodec::AV1 => {
                        // Extract raw sample bytes, under av1 they're OBUs already!
                        let sample_bytes = sample.get(&samples_buffers, sample_idx).unwrap().data;
                        (components::VideoCodec::AV1, sample_bytes)
                    }
                    VideoCodec::VP9 => panic!("VP9 is not supported for video streams"),
                    VideoCodec::VP8 => panic!("VP8 is not supported for video streams"),
                };

                let time_ns = sample
                    .presentation_timestamp
                    .into_nanos(video_data_description.timescale.unwrap());

                test_context.log_entity("video", |builder| {
                    builder.with_archetype(
                        RowId::new(),
                        [(timeline, time_ns)],
                        &VideoStream::new(codec).with_sample(sample_bytes),
                    )
                });
            }
        }
    }

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_id = blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView2D::identifier(),
        ));

        // Set a background color other than black so we can see the effect of transparency on errors & lack thereof on the video.
        let property = ViewProperty::from_archetype::<
            re_sdk_types::blueprint::archetypes::Background,
        >(ctx.blueprint_db(), ctx.blueprint_query, view_id);
        property.save_blueprint_component(
            ctx,
            &re_sdk_types::blueprint::archetypes::Background::descriptor_kind(),
            &re_sdk_types::blueprint::components::BackgroundKind::SolidColor,
        );
        property.save_blueprint_component(
            ctx,
            &re_sdk_types::blueprint::archetypes::Background::descriptor_color(),
            &re_sdk_types::components::Color::from_rgb(200, 100, 200),
        );

        view_id
    });

    // Decoding videos can take quite a while!
    let step_dt_seconds = 1.0 / 4.0; // This is also the current egui_kittest default, but let's be explicit since we use `try_run_realtime`.
    let max_total_time_seconds = 60.0;

    let mut harness = test_context
        .setup_kittest_for_rendering_3d([300.0, 200.0])
        .with_step_dt(step_dt_seconds)
        .with_max_steps((max_total_time_seconds / step_dt_seconds) as u64)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);

            std::thread::sleep(std::time::Duration::from_millis(20));
        });

    for seek_location in VideoTestSeekLocation::ALL {
        // Using a single harness for all frames - we want to make sure that we use the same decoder,
        // not tearing down the video player!
        let desired_seek_ns = seek_location.get_time_ns(&frame_timestamps_nanos);
        test_context.send_time_commands(
            test_context.active_store_id(),
            [
                TimeControlCommand::SetActiveTimeline(*timeline.name()),
                TimeControlCommand::SetTime(desired_seek_ns.into()),
            ],
        );

        // Video decoding happens in a different thread, so it's important that we give it time
        // and don't busy loop.
        harness.try_run_realtime().unwrap();
        harness.snapshot_options(
            format!("video_{video_type}_{codec:?}_{}", seek_location.get_label()),
            &SnapshotOptions::new()
                .threshold(image_diff_threshold(codec))
                .failed_pixel_count_threshold(image_failed_pixel_count_threshold(codec)),
        );
    }
}

#[test]
fn test_video_asset_codec_h264() {
    test_video(VideoType::AssetVideo, VideoCodec::H264);
}

#[test]
fn test_video_asset_codec_h265() {
    test_video(VideoType::AssetVideo, VideoCodec::H265);
}

#[test]
fn test_video_asset_codec_vp9() {
    test_video(VideoType::AssetVideo, VideoCodec::VP9);
}

#[cfg(feature = "nasm")] // Need nasm for Av1 decoding on some platforms, otherwise we error.
#[test]
fn test_video_asset_codec_av1() {
    test_video(VideoType::AssetVideo, VideoCodec::AV1);
}

#[test]
fn test_video_stream_codec_h264() {
    test_video(VideoType::VideoStream, VideoCodec::H264);
}

#[test]
fn test_video_stream_codec_h265() {
    test_video(VideoType::VideoStream, VideoCodec::H265);
}

// TODO(#10186): Unsupported codec for VideoStream
// #[test]
// fn test_video_stream_codec_vp9() {
//     test_video(VideoType::VideoStream, VideoCodec::VP9);
// }

#[cfg(feature = "nasm")] // Need nasm for Av1 decoding on some platforms otherwise we error.
#[test]
fn test_video_stream_codec_av1() {
    test_video(VideoType::VideoStream, VideoCodec::AV1);
}
