//! Integration tests for [`re_mp4_reader`] stream mode.
//!
//! The headline property: streaming an mp4 from a file path (which reads sample
//! bytes on demand via `seek` + `read_exact`, never holding the whole file in
//! memory) must produce exactly the same chunks as loading the same bytes from
//! an in-memory buffer. That equivalence is what proves the per-sample byte
//! offsets used while streaming are correct.

use std::path::PathBuf;

use re_chunk::{Chunk, EntityPath};
use re_mp4_reader::{Mode, Mp4Config, load_mp4, load_mp4_from_bytes};

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
        mode: Mode::Stream { chunk_by_gop: true },
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

        // A codec chunk plus at least one GOP chunk.
        assert!(
            from_path.len() >= 2,
            "{file_name}: expected at least a codec chunk and one GOP chunk, got {}",
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
        mode: Mode::Stream { chunk_by_gop: true },
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

    let mut times: Vec<i64> = Vec::new();
    for chunk in chunks.iter().filter(|c| !c.is_static()) {
        assert_eq!(chunk.timelines().len(), 1);
        times.extend_from_slice(chunk.timelines().values().next().unwrap().times_raw());
    }
    assert_eq!(
        times.len(),
        expected_samples,
        "transcoded sample count must match the source frame count"
    );
    assert!(
        times.windows(2).all(|w| w[0] < w[1]),
        "transcoded PTS must be strictly increasing (B-frames stripped): {times:?}"
    );
}

/// Total number of sample rows across the non-static (temporal) chunks.
fn total_sample_rows(chunks: &[Chunk]) -> usize {
    chunks
        .iter()
        .filter(|c| !c.is_static())
        .map(Chunk::num_rows)
        .sum()
}

/// When ffmpeg is missing (here forced via a bogus `ffmpeg_override`), a B-frame
/// source surfaces the "not installed" error instead of loading. Forcing the
/// path keeps this deterministic regardless of the test machine's ffmpeg.
#[test]
fn b_frames_without_ffmpeg_reports_missing_ffmpeg() {
    let entity_path = EntityPath::from("video");
    let path = fixture_path("Big_Buck_Bunny_1080_1s_h264.mp4");

    let config = Mp4Config {
        mode: Mode::Stream { chunk_by_gop: true },
        ffmpeg_override: Some(PathBuf::from("/definitely/not/a/real/ffmpeg")),
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
