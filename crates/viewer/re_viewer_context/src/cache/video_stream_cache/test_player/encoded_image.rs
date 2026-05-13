use std::sync::Arc;

use re_chunk::{Chunk, RowId, TimeInt, Timeline};
use re_entity_db::EntityDb;
use re_log_types::StoreId;
use re_sdk_types::archetypes::EncodedImage;

use crate::{SharablePlayableVideoStream, VideoStreamCache};

use super::{STREAM_ENTITY, TIMELINE_NAME, TestVideoPlayer, load_chunks, unload_chunks};

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
            &|| {
                let media_type_component = EncodedImage::descriptor_media_type().component;
                let query_result = store.storage_engine().cache().latest_at(
                    &re_chunk::LatestAtQuery::new(TIMELINE_NAME.into(), re_chunk::TimeInt::MAX),
                    &re_chunk::EntityPath::from(STREAM_ENTITY),
                    [media_type_component],
                );
                let media_type = query_result
                    .get_required(media_type_component)
                    .ok()
                    .and_then(|chunk| {
                        chunk
                            .component_mono::<re_sdk_types::components::MediaType>(
                                media_type_component,
                            )?
                            .ok()
                            .map(|mt| mt.to_string())
                    });
                Ok(re_video::VideoCodec::ImageSequence(media_type))
            },
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
