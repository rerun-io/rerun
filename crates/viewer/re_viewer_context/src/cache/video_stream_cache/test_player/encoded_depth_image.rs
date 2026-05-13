use std::sync::Arc;

use re_chunk::{Chunk, RowId, TimeInt, Timeline};
use re_entity_db::EntityDb;
use re_log_types::StoreId;
use re_sdk_types::archetypes::EncodedDepthImage;

use crate::{SharablePlayableVideoStream, VideoStreamCache};

use super::{STREAM_ENTITY, TIMELINE_NAME, TestVideoPlayer, load_chunks, unload_chunks};

fn test_depth_png_blob() -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        image::ImageEncoder::write_image(encoder, &[0u8; 2], 1, 1, image::ColorType::L16.into())
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
            &EncodedDepthImage::update_fields().with_media_type("image/png"),
        )
        .build()
        .unwrap()
}

fn depth_image_chunk(start_time: f64, dt: f64, count: u64) -> Chunk {
    let timeline = Timeline::new_duration(TIMELINE_NAME);
    let blob = test_depth_png_blob();
    let mut builder = Chunk::builder(STREAM_ENTITY);

    for i in 0..count {
        let time = start_time + i as f64 * dt;
        builder = builder.with_archetype(
            RowId::new(),
            [(timeline, TimeInt::from_secs(time))],
            &EncodedDepthImage::update_fields().with_blob(blob.clone()),
        );
    }

    builder.build().unwrap()
}

fn playable_stream(cache: &mut VideoStreamCache, store: &EntityDb) -> SharablePlayableVideoStream {
    let blob_component = EncodedDepthImage::descriptor_blob().component;
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
                let media_type_component = EncodedDepthImage::descriptor_media_type().component;
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
    let frames = Arc::new(depth_image_chunk(1.0, 1.0, 4));

    load_chunks(&mut store, &mut cache, &[codec]);
    load_chunks(&mut store, &mut cache, &[frames]);

    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream);

    player
        .play_store_with_component(
            1.0..5.0,
            1.0,
            &store,
            EncodedDepthImage::descriptor_blob().component,
        )
        .unwrap();
    player.expect_decoded_samples(0..4);
}

#[test]
fn multi_chunk_with_gc() {
    let mut cache = VideoStreamCache::default();
    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let codec = Arc::new(codec_chunk());
    let chunk_a = Arc::new(depth_image_chunk(1.0, 1.0, 4));
    let chunk_b = Arc::new(depth_image_chunk(5.0, 1.0, 4));

    load_chunks(&mut store, &mut cache, &[codec.clone(), chunk_a, chunk_b]);

    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream);

    player
        .play_store_with_component(
            1.0..9.0,
            1.0,
            &store,
            EncodedDepthImage::descriptor_blob().component,
        )
        .unwrap();
    player.expect_decoded_samples(0..8);

    // GC chunk_a, keep chunk_b.
    unload_chunks(&store, &mut cache, 5.0..9.0);

    load_chunks(&mut store, &mut cache, &[codec]);

    let video_stream = playable_stream(&mut cache, &store);
    let mut player = TestVideoPlayer::from_stream(video_stream);

    // Only chunk_b's 4 frames remain, keeping their original indices.
    player
        .play_store_with_component(
            5.0..9.0,
            1.0,
            &store,
            EncodedDepthImage::descriptor_blob().component,
        )
        .unwrap();
    player.expect_decoded_samples(4..8);
}

/// A 2x2 L16 PNG logged as `EncodedDepthImage` should be recognized with
/// the correct dimensions and bit depth.
#[test]
fn png_decoding() {
    let mut cache = VideoStreamCache::default();
    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let width = 2u32;
    let height = 2u32;
    let depth_values: [u16; 4] = [0, 1, 2, 3];

    let mut encoded_png = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new(&mut encoded_png);
        image::ImageEncoder::write_image(
            encoder,
            bytemuck::cast_slice(&depth_values),
            width,
            height,
            image::ColorType::L16.into(),
        )
        .unwrap();
    }

    let codec = Chunk::builder(STREAM_ENTITY)
        .with_archetype(
            RowId::new(),
            [(
                Timeline::new_duration(TIMELINE_NAME),
                TimeInt::from_secs(0.0),
            )],
            &EncodedDepthImage::new(encoded_png.clone()).with_media_type("image/png"),
        )
        .build()
        .unwrap();

    load_chunks(&mut store, &mut cache, &[Arc::new(codec)]);

    let video_stream = playable_stream(&mut cache, &store);
    let descr = video_stream.read_arc().video_descr().clone();

    assert_eq!(
        descr.samples.next_index(),
        1,
        "should have exactly 1 sample"
    );

    let encoding_details = descr
        .encoding_details
        .as_ref()
        .expect("should have encoding details");
    assert_eq!(
        encoding_details.coded_dimensions,
        [width as u16, height as u16]
    );
    assert_eq!(encoding_details.bit_depth, Some(16));
}

/// An `EncodedDepthImage` without an explicit media type should still be
/// loadable when the format can be guessed from the blob data.
#[test]
fn guesses_png_media_type() {
    let mut cache = VideoStreamCache::default();
    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let depth_values: [u16; 4] = [0, 1, 2, 3];

    let mut encoded_png = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new(&mut encoded_png);
        image::ImageEncoder::write_image(
            encoder,
            bytemuck::cast_slice(&depth_values),
            2,
            2,
            image::ColorType::L16.into(),
        )
        .unwrap();
    }

    // No media type set.
    let codec = Chunk::builder(STREAM_ENTITY)
        .with_archetype(
            RowId::new(),
            [(
                Timeline::new_duration(TIMELINE_NAME),
                TimeInt::from_secs(0.0),
            )],
            &EncodedDepthImage::new(encoded_png),
        )
        .build()
        .unwrap();

    load_chunks(&mut store, &mut cache, &[Arc::new(codec)]);

    let blob_component = EncodedDepthImage::descriptor_blob().component;
    let result = cache.entry(
        &store,
        &re_chunk::EntityPath::from(STREAM_ENTITY),
        TIMELINE_NAME.into(),
        re_video::DecodeSettings {
            hw_acceleration: Default::default(),
            ffmpeg_path: Some(std::path::PathBuf::from("/not/used")),
        },
        blob_component,
        &|| Ok(re_video::VideoCodec::ImageSequence(None)),
    );

    assert!(
        result.is_ok(),
        "should succeed even without explicit media type"
    );

    let video_stream = result.unwrap();
    let descr = video_stream.read_arc().video_descr().clone();
    assert_eq!(descr.samples.next_index(), 1);
}
