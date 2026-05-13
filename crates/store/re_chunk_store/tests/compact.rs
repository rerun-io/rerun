//! Tests for `compacted()` and `finalize_compaction`.

#![cfg(test)]

use std::sync::Arc;

use re_chunk::{Chunk, RowId};
use re_chunk_store::{ChunkStore, ChunkStoreConfig, CompactionOptions, IsStartOfGop};
use re_log_types::example_components::{MyPoint, MyPoints};
use re_log_types::{EntityPath, TimePoint, Timeline};
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components::{VideoCodec, VideoSample};

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
    }
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
