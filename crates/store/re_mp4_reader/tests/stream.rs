//! Integration tests for [`re_mp4_reader`] stream mode.
//!
//! The headline property: streaming an mp4 from a file path (which reads sample
//! bytes on demand via `seek` + `read_exact`, never holding the whole file in
//! memory) must produce exactly the same chunks as loading the same bytes from
//! an in-memory buffer. That equivalence is what proves the per-sample byte
//! offsets used while streaming are correct.

#![expect(clippy::unwrap_used)] // Okay to use unwrap in tests

use std::path::PathBuf;
use std::time::Duration;

use re_chunk::{Chunk, EntityPath};
use re_mp4_reader::{
    Mode, Mp4Config, Mp4TranscodeOptions, TimeWindow, VideoCodec, load_mp4, load_mp4_from_bytes,
};
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components::IsKeyframe;

/// All codecs whose 1-second fixture is decodable in `Mode::Stream` (no
/// B-frames, not an image sequence). VP8/VP9 additionally exercise the
/// load-time sync-flag fix, which reads sample bytes through the reader.
const STREAMABLE_FIXTURES: &[&str] = &[
    "Big_Buck_Bunny_1080_1s_h264_nobframes.mp4",
    "Big_Buck_Bunny_1080_1s_h265_nobframes.mp4",
    "Big_Buck_Bunny_1080_1s_av1.mp4",
    "Big_Buck_Bunny_1080_1s_vp8.mp4",
    "Big_Buck_Bunny_1080_1s_vp9.mp4",
];

fn fixture_path(file_name: &str) -> PathBuf {
    // `crates/store/re_mp4_reader` → workspace root is three levels up.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .expect("path exists")
        .join("tests/assets/video")
        .join(file_name)
}

fn stream_config() -> Mp4Config {
    Mp4Config {
        mode: Mode::Stream {
            chunk_by_gop: true,
            transcode: Mp4TranscodeOptions::default(),
        },
        ..Default::default()
    }
}

/// Drain a chunk iterator, asserting no per-chunk error was surfaced.
fn collect_chunks(
    iter: impl Iterator<Item = Result<Chunk, re_mp4_reader::Mp4Error>>,
    label: &str,
) -> Vec<Chunk> {
    iter.map(|c| c.unwrap_or_else(|err| panic!("chunk error in {label}: {err}")))
        .collect()
}

/// Does this chunk carry video sample bytes?
fn is_sample_chunk(chunk: &Chunk) -> bool {
    chunk
        .components()
        .contains_component(VideoStream::descriptor_sample().component)
}

/// The sample chunks only — excludes the static codec chunk and the trailing
/// `IsKeyframe` marker chunk.
fn sample_chunks(chunks: &[Chunk]) -> Vec<&Chunk> {
    chunks
        .iter()
        .filter(|c| !c.is_static() && is_sample_chunk(c))
        .collect()
}

/// Sample times across the sample chunks, in emission order.
fn sample_times(chunks: &[Chunk]) -> Vec<i64> {
    let mut times = Vec::new();
    for chunk in sample_chunks(chunks) {
        assert_eq!(chunk.timelines().len(), 1);
        times.extend_from_slice(chunk.timelines().values().next().unwrap().times_raw());
    }
    times
}

/// The times on the sparse `IsKeyframe` marker chunk, asserting every value is
/// `true` and that exactly one such chunk exists.
fn keyframe_marker_times(chunks: &[Chunk], label: &str) -> Vec<i64> {
    let markers: Vec<&Chunk> = chunks
        .iter()
        .filter(|c| {
            !is_sample_chunk(c)
                && c.components()
                    .contains_component(VideoStream::descriptor_is_keyframe().component)
        })
        .collect();
    assert_eq!(
        markers.len(),
        1,
        "{label}: expected exactly one dedicated IsKeyframe marker chunk"
    );
    let marker = markers[0];
    assert!(
        !marker.is_static(),
        "{label}: the keyframe marker must be temporal"
    );

    let flags: Vec<bool> = marker
        .iter_component::<IsKeyframe>(VideoStream::descriptor_is_keyframe().component)
        .flat_map(|batch| batch.iter().map(|kf| bool::from(kf.0)).collect::<Vec<_>>())
        .collect();
    assert!(
        flags.iter().all(|&kf| kf),
        "{label}: the marker is sparse — only `true` may be logged, got {flags:?}"
    );

    assert_eq!(marker.timelines().len(), 1);
    marker
        .timelines()
        .values()
        .next()
        .unwrap()
        .times_raw()
        .to_vec()
}

#[test]
fn streaming_from_path_matches_in_memory() {
    let entity_path = EntityPath::from("video");

    for file_name in STREAMABLE_FIXTURES {
        let path = fixture_path(file_name);

        // Path-based: streams the file from disk via `BufReader<File>`.
        let from_path = collect_chunks(
            load_mp4(&path, &stream_config(), &entity_path).unwrap(),
            file_name,
        );

        // Bytes-based: same code path over an in-memory `Cursor`.
        let bytes = std::fs::read(&path).unwrap();
        let from_bytes = collect_chunks(
            load_mp4_from_bytes(bytes, &stream_config(), &entity_path, file_name).unwrap(),
            file_name,
        );

        // A codec chunk, at least one GOP chunk, and the keyframe marker.
        assert!(
            from_path.len() >= 3,
            "{file_name}: expected at least a codec chunk, one GOP chunk and the keyframe marker, got {}",
            from_path.len()
        );
        assert_eq!(
            from_path.len(),
            from_bytes.len(),
            "{file_name}: chunk count differs between path and in-memory streaming"
        );

        // The payloads (sample blobs, keyframe flags, timestamps) must be
        // byte-identical. We compare timelines and components rather than whole
        // chunks because `ChunkId`/`RowId` are randomly generated per chunk.
        for (i, (a, b)) in std::iter::zip(&from_path, &from_bytes).enumerate() {
            assert_eq!(
                a.timelines(),
                b.timelines(),
                "{file_name}: timeline mismatch in chunk {i}"
            );
            assert_eq!(
                a.components().0,
                b.components().0,
                "{file_name}: component data mismatch in chunk {i}"
            );
        }
    }
}

/// `IsKeyframe` is emitted as a sparse marker in its own chunk: only `true` rows,
/// no sample bytes alongside it, one row per GOP start.
///
/// Checked for every streamable codec, since a regression here silently costs us
/// GOP rebatching (see the module docs for why).
#[test]
fn is_keyframe_is_a_sparse_marker_in_its_own_chunk() {
    let entity_path = EntityPath::from("video");

    for file_name in STREAMABLE_FIXTURES {
        let chunks = collect_chunks(
            load_mp4(&fixture_path(file_name), &stream_config(), &entity_path).unwrap(),
            file_name,
        );

        // No sample chunk may carry the keyframe column.
        for chunk in sample_chunks(&chunks) {
            assert!(
                !chunk
                    .components()
                    .contains_component(VideoStream::descriptor_is_keyframe().component),
                "{file_name}: `is_keyframe` must not be co-located with sample bytes"
            );
        }

        let keyframes = keyframe_marker_times(&chunks, file_name);

        // Every GOP starts on a keyframe, so there is one marker row per GOP.
        let num_gops = sample_chunks(&chunks).len();
        assert_eq!(
            keyframes.len(),
            num_gops,
            "{file_name}: expected one keyframe row per GOP chunk"
        );

        // And each marker time is the first sample time of a GOP chunk.
        let gop_starts: Vec<i64> = sample_chunks(&chunks)
            .into_iter()
            .map(|c| c.timelines().values().next().unwrap().times_raw()[0])
            .collect();
        assert_eq!(
            keyframes, gop_starts,
            "{file_name}: keyframe times must be the GOP start times"
        );
    }
}

/// A B-frame mp4 in stream mode is transcoded with ffmpeg and emitted as a
/// normal `VideoStream` — a codec chunk plus per-GOP sample chunks.
///
/// Skipped (not failed) when ffmpeg isn't installed, so the suite still passes
/// on machines without it.
#[test]
fn b_frames_are_transcoded_into_a_video_stream() {
    let entity_path = EntityPath::from("video");
    let path = fixture_path("Big_Buck_Bunny_1080_1s_h264.mp4");
    let config = Mp4Config {
        mode: Mode::Stream {
            chunk_by_gop: true,
            transcode: Mp4TranscodeOptions::default(),
        },
        ..Default::default()
    };

    let iter = match load_mp4(&path, &config, &entity_path) {
        Ok(iter) => iter,
        Err(err) if err.to_string().contains("FFmpeg") => {
            eprintln!("skipping: ffmpeg not available ({err})");
            return;
        }
        Err(err) => panic!("unexpected error transcoding B-frames: {err}"),
    };

    let chunks = collect_chunks(iter, "h264_bframes");
    let static_chunks = chunks.iter().filter(|c| c.is_static()).count();
    assert_eq!(static_chunks, 1, "expected exactly one static codec chunk");

    // The B-frame fixture is the same content as the no-B-frame one, so the
    // transcoded output must carry the same number of samples — proves no frame
    // is dropped or duplicated by the round-trip through ffmpeg.
    let expected_samples = total_sample_rows(&collect_chunks(
        load_mp4(
            &fixture_path("Big_Buck_Bunny_1080_1s_h264_nobframes.mp4"),
            &stream_config(),
            &entity_path,
        )
        .unwrap(),
        "h264_nobframes",
    ));

    let times = sample_times(&chunks);
    assert_eq!(
        times.len(),
        expected_samples,
        "transcoded sample count must match the source frame count"
    );
    assert!(
        times.windows(2).all(|w| w[0] < w[1]),
        "transcoded PTS must be strictly increasing (B-frames stripped): {times:?}"
    );

    // The keyframe marker must still line up with the transcoded samples.
    let keyframes = keyframe_marker_times(&chunks, "h264_bframes");
    assert!(!keyframes.is_empty());
    assert!(
        keyframes.iter().all(|kf| times.contains(kf)),
        "every keyframe time must be one of the emitted sample times"
    );
}

/// Total number of sample rows across the sample chunks (excludes the static
/// codec chunk and the sparse `IsKeyframe` marker chunk).
fn total_sample_rows(chunks: &[Chunk]) -> usize {
    sample_chunks(chunks).into_iter().map(Chunk::num_rows).sum()
}

/// When ffmpeg is missing (here forced via a bogus `ffmpeg_override`), a B-frame
/// source surfaces the "not installed" error instead of loading. Forcing the
/// path keeps this deterministic regardless of the test machine's ffmpeg.
#[test]
fn b_frames_without_ffmpeg_reports_missing_ffmpeg() {
    let entity_path = EntityPath::from("video");
    let path = fixture_path("Big_Buck_Bunny_1080_1s_h264.mp4");

    let config = Mp4Config {
        mode: Mode::Stream {
            chunk_by_gop: true,
            transcode: Mp4TranscodeOptions::default()
                .with_ffmpeg_override(PathBuf::from("/definitely/not/a/real/ffmpeg")),
        },
        ..Default::default()
    };

    let msg = match load_mp4(&path, &config, &entity_path) {
        Ok(_) => panic!("expected a transcode error when ffmpeg is missing"),
        Err(err) => err.to_string(),
    };
    assert!(
        msg.contains("Couldn't find an installation of the FFmpeg executable"),
        "expected the FFmpeg-not-installed message, got: {msg}"
    );
}

/// A clean, B-frame-free source of *any* codec can be re-encoded to a different
/// output codec (here AV1 → H.264).
///
/// Skipped (not failed) when ffmpeg (or the libx264 encoder) isn't available.
#[test]
fn av1_source_transcodes_to_h264_output() {
    let entity_path = EntityPath::from("video");
    let path = fixture_path("Big_Buck_Bunny_1080_1s_av1.mp4");
    let config = Mp4Config {
        mode: Mode::Stream {
            chunk_by_gop: true,
            transcode: Mp4TranscodeOptions::default().with_output_codec(VideoCodec::H264),
        },
        ..Default::default()
    };

    let iter = match load_mp4(&path, &config, &entity_path) {
        Ok(iter) => iter,
        // Missing ffmpeg or missing libx264 encoder — environment-dependent, so skip.
        Err(err) if err.to_string().contains("FFmpeg") || err.to_string().contains("encoder") => {
            eprintln!("skipping: ffmpeg/encoder not available ({err})");
            return;
        }
        Err(err) => panic!("unexpected error transcoding AV1 → H.264: {err}"),
    };

    // The point of the test: this used to fail fast with an unsupported-codec
    // error before reaching ffmpeg. Now it produces a normal `VideoStream` — one
    // static codec chunk plus per-GOP sample chunks.
    let chunks = collect_chunks(iter, "av1_to_h264");
    let static_chunks = chunks.iter().filter(|c| c.is_static()).count();
    assert_eq!(static_chunks, 1, "expected one static codec chunk");
    assert!(
        chunks.iter().any(|c| !c.is_static()),
        "expected at least one sample chunk"
    );
}

/// Requesting the `output_codec` the source already uses is a no-op: it must
/// stay on the direct (no-ffmpeg) path rather than round-tripping through a
/// pointless re-encode.
///
/// Proven by pointing `ffmpeg_override` at a path that does not exist — if this
/// wrongly triggered a transcode it would fail trying to spawn that binary;
/// instead it reads directly and succeeds. Needs no ffmpeg, so it runs anywhere.
#[test]
fn requesting_the_source_codec_stays_on_the_direct_path() {
    let entity_path = EntityPath::from("video");
    // A clean, B-frame-free H.264 source; we ask for H.264 output again.
    let path = fixture_path("Big_Buck_Bunny_1080_1s_h264_nobframes.mp4");
    let config = Mp4Config {
        mode: Mode::Stream {
            chunk_by_gop: true,
            transcode: Mp4TranscodeOptions::default()
                .with_output_codec(VideoCodec::H264)
                .with_ffmpeg_override(PathBuf::from("/definitely/not/a/real/ffmpeg")),
        },
        ..Default::default()
    };

    let iter =
        load_mp4(&path, &config, &entity_path).expect("no-op output_codec must not invoke ffmpeg");
    let chunks = collect_chunks(iter, "h264_noop");
    assert_eq!(
        chunks.iter().filter(|c| c.is_static()).count(),
        1,
        "expected one static codec chunk"
    );
    assert!(
        chunks.iter().any(|c| !c.is_static()),
        "expected at least one sample chunk"
    );
}

/// Run a transcoding stream (`chunk_by_gop=true`), returning `None` — with a
/// printed reason — when the local ffmpeg or the required encoder isn't
/// available, or when the encode fails at runtime (e.g. a listed-but-broken
/// encoder). Mirrors the skip convention of the other transcode tests so the
/// suite still passes on a machine without a full ffmpeg build.
fn transcode_or_skip(
    path: &std::path::Path,
    transcode: Mp4TranscodeOptions,
    label: &str,
) -> Option<Vec<Chunk>> {
    let entity_path = EntityPath::from("video");
    let config = Mp4Config {
        mode: Mode::Stream {
            chunk_by_gop: true,
            transcode,
        },
        ..Default::default()
    };

    let is_env_error = |err: &re_mp4_reader::Mp4Error| {
        let msg = err.to_string();
        msg.contains("FFmpeg") || msg.contains("encoder") || msg.contains("transcode")
    };

    let iter = match load_mp4(path, &config, &entity_path) {
        Ok(iter) => iter,
        Err(err) if is_env_error(&err) => {
            eprintln!("skipping {label}: ffmpeg/encoder not available ({err})");
            return None;
        }
        Err(err) => panic!("unexpected error in {label}: {err}"),
    };

    // The eager checks pass, but the encode itself can still fail mid-stream
    // (e.g. an encoder present in `-encoders` but non-functional). Treat that as
    // a skip too rather than a hard failure.
    let mut chunks = Vec::new();
    for item in iter {
        match item {
            Ok(chunk) => chunks.push(chunk),
            Err(err) if is_env_error(&err) => {
                eprintln!("skipping {label}: transcode failed at runtime ({err})");
                return None;
            }
            Err(err) => panic!("chunk error in {label}: {err}"),
        }
    }
    Some(chunks)
}

/// `gop_size = N` must force a keyframe every `N` frames in the transcoded
/// output. With `chunk_by_gop=true` that means one temporal chunk per GOP, so
/// every GOP chunk but the last holds exactly `N` samples — this is what
/// verifies the `-force_key_frames` expression actually takes effect.
///
/// Skipped (not failed) when ffmpeg / the H.264 encoder isn't available.
#[test]
fn gop_size_forces_keyframe_spacing() {
    const GOP: usize = 10;
    let path = fixture_path("Big_Buck_Bunny_1080_1s_h264_nobframes.mp4");
    let Some(chunks) = transcode_or_skip(
        &path,
        Mp4TranscodeOptions::default().with_gop_size(GOP as u32),
        "gop_spacing",
    ) else {
        return;
    };

    let gop_sizes: Vec<usize> = sample_chunks(&chunks)
        .into_iter()
        .map(Chunk::num_rows)
        .collect();
    assert!(
        gop_sizes.len() >= 2,
        "gop_size={GOP} should force multiple GOPs on a 1s clip, got {gop_sizes:?}"
    );

    // One sparse marker row per forced keyframe, i.e. one per GOP chunk.
    let keyframes = keyframe_marker_times(&chunks, "gop_spacing");
    assert_eq!(
        keyframes.len(),
        gop_sizes.len(),
        "expected one keyframe marker row per GOP, got {keyframes:?} for GOPs {gop_sizes:?}"
    );
    for (i, &n) in gop_sizes.iter().enumerate() {
        if i + 1 < gop_sizes.len() {
            assert_eq!(
                n, GOP,
                "GOP {i} should hold exactly {GOP} samples, got {n} (all: {gop_sizes:?})"
            );
        } else {
            assert!(
                (1..=GOP).contains(&n),
                "the last GOP should hold 1..={GOP} samples, got {n}"
            );
        }
    }
}

/// The any→any generalization across a handful of `(source, output)` pairs:
/// each must produce a normal `VideoStream` (one codec chunk + per-GOP samples)
/// with strictly increasing PTS (i.e. B-frame-free). Exercises more of the
/// encoder table than the single AV1→H.264 case.
///
/// Each pair skips independently when its output encoder is unavailable.
#[test]
fn transcodes_across_codec_pairs() {
    let pairs = [
        (
            "Big_Buck_Bunny_1080_1s_h264_nobframes.mp4",
            VideoCodec::AV1,
            "h264_to_av1",
        ),
        (
            "Big_Buck_Bunny_1080_1s_h264_nobframes.mp4",
            VideoCodec::VP9,
            "h264_to_vp9",
        ),
        (
            "Big_Buck_Bunny_1080_1s_h265_nobframes.mp4",
            VideoCodec::H264,
            "h265_to_h264",
        ),
    ];

    let mut ran = 0;
    for (fixture, target, label) in pairs {
        let path = fixture_path(fixture);
        let Some(chunks) = transcode_or_skip(
            &path,
            Mp4TranscodeOptions::default().with_output_codec(target),
            label,
        ) else {
            continue;
        };

        assert_eq!(
            chunks.iter().filter(|c| c.is_static()).count(),
            1,
            "{label}: expected one static codec chunk"
        );

        let times = sample_times(&chunks);
        assert!(!times.is_empty(), "{label}: expected sample chunks");
        assert!(
            times.windows(2).all(|w| w[0] < w[1]),
            "{label}: transcoded PTS must be strictly increasing: {times:?}"
        );
        assert!(
            !keyframe_marker_times(&chunks, label).is_empty(),
            "{label}: expected a sparse keyframe marker"
        );
        ran += 1;
    }

    if ran == 0 {
        eprintln!("skipping transcodes_across_codec_pairs: no output encoders available");
    }
}

/// A time window always routes through the transcode: slicing is ffmpeg's job,
/// on every source — even one that streams directly when unwindowed.
///
/// Proven by pointing `ffmpeg_override` at a path that does not exist: the
/// windowed read of a clean AV1 source fails reaching ffmpeg instead of reading
/// directly. Needs no ffmpeg, so it runs anywhere.
#[test]
fn windowing_routes_through_the_transcode() {
    let entity_path = EntityPath::from("video");
    let path = fixture_path("Big_Buck_Bunny_1080_1s_av1.mp4");
    let window = TimeWindow::new(Duration::ZERO, Duration::from_millis(500)).expect("valid window");
    let config = Mp4Config {
        mode: Mode::Stream {
            chunk_by_gop: true,
            transcode: Mp4TranscodeOptions::default()
                .with_time_window(window)
                .with_ffmpeg_override(PathBuf::from("/definitely/not/a/real/ffmpeg")),
        },
        ..Default::default()
    };

    let msg = match load_mp4(&path, &config, &entity_path) {
        Ok(_) => panic!("a windowed read must transcode, so it must fail without ffmpeg"),
        Err(err) => err.to_string(),
    };
    assert!(
        msg.contains("Couldn't find an installation of the FFmpeg executable"),
        "expected the FFmpeg-not-installed message, got: {msg}"
    );
}

/// A windowed transcode emits fewer samples than a full one, with timestamps
/// rebased to zero and bounded by the window's length. The exact boundary
/// behavior at `end` is ffmpeg's (`-to` input seek).
///
/// Skipped (not failed) when ffmpeg isn't available.
#[test]
fn windowed_transcode_emits_only_the_window() {
    // B-frames force the transcode on both loads, so the comparison is
    // machinery-for-machinery.
    let path = fixture_path("Big_Buck_Bunny_1080_1s_h264.mp4");
    let Some(full) = transcode_or_skip(
        &path,
        Mp4TranscodeOptions::default(),
        "windowed_transcode_full",
    ) else {
        return;
    };

    let window = TimeWindow::new(Duration::from_millis(200), Duration::from_millis(600))
        .expect("valid window");
    let Some(windowed) = transcode_or_skip(
        &path,
        Mp4TranscodeOptions::default().with_time_window(window),
        "windowed_transcode",
    ) else {
        return;
    };

    let window_len_ns = 400_000_000;
    let times = sample_times(&windowed);
    assert!(!times.is_empty(), "the window must not be empty");
    assert!(
        times.iter().all(|&t| (0..=window_len_ns).contains(&t)),
        "every emitted time must be rebased to zero and lie within the window's length, got {times:?}"
    );
    assert!(
        times.len() < sample_times(&full).len(),
        "a windowed transcode must emit fewer samples than the full file"
    );
}
