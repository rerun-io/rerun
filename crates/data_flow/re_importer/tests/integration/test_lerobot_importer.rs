use std::sync::Arc;

use re_chunk::Chunk;
use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle};
use re_lerobot::{LeRobotConfig, LeRobotDataset, VideoMode};
use re_log_types::StoreId;
use re_sdk_types::archetypes::VideoStream;

const FIXTURES: [&str; 2] = ["v21_apple_storage", "v30_apple_storage"];

fn fixture(name: &str) -> std::path::PathBuf {
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map_or_else(
            || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            std::path::PathBuf::from,
        )
        .join("tests/assets/lerobot")
        .join(name)
}

/// Is an `ffmpeg` executable available on `PATH`?
fn ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Gate for the tests that need ffmpeg (the fixture's H.264 stream has B-frames, which
/// forces a re-encode — `VideoStream` cannot model decode-order reordering).
///
/// Locally a missing ffmpeg skips the test; on CI it fails instead, so the v3 video
/// coverage can never silently disappear (CI gets ffmpeg from the pixi environment).
fn ffmpeg_available_or_fail_on_ci(test_name: &str) -> bool {
    if ffmpeg_available() {
        return true;
    }
    assert!(
        std::env::var_os("CI").is_none(),
        "{test_name} needs ffmpeg, which the pixi environment provides on CI — \
         a missing ffmpeg here means the v3 video coverage is gone"
    );
    eprintln!("skipping {test_name}: ffmpeg is not available");
    false
}

/// All chunks of all episodes, in load order.
fn load_all_chunks(fixture_name: &str, config: &LeRobotConfig) -> Vec<Chunk> {
    let dataset = LeRobotDataset::open(fixture(fixture_name)).expect("fixture should open");
    dataset
        .episodes()
        .flat_map(|episode| {
            dataset
                .stream(episode, config)
                .expect("fixture episode should stream")
                .map(|chunk| chunk.expect("fixture chunk should build"))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Schema-level snapshot.
///
/// Episodes share one store here (each is its own recording in the real importer); for
/// schema purposes the union across episodes is what matters. v3 videos are skipped:
/// the fixture's H.264 B-frames force an ffmpeg re-encode, and a snapshot must not depend on the
/// environment — `v3_video_structure_with_ffmpeg` covers the video half.
#[test]
fn test_lerobot_importer_schema() {
    for fixture_name in FIXTURES {
        let config = LeRobotConfig {
            video: if fixture_name == "v30_apple_storage" {
                VideoMode::Skip
            } else {
                VideoMode::Native
            },
            ..Default::default()
        };

        let store_handle = ChunkStoreHandle::new(ChunkStore::new(
            StoreId::random(re_log_types::StoreKind::Recording, "test_lerobot_importer"),
            ChunkStoreConfig::default(),
        ));

        {
            let mut store = store_handle.write();
            for chunk in load_all_chunks(fixture_name, &config) {
                store.insert_chunk(&Arc::new(chunk)).unwrap();
            }
        }

        let schema = store_handle.read().schema().chunk_column_descriptors();
        insta::assert_debug_snapshot!(format!("{fixture_name}_schema"), schema);
    }
}

/// The full v3 output schema with videos on (the default config): the snapshot to diff
/// against when a feature kind is added or changed.
///
/// Schema descriptors carry no encoded bytes, so unlike sample data they are stable
/// across ffmpeg versions and can be snapshotted.
#[test]
fn v3_native_schema_with_ffmpeg() {
    if !ffmpeg_available_or_fail_on_ci("v3_native_schema_with_ffmpeg") {
        return;
    }

    let store_handle = ChunkStoreHandle::new(ChunkStore::new(
        StoreId::random(re_log_types::StoreKind::Recording, "test_lerobot_importer"),
        ChunkStoreConfig::default(),
    ));

    {
        let mut store = store_handle.write();
        for chunk in load_all_chunks("v30_apple_storage", &LeRobotConfig::default()) {
            store.insert_chunk(&Arc::new(chunk)).unwrap();
        }
    }

    let schema = store_handle.read().schema().chunk_column_descriptors();
    insta::assert_debug_snapshot!("v30_apple_storage_native_schema", schema);
}

/// Two independent `open()`s of the same dataset must stream an episode with the same
/// chunk structure, in the same order. The emit order shapes the `.rrd` output bytes, so
/// it must not depend on per-instance or per-process hash seeds — the same input must
/// reproduce the same output.
#[test]
fn independent_opens_stream_identical_structure() {
    let config = LeRobotConfig {
        video: VideoMode::Skip,
        ..Default::default()
    };
    let structure = |fixture_name: &str| -> Vec<(String, usize, bool)> {
        let dataset = LeRobotDataset::open(fixture(fixture_name)).expect("fixture opens");
        let episode = dataset.episodes().next().expect("fixture has episodes");
        dataset
            .stream(episode, &config)
            .expect("fixture episode should stream")
            .map(|chunk| chunk.expect("fixture chunk should build"))
            .map(|chunk| {
                (
                    chunk.entity_path().to_string(),
                    chunk.num_rows(),
                    chunk.is_static(),
                )
            })
            .collect()
    };

    for fixture_name in FIXTURES {
        let first = structure(fixture_name);
        let second = structure(fixture_name);
        assert!(!first.is_empty(), "{fixture_name}: chunks expected");
        assert_eq!(
            first, second,
            "{fixture_name}: two opens must stream identically"
        );
    }
}

/// Without ffmpeg, streaming a v3 video that needs a re-encode (here: the fixture's
/// H.264 B-frames) yields an `Err` item that names both
/// remedies (install ffmpeg / `VideoMode::Skip`) instead of panicking or dying silently.
///
/// Skipped when ffmpeg is available (the transcode then succeeds).
#[test]
fn v3_video_without_ffmpeg_yields_actionable_error() {
    if ffmpeg_available() {
        eprintln!("skipping: ffmpeg is available, the transcode will succeed");
        return;
    }

    let dataset = LeRobotDataset::open(fixture("v30_apple_storage")).expect("fixture opens");
    let episode = dataset.episodes().next().expect("fixture has episodes");
    let errors: Vec<String> = dataset
        .stream(episode, &LeRobotConfig::default())
        .expect("planning needs no ffmpeg")
        .filter_map(|result| result.err())
        .map(|err| err.to_string())
        .collect();

    assert_eq!(errors.len(), 1, "one error for the one video feature");
    assert!(
        errors[0].contains("ffmpeg") && errors[0].contains("VideoMode::Skip"),
        "the error must name both remedies, got: {}",
        errors[0]
    );
}

/// Each streamed v3 episode covers exactly its own rows of the shared data file: the
/// per-episode chunk row counts match the episode lengths from `meta/episodes`, and the
/// derived `frame_index` timeline restarts at zero.
#[test]
fn v3_episode_row_ranges_are_exclusive() {
    let dataset = LeRobotDataset::open(fixture("v30_apple_storage")).expect("fixture opens");
    let config = LeRobotConfig {
        video: VideoMode::Skip,
        ..Default::default()
    };
    let episode_lengths = [299_i64, 300, 300];

    for (episode, expected_len) in std::iter::zip(dataset.episodes(), episode_lengths) {
        let chunks: Vec<Chunk> = dataset
            .stream(episode, &config)
            .expect("fixture episode should stream")
            .map(|chunk| chunk.expect("fixture chunk should build"))
            .collect();

        let action = chunks
            .iter()
            .find(|c| c.entity_path() == &re_chunk::EntityPath::from("/action"))
            .expect("fixture episodes have an /action feature");
        assert_eq!(
            i64::try_from(action.num_rows()).expect("row count fits"),
            expected_len,
            "episode {episode:?}"
        );

        let times = action
            .timelines()
            .get(&re_chunk::TimelineName::from("frame_index"))
            .expect("the /action chunk lands on the frame_index timeline")
            .times_raw();
        assert_eq!(times.first(), Some(&0), "episode {episode:?} starts at 0");
        assert_eq!(
            times.last(),
            Some(&(expected_len - 1)),
            "episode {episode:?}"
        );
    }
}

/// The v3 video half, streamed through `re_mp4_reader` with the episode's time window:
/// one static codec chunk, sample chunks retagged onto the episode's `frame_index`
/// sequence timeline, and a sparse `IsKeyframe` marker. Structure only — encoded sample
/// bytes are ffmpeg-version-dependent.
///
/// Skipped when ffmpeg is not available (the fixture's H.264 B-frames force a re-encode;
/// B-frame-free videos such as AV1 would stream directly).
#[test]
fn v3_video_structure_with_ffmpeg() {
    if !ffmpeg_available_or_fail_on_ci("v3_video_structure_with_ffmpeg") {
        return;
    }

    let dataset = LeRobotDataset::open(fixture("v30_apple_storage")).expect("fixture opens");
    let config = LeRobotConfig::default();
    let episode_lengths = [299_i64, 300, 300];

    for (episode, expected_len) in std::iter::zip(dataset.episodes(), episode_lengths) {
        let chunks: Vec<Chunk> = dataset
            .stream(episode, &config)
            .expect("fixture episode should stream")
            .map(|chunk| chunk.expect("fixture chunk should build"))
            .filter(|chunk| {
                chunk.entity_path() == &re_chunk::EntityPath::from("/observation.image")
            })
            .collect();

        let codec_chunks: Vec<&Chunk> = chunks
            .iter()
            .filter(|c| {
                c.is_static()
                    && c.components()
                        .contains_component(VideoStream::descriptor_codec().component)
            })
            .collect();
        assert_eq!(
            codec_chunks.len(),
            1,
            "episode {episode:?}: one codec chunk"
        );

        let sample_chunks: Vec<&Chunk> = chunks
            .iter()
            .filter(|c| {
                !c.is_static()
                    && c.components()
                        .contains_component(VideoStream::descriptor_sample().component)
            })
            .collect();
        assert!(
            !sample_chunks.is_empty(),
            "episode {episode:?}: sample chunks expected"
        );

        let mut times: Vec<i64> = Vec::new();
        for chunk in &sample_chunks {
            let time_column = chunk
                .timelines()
                .get(&re_chunk::TimelineName::from("frame_index"))
                .expect("samples must land on the episode's frame_index timeline");
            assert_eq!(
                time_column.timeline().typ(),
                re_log_types::TimeType::Sequence,
                "frame_index is a sequence timeline"
            );
            times.extend_from_slice(time_column.times_raw());
        }
        let num_samples = i64::try_from(times.len()).expect("sample count fits");
        assert!(
            times.iter().all(|&t| (0..=expected_len).contains(&t)),
            "episode {episode:?}: retagged frames must lie within the episode, got range \
             {:?}..={:?} over {num_samples} samples",
            times.iter().min(),
            times.iter().max()
        );
        assert!(
            (expected_len - 5..=expected_len).contains(&num_samples),
            "episode {episode:?}: expected about {expected_len} samples, got {num_samples}"
        );

        assert!(
            chunks.iter().any(|c| {
                c.components()
                    .contains_component(VideoStream::descriptor_is_keyframe().component)
            }),
            "episode {episode:?}: an IsKeyframe marker chunk is expected"
        );
    }
}
