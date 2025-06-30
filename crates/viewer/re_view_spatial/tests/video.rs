#![expect(clippy::unwrap_used)] // It's a test!

use std::cell::Cell;

use re_chunk_store::RowId;
use re_log_types::{NonMinI64, TimeInt, TimePoint};
use re_types::{
    archetypes::{AssetVideo, VideoFrameReference, VideoStream},
    components::{self, MediaType, VideoTimestamp},
};
use re_video::{VideoCodec, VideoDataDescription};
use re_viewer_context::{ViewClass as _, test_context::TestContext};
use re_viewport::test_context_ext::TestContextExt as _;
use re_viewport_blueprint::ViewBlueprint;

fn video_test_file_mp4(codec: VideoCodec, need_dts_equal_pts: bool) -> std::path::PathBuf {
    let workspace_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf();

    let codec_str = match codec {
        VideoCodec::H264 => "h264",
        VideoCodec::H265 => "h265",
        VideoCodec::VP9 => "vp9",
        VideoCodec::VP8 => panic!("Don't have test data for vp8. Mp4 doesn't support it."),
        VideoCodec::AV1 => "av1",
    };

    if need_dts_equal_pts && (codec == VideoCodec::H264 || codec == VideoCodec::H265) {
        // Only H264 and H265 have DTS != PTS when b-frames are present.
        workspace_dir.join(format!(
            "tests/assets/video/Big_Buck_Bunny_1080_1s_{codec_str}_nobframes.mp4",
        ))
    } else if codec == VideoCodec::AV1 && cfg!(debug_assertions) {
        // AV1 decodes so insanely slow on debug, we have to use a minimal video to ensure that
        // decoding past the end of the video won't get stuck for long
        workspace_dir.join("tests/assets/video/Big_Buck_Bunny_1080_6frames_av1.mp4")
    } else {
        workspace_dir.join(format!(
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

fn convert_avcc_sample_to_annexb(
    video_data_description: &VideoDataDescription,
    sample: &re_video::SampleMetadata,
    mut raw_sample_bytes: &[u8],
) -> Vec<u8> {
    // Have to convert AVCC to AnnexB.
    let mut sample_bytes = Vec::new();

    const ANNEXB_NAL_START_CODE: &[u8] = &[0x00, 0x00, 0x00, 0x01];

    let avcc = video_data_description
        .encoding_details
        .as_ref()
        .and_then(|d| d.avcc())
        .expect("AVCC box should be present for H264 mp4");

    if sample.is_sync {
        for nal_unit in &avcc.avcc.contents.sequence_parameter_sets {
            sample_bytes.extend_from_slice(ANNEXB_NAL_START_CODE);
            sample_bytes.extend_from_slice(&nal_unit.bytes);
        }
        for nal_unit in &avcc.avcc.contents.picture_parameter_sets {
            sample_bytes.extend_from_slice(ANNEXB_NAL_START_CODE);
            sample_bytes.extend_from_slice(&nal_unit.bytes);
        }
    }

    // There can (and will be!) be several NAL units in a single sample.
    // Need to extract the length prefix one by one and use start codes instead.
    let length_prefix_size = avcc.avcc.length_size_minus_one as usize + 1;
    while !raw_sample_bytes.is_empty() {
        sample_bytes.extend_from_slice(ANNEXB_NAL_START_CODE);
        let sample_size = match length_prefix_size {
            1 => raw_sample_bytes[0] as usize,
            2 => u16::from_be_bytes(
                #[expect(clippy::unwrap_used)] // can't fail
                raw_sample_bytes[..2].try_into().unwrap(),
            ) as usize,
            4 => u32::from_be_bytes(
                #[expect(clippy::unwrap_used)] // can't fail
                raw_sample_bytes[..4].try_into().unwrap(),
            ) as usize,
            _ => {
                panic!("Bad length prefix size: {length_prefix_size}")
            }
        };

        let data_start = length_prefix_size; // Skip the size.
        let data_end = sample_size + length_prefix_size;

        sample_bytes.extend_from_slice(&raw_sample_bytes[data_start..data_end]);
        raw_sample_bytes = &raw_sample_bytes[data_end..];
    }

    sample_bytes
}

fn test_video(video_type: VideoType, codec: VideoCodec) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();

    let need_dts_equal_pts = video_type == VideoType::VideoStream; // TODO(#10090): Video stream doesn't support bframes
    let video_path = video_test_file_mp4(codec, need_dts_equal_pts);

    let video_asset = AssetVideo::from_file_path(&video_path).unwrap();
    let frame_timestamps_nanos = video_asset.read_frame_timestamps_nanos().unwrap();
    let timeline = test_context.active_timeline();

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
            let blob_bytes = video_asset.video_blob_bytes().unwrap();
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

            for sample in video_data_description.samples.iter() {
                let raw_sample_bytes =
                    &blob_bytes[sample.byte_span.start as usize..sample.byte_span.end() as usize];

                let (codec, sample_bytes) = match video_data_description.codec {
                    VideoCodec::H264 => {
                        let sample_bytes = convert_avcc_sample_to_annexb(
                            &video_data_description,
                            sample,
                            raw_sample_bytes,
                        );

                        (components::VideoCodec::H264, sample_bytes)
                    }
                    VideoCodec::H265 => panic!("H265 is not supported for video streams"),
                    VideoCodec::VP9 => panic!("VP9 is not supported for video streams"),
                    VideoCodec::VP8 => panic!("VP8 is not supported for video streams"),
                    VideoCodec::AV1 => panic!("AV1 is not supported for video streams"),
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

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView2D::identifier(),
        ))
    });

    // Decoding videos can take quite a while!
    let step_dt_seconds = 1.0 / 4.0; // This is also the default, but let's be explicit since we use `try_run_realtime`.
    let max_total_time_seconds = 10.0;

    // Using a single harness for all frames - we want to make sure that we use the same decoder,
    // not tearing down the video player!
    let desired_seek_ns = Cell::new(0);
    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_step_dt(step_dt_seconds)
        .with_max_steps((max_total_time_seconds / step_dt_seconds) as u64)
        .with_size(egui::vec2(300.0, 200.0))
        .build(|ctx| {
            {
                let mut time_ctrl = test_context.recording_config.time_ctrl.write();
                time_ctrl.set_time(TimeInt::from_nanos(
                    NonMinI64::new(desired_seek_ns.get()).unwrap(),
                ));
            }
            test_context.run_with_single_view(ctx, view_id);

            std::thread::sleep(std::time::Duration::from_millis(20));
        });

    for seek_location in VideoTestSeekLocation::ALL {
        if codec == VideoCodec::H264 && seek_location == VideoTestSeekLocation::BeyondEnd {
            // TODO(#8073): H264 decoder never finishes for the last few frames.
            continue;
        }

        desired_seek_ns.set(seek_location.get_time_ns(&frame_timestamps_nanos));

        // Video decoding happens in a different thread, so it's important that we give it time
        // and don't busy loop.
        harness.try_run_realtime().unwrap();
        harness.snapshot(&format!(
            "video_{video_type}_{codec:?}_{}",
            seek_location.get_label()
        ));
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

#[test]
fn test_video_asset_codec_av1() {
    test_video(VideoType::AssetVideo, VideoCodec::AV1);
}

#[test]
fn test_video_stream_codec_h264() {
    test_video(VideoType::VideoStream, VideoCodec::H264);
}

// TODO(#10185): Unsupported codec for VideoStream
// #[test]
// fn test_video_stream_codec_h265() {
//     test_video(VideoType::VideoStream, VideoCodec::H265);
// }

// TODO(#10186): Unsupported codec for VideoStream
// #[test]
// fn test_video_stream_codec_vp9() {
//     test_video(VideoType::VideoStream, VideoCodec::VP9);
// }

// TODO(#10184): Unsupported codec for VideoStream
// #[test]
// fn test_video_stream_codec_av1() {
//     test_video(VideoType::VideoStream, VideoCodec::AV1);
// }
