use std::sync::Arc;

use re_chunk::{Chunk, RowId, TimeInt, TimePoint, Timeline};
use re_chunk_store::ChunkTrackingMode;
use re_entity_db::EntityDb;
use re_log_types::StoreId;
use re_sdk_types::archetypes::EncodedImage;

use crate::cache::cache_trait::Cache as _;
use crate::{SharablePlayableVideoStream, VideoStreamCache};

use super::{
    STREAM_ENTITY, TIMELINE_NAME, TestVideoPlayer, assert_loading, load_chunks,
    load_into_rrd_manifest, unload_chunks,
};

fn test_png_blob() -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        image::ImageEncoder::write_image(encoder, &[0u8; 4], 1, 1, image::ColorType::Rgba8.into())
            .unwrap();
    }
    buf
}

fn codec_chunk() -> Chunk {
    Chunk::builder(STREAM_ENTITY)
        .with_archetype(
            RowId::new(),
            [(
                Timeline::new_duration(TIMELINE_NAME),
                TimeInt::from_secs(0.0),
            )],
            &EncodedImage::update_fields().with_media_type("image/png"),
        )
        .build()
        .unwrap()
}

fn image_chunk(start_time: f64, dt: f64, count: u64) -> Chunk {
    let timeline = Timeline::new_duration(TIMELINE_NAME);
    let blob = test_png_blob();
    let mut builder = Chunk::builder(STREAM_ENTITY);

    for i in 0..count {
        let time = start_time + i as f64 * dt;
        builder = builder.with_archetype(
            RowId::new(),
            [(timeline, TimeInt::from_secs(time))],
            &EncodedImage::update_fields().with_blob(blob.clone()),
        );
    }

    builder.build().unwrap()
}

fn playable_stream(cache: &mut VideoStreamCache, store: &EntityDb) -> SharablePlayableVideoStream {
    let blob_component = EncodedImage::descriptor_blob().component;
    let media_type_component = EncodedImage::descriptor_media_type().component;
    let query_result = store.storage_engine().cache().latest_at(
        ChunkTrackingMode::Report,
        &re_chunk::LatestAtQuery::new(TIMELINE_NAME.into(), re_chunk::TimeInt::MAX),
        &re_chunk::EntityPath::from(STREAM_ENTITY),
        [media_type_component],
    );
    let media_type = query_result
        .get_required(media_type_component)
        .ok()
        .and_then(|chunk| {
            chunk
                .component_mono::<re_sdk_types::components::MediaType>(media_type_component)?
                .ok()
                .map(|mt| mt.to_string())
        });
    cache
        .entry(
            store,
            &re_chunk::EntityPath::from(STREAM_ENTITY),
            TIMELINE_NAME.into(),
            re_video::DecodeSettings {
                hw_acceleration: Default::default(),
                ffmpeg_path: Some(std::path::PathBuf::from("/not/used")),
            },
            blob_component,
            re_video::VideoCodec::ImageSequence(media_type),
        )
        .unwrap()
}

#[test]
fn basic_playback() {
    let mut cache = VideoStreamCache::default();
    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let codec = Arc::new(codec_chunk());
    let frames = Arc::new(image_chunk(1.0, 1.0, 4));

    load_chunks(&mut store, &mut cache, &[codec]);
    load_chunks(&mut store, &mut cache, &[frames]);

    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream);

    player
        .play_store_with_component(
            1.0..5.0,
            1.0,
            &store,
            EncodedImage::descriptor_blob().component,
        )
        .unwrap();
    player.expect_decoded_samples(0..4);
}

#[test]
fn multi_chunk_with_gc() {
    let mut cache = VideoStreamCache::default();
    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let codec = Arc::new(codec_chunk());
    let chunk_a = Arc::new(image_chunk(1.0, 1.0, 4));
    let chunk_b = Arc::new(image_chunk(5.0, 1.0, 4));

    load_chunks(&mut store, &mut cache, &[codec.clone(), chunk_a, chunk_b]);

    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream);

    player
        .play_store_with_component(
            1.0..9.0,
            1.0,
            &store,
            EncodedImage::descriptor_blob().component,
        )
        .unwrap();
    player.expect_decoded_samples(0..8);

    // GC chunk_a, keep chunk_b (times 5..9).
    unload_chunks(&store, &mut cache, 5.0..9.0);

    // Reload codec since GC may have removed it.
    load_chunks(&mut store, &mut cache, &[codec]);

    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream);

    // Only chunk_b's 4 frames remain, keeping their original indices.
    player
        .play_store_with_component(
            5.0..9.0,
            1.0,
            &store,
            EncodedImage::descriptor_blob().component,
        )
        .unwrap();
    player.expect_decoded_samples(4..8);
}

/// A statically logged encoded image carries no timeline, so it isn't in the manifest's
/// `temporal_map`.
fn static_codec_chunk() -> Chunk {
    Chunk::builder(STREAM_ENTITY)
        .with_archetype(
            RowId::new(),
            TimePoint::default(),
            &EncodedImage::update_fields().with_media_type("image/png"),
        )
        .build()
        .unwrap()
}

fn static_image_chunk() -> Chunk {
    Chunk::builder(STREAM_ENTITY)
        .with_archetype(
            RowId::new(),
            TimePoint::default(),
            &EncodedImage::update_fields().with_blob(test_png_blob()),
        )
        .build()
        .unwrap()
}

/// A static sample chunk described only by the manifest is still pre-allocated, and decodes
/// once the chunk is materialized.
#[test]
fn static_image_from_manifest() {
    let mut cache = VideoStreamCache::default();
    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let codec = Arc::new(static_codec_chunk());
    let image = Arc::new(static_image_chunk());

    // The manifest describes both static chunks, but only the media type is materialized.
    load_into_rrd_manifest(&mut store, &[codec.clone(), image.clone()]);
    load_chunks(&mut store, &mut cache, &[codec]);

    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream.clone());

    // The static sample comes from the manifest's `static_map`, not its `temporal_map`. Its
    // data isn't loaded yet, so playback reports loading.
    assert_eq!(
        video_stream
            .read()
            .video_renderer
            .data_descr()
            .samples
            .num_elements(),
        1
    );
    assert_loading(player.play_store_with_component(
        0.0..2.0,
        1.0,
        &store,
        EncodedImage::descriptor_blob().component,
    ));
    player.expect_decoded_samples(None);

    // Materializing the static chunk fills the pre-allocated slot without adding another sample.
    load_chunks(&mut store, &mut cache, &[image]);
    assert_eq!(
        video_stream
            .read()
            .video_renderer
            .data_descr()
            .samples
            .num_elements(),
        1
    );
    player
        .play_store_with_component(
            0.0..2.0,
            1.0,
            &store,
            EncodedImage::descriptor_blob().component,
        )
        .unwrap();
    player.expect_decoded_samples(0..1);
}

/// When a static sample chunk is both described by the manifest and materialized, the two
/// describe the same sample rather than being counted separately.
#[test]
fn static_image_not_double_counted() {
    let mut cache = VideoStreamCache::default();
    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let codec = Arc::new(static_codec_chunk());
    let image = Arc::new(static_image_chunk());

    load_into_rrd_manifest(&mut store, &[codec.clone(), image.clone()]);
    load_chunks(&mut store, &mut cache, &[codec, image]);

    let video_stream = playable_stream(&mut cache, &store);
    let guard = video_stream.read();
    let descr = guard.video_renderer.data_descr();

    assert_eq!(descr.samples.num_elements(), 1);
    assert!(descr.samples[descr.samples.min_index()].sample().is_some());
}

/// Logging a newer static image to the same entity replaces the previous one, leaving the
/// stream backed by the newest chunk.
#[test]
fn static_image_overwrite() {
    let mut cache = VideoStreamCache::default();
    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let codec = Arc::new(static_codec_chunk());
    let first = Arc::new(static_image_chunk());
    load_chunks(&mut store, &mut cache, &[codec, first]);

    let video_stream = playable_stream(&mut cache, &store);
    assert_eq!(
        video_stream
            .read()
            .video_renderer
            .data_descr()
            .samples
            .num_elements(),
        1
    );

    let second = Arc::new(static_image_chunk());
    load_chunks(&mut store, &mut cache, std::slice::from_ref(&second));

    let guard = video_stream.read();
    let descr = guard.video_renderer.data_descr();
    assert_eq!(descr.samples.num_elements(), 1);
    assert_eq!(
        descr.samples[descr.samples.min_index()].source_primary_id(),
        Some(second.id().as_tuid())
    );
}

/// Static data isn't garbage collected, so a materialized static image stays loaded across a
/// collection that evicts everything else.
#[test]
fn static_image_survives_gc() {
    let mut cache = VideoStreamCache::default();
    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let codec = Arc::new(static_codec_chunk());
    let image = Arc::new(static_image_chunk());
    load_into_rrd_manifest(&mut store, &[codec.clone(), image.clone()]);
    load_chunks(&mut store, &mut cache, &[codec, image]);

    let video_stream = playable_stream(&mut cache, &store);
    {
        let guard = video_stream.read();
        let descr = guard.video_renderer.data_descr();
        assert_eq!(descr.samples.num_elements(), 1);
        assert!(descr.samples[descr.samples.min_index()].sample().is_some());
    }

    let events = store.gc(&re_chunk_store::GarbageCollectionOptions {
        target: re_chunk_store::GarbageCollectionTarget::Everything,
        time_budget: std::time::Duration::from_secs(u64::MAX),
        protect_latest: 0,
        protected_chunks: Default::default(),
        protected_time_ranges: Default::default(),
        furthest_from: None,
        perform_deep_deletions: false,
    });
    cache.on_store_events(&events.iter().collect::<Vec<_>>(), &store);

    // The static chunk is untouched by the collection, so its sample stays loaded.
    let guard = video_stream.read();
    let descr = guard.video_renderer.data_descr();
    assert_eq!(descr.samples.num_elements(), 1);
    assert!(descr.samples[descr.samples.min_index()].sample().is_some());
}

/// Overwriting a manifest-backed static image with a newer one leaves the stream backed by
/// the newest chunk.
#[test]
fn static_image_overwrite_with_manifest() {
    let mut cache = VideoStreamCache::default();
    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let codec = Arc::new(static_codec_chunk());
    let first = Arc::new(static_image_chunk());

    // The first image is described by the manifest and materialized.
    load_into_rrd_manifest(&mut store, &[codec.clone(), first.clone()]);
    load_chunks(&mut store, &mut cache, &[codec, first]);

    let video_stream = playable_stream(&mut cache, &store);
    assert_eq!(
        video_stream
            .read()
            .video_renderer
            .data_descr()
            .samples
            .num_elements(),
        1
    );

    let second = Arc::new(static_image_chunk());
    load_chunks(&mut store, &mut cache, std::slice::from_ref(&second));

    let guard = video_stream.read();
    let descr = guard.video_renderer.data_descr();
    assert_eq!(descr.samples.num_elements(), 1);
    assert_eq!(
        descr.samples[descr.samples.min_index()].source_primary_id(),
        Some(second.id().as_tuid())
    );
}
