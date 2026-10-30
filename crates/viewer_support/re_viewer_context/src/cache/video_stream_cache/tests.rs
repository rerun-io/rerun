#![expect(clippy::cast_possible_wrap)] // u64 -> i64 is fine
use std::assert_matches;

use re_chunk::{ChunkBuilder, RowId, TimePoint, Timeline};
use re_chunk_store::ChunkStoreDiff;
use re_log_types::StoreId;
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components::VideoCodec;
use re_video::{VideoDataDescription, VideoEncodingDetails};

use super::*;

// Generated using:
// ffmpeg -i 'rerun-io/internal-test-assets/video/gif_conversion_color_issues_h264.mp4' -c:v libx264 -pix_fmt yuv420p -g 10 -vf scale=iw/2:ih/2 -bf 0 gif_as_h264_nobframes.mp4
const RAW_H264_DATA: &[u8] =
    include_bytes!("../../../../../../tests/assets/video/gif_as_h264_nobframes.h264");

const GRINDA_JPEG: &[u8] = include_bytes!("../../../../../../tests/assets/image/grinda.jpg");

const NUM_FRAMES: usize = 44;

/// Iter h264 frames of test data.
/// Makes a bunch of assumptions on the test data. DO NOT USE THIS IN PRODUCTION.
fn iter_h264_frames(data: &[u8]) -> impl Iterator<Item = &[u8]> {
    // Don't want to depend on `h264_reader`` here, so quickly whip this up by hand!
    // Assumes Annex-B format with 0x00000001 start codes
    let mut pos = 0;
    std::iter::from_fn(move || {
        if pos >= data.len() {
            return None;
        }
        let start = pos;

        // Skip over current start code.
        pos += 4;

        // Find next start code
        while pos < data.len() {
            if pos + 4 < data.len() && data[pos..pos + 4] == [0, 0, 0, 1] {
                // Check NAL type (first byte after start code)
                let nal_type = data[pos + 4] & 0x1F;

                // Cut a frame if the nal type is 1 (regular frame) or 7 (SPS, expected to be followed by a keyframe).
                if nal_type == 1 || nal_type == 7 {
                    return Some(&data[start..pos]);
                }
            }
            pos += 1;
        }

        Some(&data[start..])
    })
}

fn validate_stream_from_test_data(video_stream: &PlayableVideoStream, num_frames_submitted: usize) {
    let data_descr = video_stream.video_renderer.data_descr();
    data_descr.sanity_check().unwrap();

    let VideoDataDescription {
        codec,
        encoding_details,
        timescale,
        delivery_method,
        keyframe_indices,
        samples,
        samples_statistics,
        mp4_tracks,
    } = data_descr.clone();

    assert_eq!(codec, re_video::VideoCodec::H264);
    assert_eq!(timescale, None); // Sequence timeline doesn't have a timescale.
    assert_matches!(
        delivery_method,
        re_video::VideoDeliveryMethod::Stream { .. }
    );
    assert!(samples_statistics.dts_always_equal_pts);
    assert!(samples_statistics.has_sample_highest_pts_so_far.is_none());
    assert!(mp4_tracks.is_empty());

    let VideoEncodingDetails {
        codec_string,
        coded_dimensions,
        bit_depth,
        chroma_subsampling,
        max_num_reorder_frames,
        stsd,
    } = encoding_details.unwrap();
    assert_eq!(codec_string, "avc1.64000A");
    assert_eq!(coded_dimensions, [110, 82]);
    assert_eq!(bit_depth, Some(8));
    assert_eq!(
        chroma_subsampling,
        Some(re_video::ChromaSubsamplingModes::Yuv420)
    );
    assert_eq!(max_num_reorder_frames, Some(0));
    assert_eq!(stsd, None);

    assert_eq!(samples.num_elements(), num_frames_submitted);

    // The GOPs in the sample data have a fixed size of 10.
    assert_eq!(keyframe_indices[0], 0);
    if num_frames_submitted > 10 {
        assert_eq!(keyframe_indices[1], 10);
    }
    if num_frames_submitted > 20 {
        assert_eq!(keyframe_indices[2], 20);
    }
    if num_frames_submitted > 30 {
        assert_eq!(keyframe_indices[3], 30);
    }
    if num_frames_submitted > 40 {
        assert_eq!(keyframe_indices[4], 40);
    }
}

/// Check that the samples point at the given rows, in the given order.
fn assert_sample_row_ids(video_stream: &PlayableVideoStream, expected_row_ids: &[RowId]) {
    let row_ids = video_stream
        .video_descr()
        .samples
        .iter()
        .map(sample_row_id)
        .collect::<Vec<_>>();
    assert_eq!(
        row_ids,
        expected_row_ids
            .iter()
            .copied()
            .map(Some)
            .collect::<Vec<_>>()
    );
}

#[test]
fn video_stream_cache_from_single_chunk() {
    let mut cache = VideoStreamCache::default();
    let mut store = re_entity_db::EntityDb::new(StoreId::random(
        re_log_types::StoreKind::Recording,
        "test_app",
    ));
    let timeline = Timeline::new_sequence("frame");

    let mut chunk_builder = ChunkBuilder::new(ChunkId::new(), "vid".into());
    for (i, frame_bytes) in iter_h264_frames(RAW_H264_DATA).enumerate() {
        chunk_builder = chunk_builder.with_archetype(
            RowId::new(),
            TimePoint::from_iter([(timeline, i as i64)]),
            &VideoStream::new(VideoCodec::H264).with_sample(frame_bytes),
        );
    }
    store
        .add_chunk(&Arc::new(chunk_builder.build().unwrap()))
        .unwrap();

    let video_stream_lock = cache
        .video_entry(
            &store,
            &"vid".into(),
            *timeline.name(),
            DecodeSettings::default(),
            ChunkTrackingMode::Report,
        )
        .unwrap();
    let video_stream = video_stream_lock.read();

    validate_stream_from_test_data(&video_stream, NUM_FRAMES);
}

#[test]
fn video_stream_cache_from_chunk_per_frame() {
    let mut cache = VideoStreamCache::default();
    let enable_viewer_indexes = false;
    let mut store = re_entity_db::EntityDb::with_store_config(
        StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        enable_viewer_indexes,
        re_chunk_store::ChunkStoreConfig::COMPACTION_DISABLED,
    );
    let timeline = Timeline::new_sequence("frame");

    for (i, frame_bytes) in iter_h264_frames(RAW_H264_DATA).enumerate() {
        let chunk_builder = ChunkBuilder::new(ChunkId::new(), "vid".into()).with_archetype(
            RowId::new(),
            TimePoint::from_iter([(timeline, i as i64)]),
            &VideoStream::new(VideoCodec::H264).with_sample(frame_bytes),
        );
        store
            .add_chunk(&Arc::new(chunk_builder.build().unwrap()))
            .unwrap();
    }

    let video_stream_lock = cache
        .video_entry(
            &store,
            &"vid".into(),
            *timeline.name(),
            DecodeSettings::default(),
            ChunkTrackingMode::Report,
        )
        .unwrap();
    let video_stream = video_stream_lock.read();

    validate_stream_from_test_data(&video_stream, NUM_FRAMES);
}

/// All frames of a video land on the same time, in chunks that reach the store in a
/// different order than the frames were logged in. Samples should come out in logging order.
#[test]
fn video_stream_cache_from_chunks_on_a_single_time() {
    let mut cache = VideoStreamCache::default();
    let enable_viewer_indexes = false;
    let mut store = re_entity_db::EntityDb::with_store_config(
        StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        enable_viewer_indexes,
        re_chunk_store::ChunkStoreConfig::COMPACTION_DISABLED,
    );
    let timeline = Timeline::new_sequence("frame");

    // Rows are created in frame order …
    let rows = iter_h264_frames(RAW_H264_DATA)
        .map(|frame_bytes| (RowId::new(), frame_bytes))
        .collect::<Vec<_>>();

    // … but the chunk with the first frames gets the newest `ChunkId`.
    let older_chunk_id = ChunkId::new();
    let newer_chunk_id = ChunkId::new();
    let (first_rows, remaining_rows) = rows.split_at(5);

    for (chunk_id, rows) in [
        (newer_chunk_id, first_rows),
        (older_chunk_id, remaining_rows),
    ] {
        let mut chunk_builder = ChunkBuilder::new(chunk_id, "vid".into());
        for (row_id, frame_bytes) in rows {
            chunk_builder = chunk_builder.with_archetype(
                *row_id,
                TimePoint::from_iter([(timeline, 0)]),
                &VideoStream::new(VideoCodec::H264).with_sample(*frame_bytes),
            );
        }
        store
            .add_chunk(&Arc::new(chunk_builder.build().unwrap()))
            .unwrap();
    }

    let video_stream_lock = cache
        .video_entry(
            &store,
            &"vid".into(),
            *timeline.name(),
            DecodeSettings::default(),
            ChunkTrackingMode::Report,
        )
        .unwrap();
    let video_stream = video_stream_lock.read();

    validate_stream_from_test_data(&video_stream, NUM_FRAMES);

    let expected_row_ids = rows.iter().map(|(row_id, _)| *row_id).collect::<Vec<_>>();
    assert_sample_row_ids(&video_stream, &expected_row_ids);
}

/// Two chunks land on the same time and their rows interleave, so the chunks overlap in
/// `RowId` alone. No chunk ordering can recover the logging order, the samples of both
/// chunks have to be merged.
#[test]
fn video_stream_cache_from_interleaved_chunks_on_a_single_time() {
    let mut cache = VideoStreamCache::default();
    let enable_viewer_indexes = false;
    let mut store = re_entity_db::EntityDb::with_store_config(
        StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        enable_viewer_indexes,
        re_chunk_store::ChunkStoreConfig::COMPACTION_DISABLED,
    );
    let timeline = Timeline::new_sequence("frame");

    // Rows are created in frame order …
    let rows = iter_h264_frames(RAW_H264_DATA)
        .map(|frame_bytes| (RowId::new(), frame_bytes))
        .collect::<Vec<_>>();

    // … but every other frame goes to the other chunk, and the chunk with the odd frames
    // gets the older `ChunkId`.
    let odd_rows = rows.iter().skip(1).step_by(2).collect::<Vec<_>>();
    let even_rows = rows.iter().step_by(2).collect::<Vec<_>>();

    for chunk_rows in [odd_rows, even_rows] {
        let mut chunk_builder = ChunkBuilder::new(ChunkId::new(), "vid".into());
        for (row_id, frame_bytes) in chunk_rows {
            chunk_builder = chunk_builder.with_archetype(
                *row_id,
                TimePoint::from_iter([(timeline, 0)]),
                &VideoStream::new(VideoCodec::H264).with_sample(*frame_bytes),
            );
        }
        store
            .add_chunk(&Arc::new(chunk_builder.build().unwrap()))
            .unwrap();
    }

    let video_stream_lock = cache
        .video_entry(
            &store,
            &"vid".into(),
            *timeline.name(),
            DecodeSettings::default(),
            ChunkTrackingMode::Report,
        )
        .unwrap();
    let video_stream = video_stream_lock.read();

    validate_stream_from_test_data(&video_stream, NUM_FRAMES);

    let expected_row_ids = rows.iter().map(|(row_id, _)| *row_id).collect::<Vec<_>>();
    assert_sample_row_ids(&video_stream, &expected_row_ids);
}

#[test]
fn video_stream_cache_from_chunk_per_frame_buildup_over_time() {
    let timeline = Timeline::new_sequence("frame");

    for compaction_enabled in [true, false] {
        println!("compaction enabled: {compaction_enabled}");

        let mut cache = VideoStreamCache::default();
        let enable_viewer_indexes = true;
        let mut store = re_entity_db::EntityDb::with_store_config(
            StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            enable_viewer_indexes,
            if compaction_enabled {
                re_chunk_store::ChunkStoreConfig::DEFAULT
            } else {
                re_chunk_store::ChunkStoreConfig::COMPACTION_DISABLED
            },
        );

        let mut frame_iter = iter_h264_frames(RAW_H264_DATA);

        // Add a first frame so we can populate the cache with something
        let chunk_builder = ChunkBuilder::new(ChunkId::new(), "vid".into()).with_archetype(
            RowId::new(),
            TimePoint::from_iter([(timeline, 0)]),
            &VideoStream::new(VideoCodec::H264).with_sample(frame_iter.next().unwrap()),
        );
        store
            .add_chunk(&Arc::new(chunk_builder.build().unwrap()))
            .unwrap();
        let video_stream = cache
            .video_entry(
                &store,
                &"vid".into(),
                *timeline.name(),
                DecodeSettings::default(),
                ChunkTrackingMode::Report,
            )
            .unwrap();
        validate_stream_from_test_data(&video_stream.read(), 1);

        for (i, frame_bytes) in frame_iter.enumerate() {
            let t = 1 + i as i64;
            let timepoint = TimePoint::from_iter([(timeline, t)]);
            let chunk_builder = ChunkBuilder::new(ChunkId::new(), "vid".into()).with_archetype(
                RowId::new(),
                timepoint,
                &VideoStream::new(VideoCodec::H264).with_sample(frame_bytes),
            );
            let store_events = store
                .add_chunk(&Arc::new(chunk_builder.build().unwrap()))
                .unwrap();
            let store_events_refs = store_events.iter().collect::<Vec<_>>();
            cache.on_store_events(&store_events_refs, &store);

            let video_stream = cache
                .video_entry(
                    &store,
                    &"vid".into(),
                    *timeline.name(),
                    DecodeSettings::default(),
                    ChunkTrackingMode::Report,
                )
                .unwrap();
            validate_stream_from_test_data(&video_stream.read(), t as usize + 1);
        }
    }
}

#[test]
fn video_stream_cache_from_chunk_per_frame_with_gc() {
    let mut cache = VideoStreamCache::default();
    let enable_viewer_indexes = true;
    let mut store = re_entity_db::EntityDb::with_store_config(
        StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        enable_viewer_indexes,
        re_chunk_store::ChunkStoreConfig::COMPACTION_DISABLED,
    );
    let timeline = Timeline::new_sequence("frame");

    for (i, frame_bytes) in iter_h264_frames(RAW_H264_DATA).enumerate() {
        let chunk_builder = ChunkBuilder::new(ChunkId::new(), "vid".into()).with_archetype(
            RowId::new(),
            TimePoint::from_iter([(timeline, i as i64)]),
            &VideoStream::new(VideoCodec::H264).with_sample(frame_bytes),
        );
        store
            .add_chunk(&Arc::new(chunk_builder.build().unwrap()))
            .unwrap();
    }

    // Create the cache entry.
    cache
        .video_entry(
            &store,
            &"vid".into(),
            *timeline.name(),
            DecodeSettings::default(),
            ChunkTrackingMode::Report,
        )
        .unwrap();

    // Instead of relying on the "real" GC, we fake it by creating a GC event, pretending the first chunk got removed.
    let storage_engine = store.storage_engine();
    let chunk_store = storage_engine.store();
    cache.on_store_events(
        &[&ChunkStoreEvent {
            store_id: store.store_id().clone(),
            store_generation: store.generation(),
            event_id: 0, // Wrong but don't care.
            diff: ChunkStoreDiff::deletion(
                chunk_store.iter_physical_chunks().next().unwrap().clone(),
                re_chunk_store::ChunkDeletionReason::GarbageCollection,
            ),
        }],
        &store,
    );

    // Check whether the chunk removal had the expected effect.

    let video_stream_lock = cache
        .video_entry(
            &store,
            &"vid".into(),
            *timeline.name(),
            DecodeSettings::default(),
            ChunkTrackingMode::Report,
        )
        .unwrap();
    let video_stream = video_stream_lock.read();

    let data_descr = video_stream.video_renderer.data_descr();
    data_descr.sanity_check().unwrap();

    // Only one frame got removed, BUT the entire first GOP since the first frame was a keyframe!
    assert_eq!(
        data_descr.samples.iter().filter(|s| s.is_loaded()).count(),
        NUM_FRAMES - 1
    );
    assert_eq!(data_descr.keyframe_indices.first(), Some(&10));
}

/// A single statically-logged encoded image should produce a valid one-sample
/// video description.
#[test]
fn video_stream_cache_from_single_static_encoded_image() {
    let jpeg_data = GRINDA_JPEG.to_vec();

    let mut cache = VideoStreamCache::default();
    let mut store = re_entity_db::EntityDb::new(StoreId::random(
        re_log_types::StoreKind::Recording,
        "test_app",
    ));
    let timeline = Timeline::new_sequence("frame");

    // Statically logged image => static chunk (empty timepoint).
    let chunk = ChunkBuilder::new(ChunkId::new(), "img".into())
        .with_archetype_auto_row(
            TimePoint::default(),
            &re_sdk_types::archetypes::EncodedImage::new(jpeg_data).with_media_type("image/jpeg"),
        )
        .build()
        .unwrap();
    assert!(chunk.is_static());
    store.add_chunk(&Arc::new(chunk)).unwrap();

    let video_stream_lock = cache
        .entry(
            &store,
            &"img".into(),
            *timeline.name(),
            DecodeSettings::default(),
            re_sdk_types::archetypes::EncodedImage::descriptor_blob().component,
            re_video::VideoCodec::ImageSequence(Some("image/jpeg".to_owned())),
        )
        .unwrap();
    let video_stream = video_stream_lock.read();

    let data_descr = video_stream.video_renderer.data_descr();
    data_descr.sanity_check().unwrap();

    assert_eq!(
        data_descr.codec,
        re_video::VideoCodec::ImageSequence(Some("image/jpeg".to_owned()))
    );

    // Exactly one sample, and it's a keyframe (images are always keyframes).
    assert_eq!(data_descr.samples.num_elements(), 1);
    assert_eq!(
        data_descr.keyframe_indices,
        vec![data_descr.samples.min_index()]
    );

    let encoding_details = data_descr.encoding_details.clone().unwrap();
    assert_eq!(encoding_details.codec_string, "image/jpeg");
    assert_eq!(encoding_details.coded_dimensions, [640, 480]);
}

/// A static chunk with several rows holds a single current value: the row with the highest
/// `RowId`. The emitted sample should point at that row, not at an earlier shadowed one.
#[test]
fn video_stream_cache_static_picks_highest_row_id() {
    let jpeg_data = GRINDA_JPEG.to_vec();
    let image =
        re_sdk_types::archetypes::EncodedImage::new(jpeg_data).with_media_type("image/jpeg");

    let mut cache = VideoStreamCache::default();
    let mut store = re_entity_db::EntityDb::new(StoreId::random(
        re_log_types::StoreKind::Recording,
        "test_app",
    ));
    let timeline = Timeline::new_sequence("frame");

    // Two statically logged rows. The second one has the higher `RowId` and shadows the first.
    let low_row_id = RowId::new();
    let high_row_id = RowId::new();
    assert!(high_row_id > low_row_id);

    let chunk = ChunkBuilder::new(ChunkId::new(), "img".into())
        .with_archetype(low_row_id, TimePoint::default(), &image)
        .with_archetype(high_row_id, TimePoint::default(), &image)
        .build()
        .unwrap();
    assert!(chunk.is_static());
    store.add_chunk(&Arc::new(chunk)).unwrap();

    let video_stream_lock = cache
        .entry(
            &store,
            &"img".into(),
            *timeline.name(),
            DecodeSettings::default(),
            re_sdk_types::archetypes::EncodedImage::descriptor_blob().component,
            re_video::VideoCodec::ImageSequence(Some("image/jpeg".to_owned())),
        )
        .unwrap();
    let video_stream = video_stream_lock.read();

    let data_descr = video_stream.video_renderer.data_descr();
    data_descr.sanity_check().unwrap();

    // Exactly one sample, sourced from the highest-`RowId` row.
    assert_eq!(data_descr.samples.num_elements(), 1);
    let sample = data_descr.samples.iter().next().unwrap().sample().unwrap();
    let re_video::VideoSource::Id { sample_id, .. } = sample.source else {
        panic!("expected an id-based video source");
    };
    assert_eq!(sample_id, Some(high_row_id.as_tuid()));
}

/// A chunk with a row without a blob between two rows with one blob each.
/// Looking up a sample by its row gives the bytes logged in that row.
#[test]
fn video_store_source_skips_empty_rows() {
    let blob_component = re_sdk_types::archetypes::EncodedImage::descriptor_blob();
    let timeline = Timeline::new_sequence("frame");
    let row_ids = [RowId::new(), RowId::new(), RowId::new()];
    let row_blobs: [Vec<components::Blob>; 3] = [
        vec![components::Blob::from(vec![1_u8, 2, 3])],
        vec![],
        vec![components::Blob::from(vec![4_u8, 5])],
    ];

    let mut builder = ChunkBuilder::new(ChunkId::new(), "img".into());
    for (i, (row_id, blobs)) in std::iter::zip(&row_ids, &row_blobs).enumerate() {
        builder = builder.with_component_batch(
            *row_id,
            TimePoint::from_iter([(timeline, i as i64)]),
            (blob_component.clone(), blobs as _),
        );
    }
    let chunk = builder.build().unwrap();
    let chunk_id = chunk.id();

    let mut store = re_entity_db::EntityDb::new(StoreId::random(
        re_log_types::StoreKind::Recording,
        "test_app",
    ));
    store.add_chunk(&Arc::new(chunk)).unwrap();

    let engine = store.storage_engine();
    let video_source = VideoStoreSource {
        store: engine.store(),
        sample_component: blob_component.component,
        indicate: false,
    };

    let sample_bytes: Vec<&[u8]> = [row_ids[0], row_ids[2]]
        .iter()
        .map(|row_id| {
            video_source.get_video_chunk(re_video::VideoSource::Id {
                container_id: chunk_id.as_tuid(),
                sample_id: Some(row_id.as_tuid()),
            })
        })
        .collect();
    assert_eq!(sample_bytes, [&[1_u8, 2, 3][..], &[4_u8, 5][..]]);
}

/// A chunk already in the store with a row without a blob between two rows with one image
/// each. Creating the cache entry gives a clear for the empty row and one loaded sample per
/// image.
#[test]
fn video_stream_cache_empty_row_is_clear() {
    let blob_component = re_sdk_types::archetypes::EncodedImage::descriptor_blob();
    let timeline = Timeline::new_sequence("frame");
    let image = components::Blob::from(GRINDA_JPEG);
    let row_blobs: [Vec<components::Blob>; 3] = [vec![image.clone()], vec![], vec![image]];

    let mut builder = ChunkBuilder::new(ChunkId::new(), "img".into());
    for (i, blobs) in row_blobs.iter().enumerate() {
        builder = builder.with_component_batch(
            RowId::new(),
            TimePoint::from_iter([(timeline, i as i64)]),
            (blob_component.clone(), blobs as _),
        );
    }

    let mut cache = VideoStreamCache::default();
    let mut store = re_entity_db::EntityDb::new(StoreId::random(
        re_log_types::StoreKind::Recording,
        "test_app",
    ));
    store
        .add_chunk(&Arc::new(builder.build().unwrap()))
        .unwrap();

    let video_stream_lock = cache
        .entry(
            &store,
            &"img".into(),
            *timeline.name(),
            DecodeSettings::default(),
            blob_component.component,
            re_video::VideoCodec::ImageSequence(Some("image/jpeg".to_owned())),
        )
        .unwrap();
    let video_stream = video_stream_lock.read();
    let data_descr = video_stream.video_renderer.data_descr();

    let samples = data_descr.samples.iter().collect::<Vec<_>>();
    assert_eq!(samples.len(), 3);
    assert!(samples[0].sample().is_some());
    assert!(samples[1].is_clear());
    assert!(samples[2].sample().is_some());
}
