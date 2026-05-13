use std::{iter::once, sync::Arc};

use re_chunk::Chunk;
use re_entity_db::EntityDb;
use re_log_types::StoreId;
use re_sdk_types::archetypes::VideoStream;

use crate::VideoStreamCache;

use super::{
    STREAM_ENTITY, TIMELINE_NAME, TestVideoPlayer, assert_loading, assert_splits_happened,
    codec_chunk, load_chunks, load_into_rrd_manifest, playable_stream, unload_chunks, video_chunk,
};

#[test]
fn cache_with_manifest() {
    let mut cache = VideoStreamCache::default();

    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let chunks: Vec<_> = (0..10)
        .map(|i| video_chunk(i as f64, 0.25, 1, 4))
        .chain(once(codec_chunk()))
        .map(Arc::new)
        .collect();

    load_into_rrd_manifest(&mut store, &chunks);

    // load codec chunk
    load_chunks(&mut store, &mut cache, &chunks[chunks.len() - 1..]);

    let video_stream = playable_stream(&mut cache, &store);

    // Load some chunks.
    load_chunks(&mut store, &mut cache, &chunks[4..5]);

    let mut player = TestVideoPlayer::from_stream(video_stream);

    assert_loading(player.play_store(6.0..10.0, 0.25, &store));
    player.expect_decoded_samples(None);

    player.play_store(4.0..4.75, 0.25, &store).unwrap();

    player.expect_decoded_samples(16..19);

    load_chunks(&mut store, &mut cache, &chunks[0..2]);

    player.play_store(0.0..1.75, 0.25, &store).unwrap();

    load_chunks(&mut store, &mut cache, &chunks[2..4]);

    player.play_store(1.75..4.75, 0.25, &store).unwrap();

    player.expect_decoded_samples(0..19);

    unload_chunks(&store, &mut cache, 4.0..5.0);

    load_chunks(&mut store, &mut cache, &chunks[4..7]);

    player.play_store(4.75..6.75, 0.25, &store).unwrap();

    player.expect_decoded_samples(20..27);

    // Load the ones we unloaded again
    load_chunks(&mut store, &mut cache, &chunks[0..4]);

    player.play_store(0.0..6.75, 0.25, &store).unwrap();

    player.expect_decoded_samples(0..27);
}

#[test]
fn cache_with_streaming() {
    let mut cache = VideoStreamCache::default();

    let mut store = EntityDb::with_store_config(
        StoreId::recording("test", "test"),
        true,
        re_chunk_store::ChunkStoreConfig {
            enable_changelog: true,
            chunk_max_bytes: u64::MAX,
            chunk_max_rows: 12,
            chunk_max_rows_if_unsorted: 12,
        },
    );

    let chunk_count = 100;

    let dt = 0.25;
    let chunks: Vec<_> = (0..chunk_count)
        .map(|i| video_chunk(i as f64, dt, 1, 4))
        .chain(once(codec_chunk()))
        .map(Arc::new)
        .collect();

    // load codec chunk
    load_chunks(&mut store, &mut cache, &chunks[chunks.len() - 1..]);

    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream);

    // Load all sample chunks.
    load_chunks(&mut store, &mut cache, &chunks[0..chunk_count]);

    player.play_store(0.0..25.0, dt, &store).unwrap();

    player.expect_decoded_samples(0..chunk_count);

    unload_chunks(&store, &mut cache, 15.0..25.0);

    // Try dropping chunks at the start.
    player.play_store(15.0..25.0, dt, &store).unwrap();

    player.expect_decoded_samples(60..chunk_count);
}

#[test]
fn cache_with_manifest_and_streaming() {
    let mut cache = VideoStreamCache::default();

    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let chunks: Vec<_> = once(codec_chunk())
        .chain((0..6).map(|i| video_chunk(i as f64 + 1.0, 0.25, 1, 4)))
        .map(Arc::new)
        .collect();

    // Load first 5 chunks into the manifest.
    load_into_rrd_manifest(&mut store, &chunks[..5]);

    // load codec chunk
    load_chunks(&mut store, &mut cache, &chunks[..1]);

    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream);

    // Load some chunks.
    load_chunks(&mut store, &mut cache, &chunks[3..5]);

    assert_loading(player.play_store(1.0..3.0, 0.25, &store));
    player.expect_decoded_samples(None);

    player.play_store(3.0..5.0, 0.25, &store).unwrap();
    player.expect_decoded_samples(8..16);

    load_chunks(&mut store, &mut cache, &chunks[5..6]);
    player.play_store(5.0..6.0, 0.25, &store).unwrap();
    player.expect_decoded_samples(16..20);

    load_chunks(&mut store, &mut cache, &chunks[6..7]);
    player.play_store(6.0..7.0, 0.25, &store).unwrap();
    player.expect_decoded_samples(20..24);

    player.play_store(3.0..7.0, 0.25, &store).unwrap();
    player.expect_decoded_samples(8..24);

    load_chunks(&mut store, &mut cache, &chunks[1..3]);
    player.play_store(1.0..7.0, 0.25, &store).unwrap();
    player.expect_decoded_samples(0..24);

    unload_chunks(&store, &mut cache, 4.0..6.0);
    // Check that all remaining samples are still playable.
    player.play_store(4.0..6.0, 0.25, &store).unwrap();
    player.expect_decoded_samples(12..20);
}

#[test]
fn cache_with_streaming_splits() {
    let mut cache = VideoStreamCache::default();

    let mut store = EntityDb::with_store_config(
        StoreId::recording("test", "test"),
        true,
        re_chunk_store::ChunkStoreConfig {
            enable_changelog: true,
            chunk_max_bytes: u64::MAX,
            chunk_max_rows: 100,
            chunk_max_rows_if_unsorted: 100,
        },
    );

    let chunk_count = 4;
    let gops_per_chunk = 10;
    let samples_per_gop = 200;

    let dt = 0.1;

    let samples_per_chunk = gops_per_chunk * samples_per_gop;
    let sample_count = chunk_count * samples_per_chunk;
    let time_per_chunk = samples_per_chunk as f64 * dt;

    let chunks: Vec<_> = (0..chunk_count)
        .map(|i| {
            video_chunk(
                i as f64 * time_per_chunk,
                dt,
                gops_per_chunk,
                samples_per_gop,
            )
        })
        .chain(once(codec_chunk()))
        .map(Arc::new)
        .collect();

    // load codec chunk
    load_chunks(&mut store, &mut cache, &chunks[chunks.len() - 1..]);

    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream);

    // Load all sample chunks.
    load_chunks(&mut store, &mut cache, &chunks[0..4]);

    player
        .play_store(0.0..sample_count as f64 * dt, dt, &store)
        .unwrap();

    player.expect_decoded_samples(0..sample_count as re_video::SampleIndex);

    assert_splits_happened(&store);
}

#[test]
fn cache_with_manifest_splits() {
    let mut cache = VideoStreamCache::default();

    let mut store = EntityDb::with_store_config(
        StoreId::recording("test", "test"),
        true,
        re_chunk_store::ChunkStoreConfig {
            enable_changelog: true,
            chunk_max_bytes: u64::MAX,
            chunk_max_rows: 100,
            chunk_max_rows_if_unsorted: 100,
        },
    );

    let chunk_count = 4;
    let gops_per_chunk = 10;
    let samples_per_gop = 200;

    let dt = 0.1;
    let samples_per_chunk = gops_per_chunk * samples_per_gop;
    let time_per_chunk = samples_per_chunk as f64 * dt;

    let chunks: Vec<_> = (0..chunk_count)
        .map(|i| {
            video_chunk(
                time_per_chunk * i as f64,
                dt,
                gops_per_chunk,
                samples_per_gop,
            )
        })
        .chain(once(codec_chunk()))
        .map(Arc::new)
        .collect();

    load_into_rrd_manifest(&mut store, &chunks);

    // load codec chunk
    load_chunks(&mut store, &mut cache, &chunks[chunks.len() - 1..]);

    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream);

    load_chunks(&mut store, &mut cache, &chunks[1..2]);

    player
        .play_store(time_per_chunk..time_per_chunk * 2.0 - dt, dt, &store)
        .unwrap();

    let samples_per_chunk = samples_per_chunk as usize;
    player.expect_decoded_samples(samples_per_chunk..samples_per_chunk * 2 - 1);

    load_chunks(&mut store, &mut cache, &chunks[2..3]);
    player
        .play_store(time_per_chunk * 2.0..time_per_chunk * 3.0 - dt, dt, &store)
        .unwrap();

    player.expect_decoded_samples(samples_per_chunk * 2..samples_per_chunk * 3 - 1);

    let min_loaded = 1.7;
    let max_loaded = 2.3;

    unload_chunks(
        &store,
        &mut cache,
        time_per_chunk * min_loaded..time_per_chunk * max_loaded,
    );

    // Assert that the beginning/end splits have been gc'd
    assert_loading(player.play_store(time_per_chunk..time_per_chunk * 1.5, dt, &store));
    player.expect_decoded_samples(None);

    let play_store = player.play_store(time_per_chunk * 2.5..time_per_chunk * 3.0 - dt, dt, &store);
    player.expect_decoded_samples(None);
    assert_loading(play_store);

    player
        .play_store(
            time_per_chunk * min_loaded..time_per_chunk * max_loaded - dt,
            dt,
            &store,
        )
        .unwrap();

    let end = (samples_per_chunk as f64 * max_loaded) as usize;
    player.expect_decoded_samples((samples_per_chunk as f64 * min_loaded).ceil() as usize..end);

    load_chunks(&mut store, &mut cache, &chunks[0..2]);
    player
        .play_store(0.0..time_per_chunk * max_loaded - dt, dt, &store)
        .unwrap();

    player.expect_decoded_samples(0..end);

    assert_splits_happened(&store);
}

#[test]
fn cache_with_unordered_chunks() {
    use re_chunk::{RowId, TimeInt, Timeline};
    use re_video::AV1_TEST_INTER_FRAME;
    use re_video::AV1_TEST_KEYFRAME;

    let mut cache = VideoStreamCache::default();

    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let chunk_count = 100;

    let gop_count = 1;
    let samples_per_gop = 4;

    let dt = 0.25;
    let chunks: Vec<_> = (0..chunk_count)
        .map(|i| {
            let timeline = Timeline::new_duration(TIMELINE_NAME);
            let mut builder = Chunk::builder(STREAM_ENTITY);
            let mut row_ids: Vec<_> = (0..gop_count * samples_per_gop)
                .map(|_| RowId::new())
                .collect();

            use rand::SeedableRng as _;
            use rand::seq::SliceRandom as _;
            let mut rng = rand::rngs::StdRng::seed_from_u64(i as u64);

            // Shuffle row ids to make the chunk (very likely) unsorted on the timeline.
            row_ids.shuffle(&mut rng);

            let start_time = i as f64;
            for i in 0..gop_count {
                let gop_start_time = start_time + (i * samples_per_gop) as f64 * dt;

                builder = builder.with_archetype(
                    row_ids.pop().unwrap(),
                    [(timeline, TimeInt::from_secs(gop_start_time))],
                    &VideoStream::update_fields().with_sample(AV1_TEST_KEYFRAME),
                );

                for i in 1..samples_per_gop {
                    let time = gop_start_time + i as f64 * dt;
                    builder = builder.with_archetype(
                        row_ids.pop().unwrap(),
                        [(timeline, TimeInt::from_secs(time))],
                        &VideoStream::update_fields().with_sample(AV1_TEST_INTER_FRAME),
                    );
                }
            }

            let mut chunk = builder.build().unwrap();

            chunk.sort_if_unsorted();

            chunk
        })
        .chain(once(codec_chunk()))
        .map(Arc::new)
        .collect();

    assert!(
        chunks.iter().any(|chunk| {
            chunk
                .timelines()
                .get(&re_chunk::TimelineName::new(TIMELINE_NAME))
                .is_some_and(|t| !t.is_sorted())
        }),
        "We are testing unsorted chunks, at least one should end up unsorted"
    );

    // load codec chunk
    load_chunks(&mut store, &mut cache, &chunks[chunks.len() - 1..]);

    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream);

    // Load all sample chunks.
    load_chunks(&mut store, &mut cache, &chunks[0..chunk_count]);

    player.play_store(0.0..25.0, dt, &store).unwrap();

    player.expect_decoded_samples(0..chunk_count);
}

/// Loads chunks in non-chronological order so that a later-arriving chunk
/// has timestamps that fall before existing samples, triggering
/// out-of-order detection and re-merge.
#[test]
fn cache_with_out_of_order_chunk_arrival() {
    let mut cache = VideoStreamCache::default();

    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let dt = 0.25;
    let samples_per_gop = 4;

    // 10 chunks, each 1 GOP of 4 samples.
    let chunk_count = 10usize;
    let chunks: Vec<_> = (0..chunk_count)
        .map(|i| video_chunk(i as f64, dt, 1, samples_per_gop))
        .chain(once(codec_chunk()))
        .map(Arc::new)
        .collect();

    // Load codec chunk and create the cache entry.
    load_chunks(&mut store, &mut cache, &chunks[chunks.len() - 1..]);
    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream);

    // Load chunks 0, 1, 2 in order.
    load_chunks(&mut store, &mut cache, &chunks[0..3]);

    player.play_store(0.0..3.0, dt, &store).unwrap();
    player.expect_decoded_samples(0..12);

    // Skip chunk 3 and load chunk 4 first, still in order relative to
    // what was already loaded.
    load_chunks(&mut store, &mut cache, &chunks[4..5]);

    player.play_store(4.0..5.0, dt, &store).unwrap();
    player.expect_decoded_samples(12..16);

    // Now load chunk 3 which has times [3.0, 3.25, 3.5, 3.75] -- this
    // falls between the already-loaded chunks 2 and 4, triggering the
    // out-of-order / delta re-merge path.
    load_chunks(&mut store, &mut cache, &chunks[3..4]);

    // The cache entry should still exist (delta re-merge, not removal).
    assert!(
        cache
            .entries
            .contains_key(&crate::cache::video_stream_cache::VideoStreamKey {
                entity_path: re_chunk::EntityPath::from(STREAM_ENTITY).hash(),
                timeline: re_chunk::TimelineName::new(TIMELINE_NAME),
                sample_component: VideoStream::descriptor_sample().component,
            }),
        "Cache entry should survive delta re-merge"
    );

    // All 20 samples (chunks 0-4) should be playable.
    player.play_store(0.0..5.0, dt, &store).unwrap();
    player.expect_decoded_samples(0..20);

    // Load chunks 7, 8, 9 (skipping 5, 6).
    load_chunks(&mut store, &mut cache, &chunks[7..10]);

    player.play_store(7.0..10.0, dt, &store).unwrap();
    player.expect_decoded_samples(20..32);

    // Now load the skipped chunks 5 and 6 out of order.
    load_chunks(&mut store, &mut cache, &chunks[5..7]);

    // Everything from 0 through 10 should work.
    player.play_store(0.0..10.0, dt, &store).unwrap();
    player.expect_decoded_samples(0..40);
}

/// Out-of-order chunk arrival followed by compaction, where a
/// `ChunkSampleRange` has less samples than the amount of samples it spans.
#[test]
fn cache_out_of_order_arrival_with_compaction() {
    let mut cache = VideoStreamCache::default();

    let mut store = EntityDb::with_store_config(
        StoreId::recording("test", "test"),
        true,
        re_chunk_store::ChunkStoreConfig {
            enable_changelog: true,
            chunk_max_bytes: u64::MAX,
            chunk_max_rows: 4,
            chunk_max_rows_if_unsorted: 4,
        },
    );

    let codec_chunk = Arc::new(codec_chunk());

    // Create chunk0 with 4 rows so it won't compact.
    let chunk0 = Arc::new(video_chunk(0.0, 2.0, 1, 4)); // times: 0.0, 2.0, 4.0, 6.0

    // Create chunk1 and chunk2 with less than 4 rows combined so they compact.
    let chunk1 = Arc::new(video_chunk(5.0, 2.0, 1, 2)); // times: 5.0, 7.0
    let chunk2 = Arc::new(video_chunk(8.0, 0.0, 1, 1)); // time: 8.0

    let codec_chunk_id = codec_chunk.id();
    let chunk0_id = chunk0.id();
    let chunk1_id = chunk1.id();
    let chunk2_id = chunk2.id();

    let replace_id = |s: &str| -> String {
        s.replace(
            &codec_chunk_id.to_string(),
            &format!("chunk_codec {}", codec_chunk_id.short_string()),
        )
        .replace(
            &chunk0_id.to_string(),
            &format!("chunk0 {}", chunk0_id.short_string()),
        )
        .replace(
            &chunk1_id.to_string(),
            &format!("chunk1 {}", chunk1_id.short_string()),
        )
        .replace(
            &chunk2_id.to_string(),
            &format!("chunk2 {}", chunk2_id.short_string()),
        )
    };

    // Load codec chunk and chunk0.
    load_chunks(&mut store, &mut cache, &[codec_chunk, chunk0]);

    let video_stream_before = playable_stream(&mut cache, &store);

    let mut player = TestVideoPlayer::from_stream(video_stream_before);

    player.play_store(0.0..8.0, 1.0, &store).unwrap();
    player.expect_decoded_samples(0..4);

    // This triggers out-of-order handling because time 5 < time 6.
    // With delta re-merge, the cache entry is NOT cleared.
    load_chunks(&mut store, &mut cache, &[chunk1]);

    assert!(
        store
            .storage_engine()
            .store()
            .iter_physical_chunks()
            .zip([Some(codec_chunk_id), Some(chunk0_id), Some(chunk1_id), None])
            .all(|(c, expected_id)| {
                let eq = Some(c.id()) == expected_id;

                if !eq {
                    eprintln!(
                        "Expected {}, got {} with lineage:\n{}",
                        expected_id
                            .map(|c| c.short_string())
                            .unwrap_or_else(|| "nothing".to_owned()),
                        c.id().short_string(),
                        replace_id(&store.storage_engine().store().format_lineage(&c.id())),
                    );
                }

                eq
            }),
        "No compaction should've occurred yet"
    );

    // The cache entry should still exist (delta re-merge instead of removal).
    assert!(
        cache
            .entries
            .contains_key(&crate::cache::video_stream_cache::VideoStreamKey {
                entity_path: re_chunk::EntityPath::from(STREAM_ENTITY).hash(),
                timeline: re_chunk::TimelineName::new(TIMELINE_NAME),
                sample_component: VideoStream::descriptor_sample().component,
            }),
        "The video stream cache entry should still exist after delta re-merge"
    );

    // Use the same video stream -- it was re-merged in place.
    let video_stream_after = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream_after);

    player.play_store(0.0..8.0, 1.0, &store).unwrap();
    player.expect_decoded_samples(0..6);

    // This should compact with chunk1.
    load_chunks(&mut store, &mut cache, &[chunk2]);

    assert!(
        store
            .storage_engine()
            .store()
            .iter_physical_chunks()
            .any(|c| {
                if let Some(re_chunk_store::ChunkDirectLineage::CompactedFrom(chunks)) =
                    store.storage_engine().store().direct_lineage(&c.id())
                {
                    *chunks == [chunk1_id, chunk2_id].into_iter().collect()
                } else {
                    false
                }
            }),
        "chunk 1 & 2, should've been compacted.\nchunks:\n{}",
        replace_id(
            &store
                .storage_engine()
                .store()
                .iter_physical_chunks()
                .map(|c| store.storage_engine().store().format_lineage(&c.id()))
                .collect::<Vec<_>>()
                .join("\n\n")
        ),
    );

    player.play_store(0.0..9.0, 1.0, &store).unwrap();

    player.expect_decoded_samples(0..7);
}

/// When manifest-placed samples are loaded, `from_root` places all unloaded
/// samples at the start of the chunk's time range. A multi-GOP chunk whose
/// second GOP falls after another chunk's samples causes out-of-order
/// detection and re-merge.
#[test]
fn cache_with_manifest_load_resulting_in_incomplete_gop() {
    let mut cache = VideoStreamCache::default();
    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let dt = 0.25;

    // Chunk A: 2 GOPs of 4 samples each, times [0, 1.75].
    let chunk_a = video_chunk(0.0, dt, 2, 4);

    // Chunk B: 1 GOP of 3 samples, times [0.875, 1.375].
    let chunk_b = video_chunk(0.875, dt, 1, 3);

    let chunks: Vec<_> = [chunk_a, chunk_b, codec_chunk()]
        .into_iter()
        .map(Arc::new)
        .collect();

    load_into_rrd_manifest(&mut store, &chunks);

    // Load codec.
    load_chunks(&mut store, &mut cache, &chunks[2..3]);

    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream);

    // Load chunk B first.
    load_chunks(&mut store, &mut cache, &chunks[1..2]);

    player.play_store(0.875..1.375, dt, &store).unwrap();
    player.expect_decoded_samples(8..11);

    // Playing in A's unloaded range should fail with loading.
    assert_loading(player.play_store(0.0..0.75, dt, &store));
    player.expect_decoded_samples(None);

    // Load chunk A. from_root placed A's 8 samples at time 0 (indices 0-7).
    load_chunks(&mut store, &mut cache, &chunks[0..1]);

    // The cache entry should survive the delta re-merge.
    assert!(
        cache
            .entries
            .contains_key(&crate::cache::video_stream_cache::VideoStreamKey {
                entity_path: re_chunk::EntityPath::from(STREAM_ENTITY).hash(),
                timeline: re_chunk::TimelineName::new(TIMELINE_NAME),
                sample_component: VideoStream::descriptor_sample().component,
            }),
        "Cache entry should survive delta re-merge"
    );

    // After re-merge, all 11 samples should be in the correct time order.
    player.play_store(0.0..2.0, dt, &store).unwrap();
    player.expect_decoded_samples(0..11);
}

/// When a conflicting chunk is loaded last, its manifest-placed keyframe
/// may sit right before the affected range. The reorder logic must walk
/// back past that keyframe to include earlier samples whose real
/// timestamps interleave with the conflicting chunk.
#[test]
fn cache_with_manifest_skips_conflicting_chunk_keyframe() {
    let mut cache = VideoStreamCache::default();
    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    // Chunk 0: 1 GOP of 2 samples, times [1, 3].
    let chunk_0 = video_chunk(1.0, 2.0, 1, 2);

    // Chunk 1: 1 GOP of 2 samples, times [2, 4].
    let chunk_1 = video_chunk(2.0, 2.0, 1, 2);

    // Chunk 2: 1 GOP of 2 samples, times [5, 6].
    let chunk_2 = video_chunk(5.0, 1.0, 1, 2);

    let chunks: Vec<_> = [chunk_0, chunk_1, chunk_2, codec_chunk()]
        .into_iter()
        .map(Arc::new)
        .collect();

    load_into_rrd_manifest(&mut store, &chunks);

    // Load codec.
    load_chunks(&mut store, &mut cache, &chunks[3..4]);

    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream);

    // Load chunk 0, then chunk 2.
    load_chunks(&mut store, &mut cache, &chunks[0..1]);
    load_chunks(&mut store, &mut cache, &chunks[2..3]);

    player.play_store(5.0..7.0, 1.0, &store).unwrap();
    player.expect_decoded_samples(4..6);

    // Loading chunk 1 triggers out-of-order.
    load_chunks(&mut store, &mut cache, &chunks[1..2]);

    // The cache entry should survive the delta re-merge.
    assert!(
        cache
            .entries
            .contains_key(&crate::cache::video_stream_cache::VideoStreamKey {
                entity_path: re_chunk::EntityPath::from(STREAM_ENTITY).hash(),
                timeline: re_chunk::TimelineName::new(TIMELINE_NAME),
                sample_component: VideoStream::descriptor_sample().component,
            }),
        "Cache entry should survive delta re-merge"
    );

    // After re-merge, all 6 samples should be in the correct time order.
    player.play_store(1.0..7.0, 1.0, &store).unwrap();
    player.expect_decoded_samples(0..6);
}

/// Interleaved chunk arrival followed by GC of the interleaving chunk.
/// The remaining chunk's samples must still be decodable from its own keyframe.
#[test]
fn cache_with_gc_after_interleaved_arrival() {
    let mut cache = VideoStreamCache::default();
    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let codec = Arc::new(codec_chunk());
    let chunk_y = Arc::new(video_chunk(1.0, 1.0, 1, 4));
    // Starts before Y but loaded second, triggering out-of-order handling.
    let chunk_x = Arc::new(video_chunk(0.5, 1.0, 2, 1));

    load_chunks(&mut store, &mut cache, std::slice::from_ref(&codec));
    load_chunks(&mut store, &mut cache, std::slice::from_ref(&chunk_y));

    // Create a live entry before chunk_x arrives, so handle_deletion runs on it later.
    let _ = playable_stream(&mut cache, &store);

    // Loading X after Y triggers handle_out_of_order_chunk, interleaving the deques.
    load_chunks(&mut store, &mut cache, std::slice::from_ref(&chunk_x));

    // Evict X and the codec while keeping Y.
    unload_chunks(&store, &mut cache, 2.0..5.0);

    // Reload the codec.
    load_chunks(&mut store, &mut cache, std::slice::from_ref(&codec));

    // The entry was either correctly rebuilt or corrupted.
    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream);

    player.play_store(2.0..5.0, 0.25, &store).unwrap();
    player.expect_decoded_samples(0..4);
}
