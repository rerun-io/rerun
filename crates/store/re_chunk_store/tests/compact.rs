//! Tests for `compacted()` and `finalize_compaction`.

#![cfg(test)]

use std::sync::Arc;

use re_chunk::{Chunk, RowId};
use re_chunk_store::{ChunkStore, ChunkStoreConfig, CompactionOptions, IsStartOfGop};
use re_log_types::example_components::{MyPoint, MyPoints};
use re_log_types::{EntityPath, TimePoint, Timeline};
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components::{IsKeyframe, VideoCodec, VideoSample};

/// Builds a store with many single-row chunks sharing entity `/sensor` and
/// timeline `"frame"`. Intentionally fragmented to trigger compaction.
fn fragmented_store() -> ChunkStore {
    let store_id = re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app");
    let mut store = ChunkStore::new(store_id, ChunkStoreConfig::ALL_DISABLED);

    let entity_path: EntityPath = "/sensor".into();
    let timeline_frame = Timeline::new_sequence("frame");

    for i in 0..20 {
        let timepoint = TimePoint::from_iter([(timeline_frame, i as i64)]);
        let point = MyPoint::new(i as f32, i as f32);
        let chunk = Chunk::builder(entity_path.clone())
            .with_component_batch(
                RowId::new(),
                timepoint,
                (MyPoints::descriptor_points(), &[point]),
            )
            .build()
            .expect("build chunk");
        store.insert_chunk(&Arc::new(chunk)).expect("insert chunk");
    }

    store
}

fn options(num_extra_passes: Option<usize>) -> CompactionOptions {
    CompactionOptions {
        config: ChunkStoreConfig::DEFAULT,
        num_extra_passes,
        is_start_of_gop: None,
        split_size_ratio: None,
        fix_keyframe: false,
    }
}

#[test]
fn compacted_reduces_chunk_count() {
    let store = fragmented_store();
    let before = store.num_physical_chunks();
    let compacted = store.compacted(&options(Some(50))).expect("compacted");
    assert!(compacted.num_physical_chunks() < before);
}

#[test]
fn finalize_compaction_converges() {
    let store = fragmented_store()
        .compacted(&options(Some(50)))
        .expect("initial");
    let before = store.num_physical_chunks();
    let store2 = store
        .finalize_compaction(&options(Some(5)))
        .expect("idempotent");
    assert_eq!(before, store2.num_physical_chunks());
}

#[test]
fn compacted_preserves_row_count() {
    let store = fragmented_store();
    let rows_before: u64 = store
        .iter_physical_chunks()
        .map(|c| c.num_rows() as u64)
        .sum();
    let compacted = store.compacted(&options(Some(50))).expect("compacted");
    let rows_after: u64 = compacted
        .iter_physical_chunks()
        .map(|c| c.num_rows() as u64)
        .sum();
    assert_eq!(rows_before, rows_after);
}

// --- Keyframe marker tests ---

fn synthetic_video_store(entity: &str, num_samples: i64) -> ChunkStore {
    let store_id = re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "video_test");
    let mut store = ChunkStore::new(store_id, ChunkStoreConfig::ALL_DISABLED);

    let entity_path: EntityPath = entity.into();

    // Static VideoCodec chunk (required by rebatch's codec lookup).
    let codec_chunk = Chunk::builder(entity_path.clone())
        .with_component(
            RowId::new(),
            TimePoint::default(), // static
            VideoStream::descriptor_codec(),
            &VideoCodec::H264,
        )
        .expect("build codec chunk")
        .build()
        .expect("finalize codec chunk");
    store
        .insert_chunk(&Arc::new(codec_chunk))
        .expect("insert codec");

    // VideoSample chunks: one per frame, with placeholder bytes (the test's
    // is_start_of_gop callback uses the row index, not the bytes).
    let timeline_frame = Timeline::new_sequence("frame");
    for i in 0..num_samples {
        let timepoint = TimePoint::from_iter([(timeline_frame, i)]);
        let placeholder = vec![0u8; 4];
        let sample = VideoSample::from(placeholder);
        let chunk = Chunk::builder(entity_path.clone())
            .with_component(
                RowId::new(),
                timepoint,
                VideoStream::descriptor_sample(),
                &sample,
            )
            .expect("build sample chunk")
            .build()
            .expect("finalize sample chunk");
        store.insert_chunk(&Arc::new(chunk)).expect("insert sample");
    }

    store
}

/// Compaction options that drive GoP rebatching with a deterministic
/// "every Nth frame is a keyframe" rule.
fn video_options(num_samples: i64, period: i64) -> CompactionOptions {
    let counter = Arc::new(std::sync::atomic::AtomicI64::new(0));
    let is_start_of_gop: IsStartOfGop = Arc::new(move |_data, _codec| {
        // We don't get the row's time/index in the callback, so use a counter
        // that increments per call. Sample order matches insertion order in this
        // synthetic store, so this gives a deterministic period.
        let i = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % num_samples;
        Ok(i % period == 0)
    });

    CompactionOptions {
        config: ChunkStoreConfig::DEFAULT,
        num_extra_passes: Some(2),
        is_start_of_gop: Some(is_start_of_gop),
        split_size_ratio: None,
        fix_keyframe: false,
    }
}

/// Like [`synthetic_video_store`] but every sample chunk is logged on two
/// timelines (`frame` + `log_time`), to exercise multi-timeline rebatching.
fn synthetic_multi_timeline_video_store(entity: &str, num_samples: i64) -> ChunkStore {
    let store_id = re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "video_test");
    let mut store = ChunkStore::new(store_id, ChunkStoreConfig::ALL_DISABLED);

    let entity_path: EntityPath = entity.into();

    let codec_chunk = Chunk::builder(entity_path.clone())
        .with_component(
            RowId::new(),
            TimePoint::default(),
            VideoStream::descriptor_codec(),
            &VideoCodec::H264,
        )
        .expect("build codec chunk")
        .build()
        .expect("finalize codec chunk");
    store
        .insert_chunk(&Arc::new(codec_chunk))
        .expect("insert codec");

    let timeline_frame = Timeline::new_sequence("frame");
    let timeline_log = Timeline::new_duration("log_time");
    for i in 0..num_samples {
        let timepoint = TimePoint::from_iter([
            (timeline_frame, i),
            (timeline_log, i * 1_000_000), // 1ms per frame
        ]);
        let placeholder = vec![0u8; 4];
        let sample = VideoSample::from(placeholder);
        let chunk = Chunk::builder(entity_path.clone())
            .with_component(
                RowId::new(),
                timepoint,
                VideoStream::descriptor_sample(),
                &sample,
            )
            .expect("build sample chunk")
            .build()
            .expect("finalize sample chunk");
        store.insert_chunk(&Arc::new(chunk)).expect("insert sample");
    }

    store
}

/// True iff `chunk` carries a column with `descriptor.component == component`.
fn chunk_has_component(chunk: &re_chunk::ChunkShared, component: &str) -> bool {
    chunk
        .components()
        .values()
        .any(|c| c.descriptor.component.as_str() == component)
}

/// Optimize emits one `is_keyframe` marker per keyframe, in chunks that don't
/// also carry `VideoSample` (so a keyframe lookup never drags the encoded video).
#[test]
fn keyframe_markers_share_store_but_not_chunks() {
    let num_samples: i64 = 30;
    let period: i64 = 5; // 6 keyframes (frames 0, 5, 10, 15, 20, 25)
    let expected_keyframes = (num_samples + period - 1) / period;

    let store = synthetic_video_store("/video", num_samples);
    let compacted = store
        .compacted(&video_options(num_samples, period))
        .unwrap();

    let mut kf_rows = 0u64;
    let mut saw_sample_chunk = false;
    for chunk in compacted.iter_physical_chunks() {
        let has_kf = chunk_has_component(chunk, "VideoStream:is_keyframe");
        let has_sample = chunk_has_component(chunk, "VideoStream:sample");

        // Load-bearing invariant: keyframe markers and VideoSample never co-locate.
        assert!(
            !(has_kf && has_sample),
            "no chunk should carry both is_keyframe and VideoSample columns: {:?}",
            chunk
                .components()
                .values()
                .map(|c| c.descriptor.component.as_str())
                .collect::<Vec<_>>()
        );

        if has_kf {
            kf_rows += chunk.num_rows() as u64;
        }
        if has_sample {
            saw_sample_chunk = true;
        }
    }

    assert_eq!(
        kf_rows, expected_keyframes as u64,
        "one is_keyframe row per keyframe"
    );
    assert!(
        saw_sample_chunk,
        "VideoSample chunks should still exist alongside the markers"
    );
}

/// When sample chunks carry multiple timelines, the rebuilt keyframe marker
/// chunk must carry the same set, otherwise queries on the non-chosen
/// timelines return nothing.
#[test]
fn keyframe_chunk_preserves_all_source_timelines() {
    let num_samples: i64 = 30;
    let period: i64 = 5;

    let store = synthetic_multi_timeline_video_store("/video", num_samples);
    let compacted = store
        .compacted(&video_options(num_samples, period))
        .unwrap();

    let kf_chunks: Vec<_> = compacted
        .iter_physical_chunks()
        .filter(|c| chunk_has_component(c, "VideoStream:is_keyframe"))
        .collect();
    assert!(
        !kf_chunks.is_empty(),
        "expected at least one keyframe chunk"
    );

    for chunk in &kf_chunks {
        let names: std::collections::BTreeSet<_> = chunk.timelines().keys().copied().collect();
        assert!(
            names.contains(&"frame".into()) && names.contains(&"log_time".into()),
            "keyframe chunk must carry both source timelines, got: {names:?}",
        );
        for tc in chunk.timelines().values() {
            assert_eq!(tc.times_raw().len(), chunk.num_rows());
        }
    }
}

/// Re-running optimize on an already-optimized store must not duplicate
/// keyframe markers. Optimize owns the `VideoStream:is_keyframe` descriptor
/// and re-derives it on every pass.
#[test]
fn keyframe_markers_are_idempotent_across_optimize_runs() {
    let num_samples: i64 = 30;
    let period: i64 = 5;

    let count_keyframe_rows = |store: &ChunkStore| -> u64 {
        store
            .iter_physical_chunks()
            .filter(|c| chunk_has_component(c, "VideoStream:is_keyframe"))
            .map(|c| c.num_rows() as u64)
            .sum()
    };

    let store = synthetic_video_store("/video", num_samples);
    // Fresh `video_options` per pass: the closure captures an `AtomicI64`
    // counter, so reusing the same options would carry state across runs.
    let once = store
        .compacted(&video_options(num_samples, period))
        .unwrap();
    let twice = once.compacted(&video_options(num_samples, period)).unwrap();

    assert_eq!(
        count_keyframe_rows(&once),
        count_keyframe_rows(&twice),
        "second optimize pass must not duplicate keyframe markers",
    );
}

// --- Helpers for tests that need a deterministic keyframe callback. ---

/// Build a video store where each sample's bytes encode its frame index as a
/// little-endian `u32`. Pair with [`indexed_keyframe_callback`] to get a
/// deterministic "every Nth frame is a keyframe" rule that doesn't depend on
/// chunk iteration order.
fn indexed_video_store(entity: &str, num_samples: i64) -> ChunkStore {
    let store_id = re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "video_test");
    let mut store = ChunkStore::new(store_id, ChunkStoreConfig::ALL_DISABLED);
    let entity_path: EntityPath = entity.into();

    let codec_chunk = Chunk::builder(entity_path.clone())
        .with_component(
            RowId::new(),
            TimePoint::default(),
            VideoStream::descriptor_codec(),
            &VideoCodec::H264,
        )
        .expect("build codec chunk")
        .build()
        .expect("finalize codec chunk");
    store
        .insert_chunk(&Arc::new(codec_chunk))
        .expect("insert codec");

    let timeline_frame = Timeline::new_sequence("frame");
    for i in 0..num_samples {
        let bytes = (i as u32).to_le_bytes().to_vec();
        let sample = VideoSample::from(bytes);
        let chunk = Chunk::builder(entity_path.clone())
            .with_component(
                RowId::new(),
                TimePoint::from_iter([(timeline_frame, i)]),
                VideoStream::descriptor_sample(),
                &sample,
            )
            .expect("build sample chunk")
            .build()
            .expect("finalize sample chunk");
        store.insert_chunk(&Arc::new(chunk)).expect("insert sample");
    }

    store
}

fn indexed_keyframe_callback(period: i64) -> IsStartOfGop {
    Arc::new(move |data, _codec| {
        anyhow::ensure!(data.len() >= 4, "sample too short");
        let i = i64::from(u32::from_le_bytes([data[0], data[1], data[2], data[3]]));
        Ok(i % period == 0)
    })
}

fn indexed_options(period: i64, fix_keyframe: bool) -> CompactionOptions {
    CompactionOptions {
        config: ChunkStoreConfig::DEFAULT,
        num_extra_passes: Some(2),
        is_start_of_gop: Some(indexed_keyframe_callback(period)),
        split_size_ratio: None,
        fix_keyframe,
    }
}

/// Append a dedicated `VideoStream:is_keyframe` chunk for `entity`, with one
/// row per `(frame, value)` pair on the `frame` timeline.
fn insert_dedicated_keyframe_chunk(
    store: &mut ChunkStore,
    entity: &str,
    labels: impl IntoIterator<Item = (i64, bool)>,
) {
    let entity_path: EntityPath = entity.into();
    let timeline_frame = Timeline::new_sequence("frame");
    let mut builder = Chunk::builder(entity_path);
    for (frame, value) in labels {
        builder = builder
            .with_component(
                RowId::new(),
                TimePoint::from_iter([(timeline_frame, frame)]),
                VideoStream::descriptor_is_keyframe(),
                &IsKeyframe::from(value),
            )
            .expect("build keyframe row");
    }
    let chunk = builder.build().expect("finalize keyframe chunk");
    store
        .insert_chunk(&Arc::new(chunk))
        .expect("insert keyframe chunk");
}

/// Sum of `is_keyframe=true` rows across all chunks for any entity.
fn count_true_keyframes(store: &ChunkStore) -> u64 {
    let kf = VideoStream::descriptor_is_keyframe().component;
    store
        .iter_physical_chunks()
        .filter(|c| chunk_has_component(c, "VideoStream:is_keyframe"))
        .flat_map(|c| {
            c.iter_component::<IsKeyframe>(kf)
                .map(|v| v.as_slice().first().copied())
                .collect::<Vec<_>>()
        })
        .flatten()
        .filter(|kf| bool::from(kf.0))
        .count() as u64
}

/// Sum of `is_keyframe=false` rows across all chunks for any entity.
fn count_false_keyframes(store: &ChunkStore) -> u64 {
    let kf = VideoStream::descriptor_is_keyframe().component;
    store
        .iter_physical_chunks()
        .filter(|c| chunk_has_component(c, "VideoStream:is_keyframe"))
        .flat_map(|c| {
            c.iter_component::<IsKeyframe>(kf)
                .map(|v| v.as_slice().first().copied())
                .collect::<Vec<_>>()
        })
        .flatten()
        .filter(|kf| !bool::from(kf.0))
        .count() as u64
}

/// Run `f`, returning its result alongside every `WARN`+ log message emitted
/// while it ran (message plus its structured fields, rendered to a string).
///
/// Used to assert that a skipped entity is *reported* — we don't want optimize
/// to silently swallow a problem, only to not abort the whole recording over it.
fn with_captured_warnings<R>(f: impl FnOnce() -> R) -> (R, Vec<String>) {
    re_log::setup_logging();
    let rx = re_log::add_log_msg_receiver(re_log::LevelFilter::WARN);
    let result = f();
    let warnings = rx
        .try_iter()
        .filter(|msg| msg.level == re_log::Level::WARN)
        .map(|msg| {
            let fields = msg
                .fields
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(" ");
            format!("{} {fields}", msg.message)
        })
        .collect();
    (result, warnings)
}

/// Canonical input: user logged the codec's keyframes in a dedicated pure
/// chunk with no `false` rows. The keyframe chunk must be preserved by ID,
/// but the sample chunks must still get GoP-rebatched (those two concerns
/// are independent).
#[test]
fn optimize_preserves_canonical_keyframes_while_rebatching_samples() {
    let num_samples: i64 = 30;
    let period: i64 = 5;
    let mut store = indexed_video_store("/video", num_samples);
    insert_dedicated_keyframe_chunk(
        &mut store,
        "/video",
        (0..num_samples)
            .filter(|i| i % period == 0)
            .map(|i| (i, true)),
    );

    let kf_descriptor = VideoStream::descriptor_is_keyframe().component;
    let kf_chunk_ids_before: std::collections::BTreeSet<_> = store
        .iter_physical_chunks()
        .filter(|c| c.components().contains_component(kf_descriptor))
        .map(|c| c.id())
        .collect();
    assert_eq!(kf_chunk_ids_before.len(), 1, "test sanity");

    let num_sample_chunks_before = store
        .iter_physical_chunks()
        .filter(|c| chunk_has_component(c, "VideoStream:sample"))
        .count();

    let compacted = store
        .compacted(&indexed_options(period, false))
        .expect("canonical input must not error");

    let kf_chunk_ids_after: std::collections::BTreeSet<_> = compacted
        .iter_physical_chunks()
        .filter(|c| c.components().contains_component(kf_descriptor))
        .map(|c| c.id())
        .collect();
    assert_eq!(
        kf_chunk_ids_before, kf_chunk_ids_after,
        "canonical keyframe chunks must be preserved by ChunkId"
    );

    // Sample chunks were 30 single-row chunks going in; they must come out
    // GoP-aligned (≤ num_samples / period + 1 chunks).
    let num_sample_chunks_after = compacted
        .iter_physical_chunks()
        .filter(|c| chunk_has_component(c, "VideoStream:sample"))
        .count();
    assert!(
        num_sample_chunks_after < num_sample_chunks_before,
        "sample chunks must still be GoP-rebatched even when keyframe chunks \
         are canonical (before: {num_sample_chunks_before}, after: \
         {num_sample_chunks_after})"
    );

    assert_eq!(count_true_keyframes(&compacted), 6);
    assert_eq!(count_false_keyframes(&compacted), 0);
}

/// User-supplied labels whose `true`-set matches the codec, but with extra
/// `is_keyframe=false` rows on non-keyframes. No `false` should remain after a
/// successful optimize (the user must set `fix_keyframe` to drop them), so this
/// entity can't be rebatched. That must not abort the whole optimize: the entity
/// is skipped and left un-optimized, retaining the user's original labels.
#[test]
fn optimize_skips_entity_with_false_rows() {
    let num_samples: i64 = 20;
    let period: i64 = 5;
    let mut store = indexed_video_store("/video", num_samples);
    // Label every frame: `true` at codec keyframes, `false` elsewhere.
    insert_dedicated_keyframe_chunk(
        &mut store,
        "/video",
        (0..num_samples).map(|i| (i, i % period == 0)),
    );

    let (compacted, warnings) = with_captured_warnings(|| {
        store
            .compacted(&indexed_options(period, false))
            .expect("a single un-rebatchable entity must not fail the whole optimize")
    });

    // The skip must be reported, and the warning must still explain the problem
    // (the `false` rows) and how to handle it (`fix_keyframe`).
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("skipping GoP rebatching")
                && w.contains("is_keyframe=false")
                && w.contains("fix_keyframe")),
        "expected a warning explaining the skipped entity, got: {warnings:?}"
    );

    // The entity is left untouched: the user's labels survive verbatim, including
    // the `false` rows that prevented rebatching.
    let expected_true = (0..num_samples).filter(|i| i % period == 0).count() as u64;
    let expected_false = num_samples as u64 - expected_true;
    assert_eq!(count_true_keyframes(&compacted), expected_true);
    assert_eq!(count_false_keyframes(&compacted), expected_false);
}

/// Same setup as `optimize_errors_on_false_rows`, but with `fix_keyframe=true`
/// the user labels are discarded and a clean dedicated chunk is emitted.
#[test]
fn optimize_fix_keyframe_strips_false_rows() {
    let num_samples: i64 = 20;
    let period: i64 = 5;
    let mut store = indexed_video_store("/video", num_samples);
    insert_dedicated_keyframe_chunk(
        &mut store,
        "/video",
        (0..num_samples).map(|i| (i, i % period == 0)),
    );

    let compacted = store
        .compacted(&indexed_options(period, true))
        .expect("fix_keyframe must succeed despite `false` rows");

    assert_eq!(count_true_keyframes(&compacted), 4);
    assert_eq!(count_false_keyframes(&compacted), 0);
    // Rebuilt keyframe chunks must not co-locate with sample chunks.
    for chunk in compacted.iter_physical_chunks() {
        assert!(
            !(chunk_has_component(chunk, "VideoStream:is_keyframe")
                && chunk_has_component(chunk, "VideoStream:sample")),
        );
    }
}

/// User labels frame 1 (a non-keyframe) as `is_keyframe=true` *and* leaves
/// frames 10/15 (codec keyframes) unlabeled. The labels disagree with the codec,
/// so the entity can't be rebatched — but that must not abort the whole optimize.
/// The entity is skipped and left un-optimized, retaining the user's labels.
#[test]
fn optimize_skips_entity_with_mismatched_keyframe_labels() {
    let num_samples: i64 = 20;
    let period: i64 = 5;
    let mut store = indexed_video_store("/video", num_samples);
    // Codec keyframes are at frames 0, 5, 10, 15. We label 0 and 5 correctly,
    // 1 incorrectly, and skip 10 and 15.
    insert_dedicated_keyframe_chunk(&mut store, "/video", [(0, true), (1, true), (5, true)]);

    let (compacted, warnings) = with_captured_warnings(|| {
        store
            .compacted(&indexed_options(period, false))
            .expect("a single un-rebatchable entity must not fail the whole optimize")
    });

    // The skip must be reported, and the warning must still call out both the
    // missing labels (frames 10, 15) and the extra one (frame 1).
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("skipping GoP rebatching")
                && w.contains("missing")
                && w.contains("not codec keyframes")),
        "expected a warning explaining the skipped entity, got: {warnings:?}"
    );

    // The entity is left untouched: the user's three `true` labels survive, and
    // no `false` rows are invented.
    assert_eq!(count_true_keyframes(&compacted), 3);
    assert_eq!(count_false_keyframes(&compacted), 0);
}

/// With `fix_keyframe`, optimize ignores user labels and re-derives from the
/// codec. Wrong labels are no longer fatal.
#[test]
fn optimize_fix_keyframe_overrides_user_labels() {
    let num_samples: i64 = 10;
    let period: i64 = 5;
    let mut store = indexed_video_store("/video", num_samples);
    // Wrong labels — same setup as `optimize_errors_on_incorrect_keyframes`.
    insert_dedicated_keyframe_chunk(&mut store, "/video", [(0, true), (1, true)]);

    let compacted = store
        .compacted(&indexed_options(period, true))
        .expect("fix_keyframe must succeed despite wrong labels");

    // Codec says 2 keyframes (frames 0 and 5). User's bogus "frame 1" must
    // not appear in the output.
    assert_eq!(count_true_keyframes(&compacted), 2);
    assert_eq!(count_false_keyframes(&compacted), 0);
}

/// `is_keyframe` rows can share a chunk with something *other than* sample —
/// e.g., another archetype's component on the same entity. The labels happen
/// to match the codec, but the layout isn't canonical, so optimize must
/// rebuild a dedicated chunk rather than treating the input as already-fine.
#[test]
fn optimize_breaks_out_keyframes_sharing_chunk_with_other_components() {
    let num_samples: i64 = 10;
    let period: i64 = 5;
    let entity_path: EntityPath = "/video".into();
    let timeline = Timeline::new_sequence("frame");

    let mut store = indexed_video_store("/video", num_samples);

    // Same row carries is_keyframe AND an unrelated component. No sample
    // co-location, but also not a pure keyframe chunk — this is the case my
    // earlier `co_located` check missed.
    let kf = IsKeyframe::from(true);
    let point = MyPoint::new(0.0, 0.0);
    let mixed_chunk = Chunk::builder(entity_path.clone())
        .with_component_batches(
            RowId::new(),
            TimePoint::from_iter([(timeline, 0_i64)]),
            [
                (
                    VideoStream::descriptor_is_keyframe(),
                    &kf as &dyn re_types_core::ComponentBatch,
                ),
                (
                    MyPoints::descriptor_points(),
                    &[point] as &dyn re_types_core::ComponentBatch,
                ),
            ],
        )
        .build()
        .expect("mixed chunk builds");
    let kf2 = IsKeyframe::from(true);
    let mixed_chunk2 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            RowId::new(),
            TimePoint::from_iter([(timeline, 5_i64)]),
            [(
                VideoStream::descriptor_is_keyframe(),
                &kf2 as &dyn re_types_core::ComponentBatch,
            )],
        )
        .build()
        .expect("solo kf chunk builds");
    let mixed_chunk_id = mixed_chunk.id();
    store
        .insert_chunk(&Arc::new(mixed_chunk))
        .expect("insert mixed chunk");
    store
        .insert_chunk(&Arc::new(mixed_chunk2))
        .expect("insert solo kf chunk");

    let compacted = store
        .compacted(&indexed_options(period, false))
        .expect("correct labels in a non-pure chunk must not error");

    // The original mixed chunk must be gone (Skip would have left it).
    let mixed_survived = compacted
        .iter_physical_chunks()
        .any(|c| c.id() == mixed_chunk_id);
    assert!(
        !mixed_survived,
        "non-pure keyframe chunk must be rebuilt, not skipped"
    );

    // Every is_keyframe row in the output lives in a chunk that holds nothing
    // else.
    for chunk in compacted.iter_physical_chunks() {
        if !chunk_has_component(chunk, "VideoStream:is_keyframe") {
            continue;
        }
        let only_keyframe = chunk
            .components()
            .values()
            .all(|c| c.descriptor.component.as_str() == "VideoStream:is_keyframe");
        assert!(
            only_keyframe,
            "rebuilt keyframe chunk must hold only is_keyframe, got: {:?}",
            chunk
                .components()
                .values()
                .map(|c| c.descriptor.component.as_str())
                .collect::<Vec<_>>()
        );
    }

    assert_eq!(count_true_keyframes(&compacted), 2);
    assert_eq!(count_false_keyframes(&compacted), 0);
}

/// A user that logs `is_keyframe=true` sparsely (only on codec keyframes) but
/// co-located in the sample chunks: optimize must move the keyframe column
/// out of the sample chunks and into a dedicated marker chunk.
#[test]
fn optimize_strips_user_supplied_is_keyframe_from_sample_chunks() {
    let num_samples: i64 = 30;
    let period: i64 = 5;
    let entity_path: EntityPath = "/video".into();
    let timeline = Timeline::new_sequence("frame");

    let store_id = re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "video_test");
    let mut store = ChunkStore::new(store_id, ChunkStoreConfig::ALL_DISABLED);

    let codec_chunk = Chunk::builder(entity_path.clone())
        .with_component(
            RowId::new(),
            TimePoint::default(),
            VideoStream::descriptor_codec(),
            &VideoCodec::H264,
        )
        .unwrap()
        .build()
        .unwrap();
    store.insert_chunk(&Arc::new(codec_chunk)).unwrap();

    // Per-frame chunks: every frame logs `sample`; codec keyframes additionally
    // log `is_keyframe=true` in the same chunk. The latter is what we expect
    // optimize to split out into a dedicated chunk.
    for i in 0..num_samples {
        let bytes = (i as u32).to_le_bytes().to_vec();
        let mut stream = VideoStream::update_fields().with_sample(VideoSample::from(bytes));
        if i % period == 0 {
            stream = stream.with_is_keyframe(IsKeyframe::from(true));
        }
        let chunk = Chunk::builder(entity_path.clone())
            .with_archetype(RowId::new(), TimePoint::from_iter([(timeline, i)]), &stream)
            .build()
            .unwrap();
        store.insert_chunk(&Arc::new(chunk)).unwrap();
    }

    let compacted = store
        .compacted(&indexed_options(period, false))
        .expect("sparse correct co-located labels must not error");

    for chunk in compacted.iter_physical_chunks() {
        let has_sample = chunk_has_component(chunk, "VideoStream:sample");
        let has_kf = chunk_has_component(chunk, "VideoStream:is_keyframe");
        assert!(
            !(has_sample && has_kf),
            "VideoStream:sample and VideoStream:is_keyframe must not co-locate: {:?}",
            chunk
                .components()
                .values()
                .map(|c| c.descriptor.component.as_str())
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(count_true_keyframes(&compacted), 6);
    assert_eq!(count_false_keyframes(&compacted), 0);
}
