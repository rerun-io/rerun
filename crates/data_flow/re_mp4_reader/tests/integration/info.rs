//! Integration tests for [`re_mp4_reader::Mp4Info`].

#![expect(clippy::unwrap_used)] // Okay to use unwrap in tests

use itertools::Itertools as _;
use re_chunk::{Chunk, ComponentIdentifier, EntityPath};
use re_log_types::TimelineName;
use re_mp4_reader::{Mode, Mp4Config, Mp4Samples, Mp4TranscodeOptions, load_mp4, load_mp4_info};
use re_sdk_types::archetypes::VideoStream;

use crate::{STREAMABLE_FIXTURES, fixture_path};

const MULTI_GOP_FIXTURE: &str = "Sintel_1080_10s_av1.mp4";

/// Fixtures that `Mode::Stream` emits without transcoding.
fn untranscoded_fixtures() -> impl Iterator<Item = &'static str> {
    std::iter::chain(STREAMABLE_FIXTURES.iter().copied(), [MULTI_GOP_FIXTURE])
}

fn keyframe_pts(samples: &Mp4Samples) -> Vec<i64> {
    let mut pts: Vec<i64> = std::iter::zip(samples.pts.iter(), samples.is_keyframe.iter())
        .filter_map(|(pts, is_keyframe)| is_keyframe.then_some(pts))
        .collect();
    pts.sort_unstable();
    pts
}

/// The chunks `Mode::Stream` emits for the file, in emission order.
fn streamed_chunks(file_name: &str) -> Vec<Chunk> {
    let config = Mp4Config {
        mode: Mode::Stream {
            chunk_by_gop: true,
            transcode: Mp4TranscodeOptions::default(),
            time_window: None,
        },
        ..Default::default()
    };
    load_mp4(
        &fixture_path(file_name),
        &config,
        &EntityPath::from("video"),
    )
    .unwrap()
    .map(|chunk| chunk.unwrap())
    .collect()
}

/// The sorted times at which `Mode::Stream` logs `component` for the file.
fn streamed_times(file_name: &str, component: ComponentIdentifier) -> Vec<i64> {
    let mut times: Vec<i64> = streamed_chunks(file_name)
        .iter()
        .filter(|chunk| !chunk.is_static() && chunk.components().contains_component(component))
        .flat_map(|chunk| {
            chunk.timelines()[&TimelineName::from_static_str("video")]
                .times_raw()
                .to_vec()
        })
        .collect();
    times.sort_unstable();
    times
}

/// The keyframe list must match what a full stream read emits,
/// since it exists to replace that read.
#[test]
fn keyframes_match_the_stream_marker_chunk() {
    for file_name in untranscoded_fixtures() {
        let info = load_mp4_info(&fixture_path(file_name)).unwrap();
        assert!(!info.has_b_frames, "{file_name}");
        assert_eq!(
            keyframe_pts(&info.samples),
            streamed_times(file_name, VideoStream::descriptor_is_keyframe().component),
            "{file_name}"
        );
    }
}

#[test]
fn pts_match_the_stream_timeline() {
    for file_name in untranscoded_fixtures() {
        let info = load_mp4_info(&fixture_path(file_name)).unwrap();
        let mut pts = info.samples.pts.as_slice().to_vec();
        pts.sort_unstable();
        assert_eq!(
            pts,
            streamed_times(file_name, VideoStream::descriptor_sample().component),
            "{file_name}"
        );
    }
}

/// `Mode::Stream` emits AV1, VP8 and VP9 samples as stored in the file,
/// whereas it rewrites H.264 and H.265 samples to Annex B.
#[test]
fn byte_spans_hold_the_streamed_samples() {
    for file_name in [
        "Big_Buck_Bunny_1080_1s_av1.mp4",
        "Big_Buck_Bunny_1080_1s_vp8.mp4",
        "Big_Buck_Bunny_1080_1s_vp9.mp4",
        MULTI_GOP_FIXTURE,
    ] {
        let path = fixture_path(file_name);
        let file = std::fs::read(&path).unwrap();
        let info = load_mp4_info(&path).unwrap();
        assert!(
            !info.has_b_frames,
            "{file_name}: rows are in emission order"
        );

        let chunks = streamed_chunks(file_name);
        let streamed_samples: Vec<_> = chunks
            .iter()
            .flat_map(|chunk| {
                chunk.iter_slices::<&[u8]>(VideoStream::descriptor_sample().component)
            })
            .flatten()
            .collect();
        assert_eq!(streamed_samples.len(), info.num_frames, "{file_name}");

        let spans = std::iter::zip(
            info.samples.byte_offset.iter(),
            info.samples.byte_size.iter(),
        );
        for (row, ((offset, size), streamed)) in
            std::iter::zip(spans, &streamed_samples).enumerate()
        {
            let stored = &file[offset as usize..(offset + size) as usize];
            assert!(stored == streamed.as_slice(), "{file_name}: sample {row}");
        }
    }
}

#[test]
fn describes_an_h264_file() {
    let info = load_mp4_info(&fixture_path("Big_Buck_Bunny_1080_1s_h264_nobframes.mp4")).unwrap();

    assert_eq!(info.codec, re_mp4_reader::VideoCodec::H264);
    assert_eq!(info.codec_string, "avc1.640028");
    assert_eq!(info.coded_dimensions, [1920, 1080]);
    assert_eq!(info.bit_depth, Some(8));
    assert_eq!(info.num_frames, 30);
    assert_eq!(info.samples.pts.len(), 30);
    assert_eq!(info.duration_nanos, 1_000_000_000);
}

#[test]
fn sample_table_is_consistent() {
    for file_name in std::iter::chain(
        untranscoded_fixtures(),
        [
            "Big_Buck_Bunny_1080_1s_h264.mp4",
            "Big_Buck_Bunny_1080_1s_h265.mp4",
        ],
    ) {
        let path = fixture_path(file_name);
        let info = load_mp4_info(&path).unwrap();
        let samples = &info.samples;
        let file_size = std::fs::metadata(&path).unwrap().len();

        // `frame_nr` is the rank of each sample's presentation time.
        let pts = samples.pts.as_slice();
        let mut by_pts: Vec<usize> = (0..pts.len()).collect();
        by_pts.sort_by_key(|&row| pts[row]);
        let frame_nr = samples.frame_nr.as_slice();
        for (rank, &row) in std::iter::zip(0_i64.., &by_pts) {
            assert_eq!(frame_nr[row], rank, "{file_name}");
        }

        // A sample is presented until the next one is, up to the rounding to nanoseconds.
        for (&row, &next_row) in by_pts.iter().tuple_windows() {
            let duration = samples.duration.value(row);
            assert!(
                (pts[next_row] - pts[row] - duration).abs() <= 1,
                "{file_name}"
            );
        }

        let dts = samples.dts.as_slice();
        assert!(dts.is_sorted(), "{file_name}: rows are in decode order");
        assert_eq!(info.has_b_frames, dts != pts, "{file_name}");

        // The samples are non-empty, disjoint byte ranges of the file.
        let mut spans: Vec<(u64, u64)> =
            std::iter::zip(samples.byte_offset.iter(), samples.byte_size.iter()).collect();
        spans.sort_unstable();
        let span_ends = spans.iter().map(|(offset, size)| offset + size);
        let next_starts =
            std::iter::chain(spans.iter().skip(1).map(|(offset, _)| *offset), [file_size]);
        for (&(_, size), end, next_start) in itertools::izip!(&spans, span_ends, next_starts) {
            assert!(size > 0, "{file_name}");
            assert!(end <= next_start, "{file_name}");
        }

        // The file starts on a keyframe, so every GOP runs from one keyframe row to the next.
        assert!(samples.is_keyframe.value(0), "{file_name}");
        let keyframe_rows = samples
            .is_keyframe
            .iter()
            .positions(|is_keyframe| is_keyframe);
        let gop_sizes: Vec<usize> = std::iter::chain(keyframe_rows, [info.num_frames])
            .tuple_windows()
            .map(|(start, end)| end - start)
            .collect();
        assert_eq!(
            Some(info.gop_size_min),
            gop_sizes.iter().copied().min(),
            "{file_name}"
        );
        assert_eq!(
            Some(info.gop_size_max),
            gop_sizes.iter().copied().max(),
            "{file_name}"
        );
    }
}

#[test]
fn counts_the_gops_of_a_multi_gop_file() {
    let info = load_mp4_info(&fixture_path(MULTI_GOP_FIXTURE)).unwrap();

    assert_eq!(info.num_frames, 300);
    assert_eq!(keyframe_pts(&info.samples).len(), 12);
    assert_eq!((info.gop_size_min, info.gop_size_max), (2, 72));
}

#[test]
fn a_single_gop_is_counted() {
    let info = load_mp4_info(&fixture_path("Big_Buck_Bunny_1080_1s_av1.mp4")).unwrap();

    assert_eq!(keyframe_pts(&info.samples).len(), 1);
    assert_eq!((info.gop_size_min, info.gop_size_max), (30, 30));
}

#[test]
fn b_frames_are_reported() {
    for file_name in [
        "Big_Buck_Bunny_1080_1s_h264.mp4",
        "Big_Buck_Bunny_1080_1s_h265.mp4",
    ] {
        let info = load_mp4_info(&fixture_path(file_name)).unwrap();
        assert!(info.has_b_frames, "{file_name}");
    }
}

#[test]
fn an_unsupported_codec_is_an_error() {
    let err = load_mp4_info(&fixture_path("mpeg4_part2.mp4")).unwrap_err();
    assert!(err.to_string().contains("mp4v"), "{err}");
}
