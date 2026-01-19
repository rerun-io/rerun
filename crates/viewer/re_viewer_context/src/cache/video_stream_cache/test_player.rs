use std::{iter::once, ops::Range, sync::Arc};

use crossbeam::channel::{Receiver, Sender};
use re_byte_size::SizeBytes as _;
use re_chunk::{Chunk, RowId, TimeInt, Timeline};
use re_entity_db::EntityDb;
use re_log_encoding::{RrdManifest, RrdManifestBuilder};
use re_log_types::{AbsoluteTimeRange, StoreId, external::re_tuid::Tuid};
use re_renderer::video::{
    InsufficientSampleDataError, VideoPlayer, VideoPlayerError, VideoSampleDecoder,
};
use re_sdk_types::{archetypes::VideoStream, components::VideoCodec};
use re_video::{
    AV1_TEST_INTER_FRAME, AV1_TEST_KEYFRAME, AsyncDecoder, AsyncDecoder, Receiver, SampleIndex,
    SampleIndex, SampleMetadataState, SampleMetadataState, Sender, Time, Time,
    VideoDataDescription, VideoDataDescription,
};

use crate::{
    Cache as _, SharablePlayableVideoStream, VideoStreamCache, VideoStreamProcessingError,
};

struct TestDecoder {
    sender: Sender<Result<re_video::Frame, re_video::DecodeError>>,
    sample_tx: Sender<SampleIndex>,
    min_num_samples_to_enqueue_ahead: usize,
}

impl AsyncDecoder for TestDecoder {
    fn submit_chunk(&mut self, chunk: re_video::Chunk) -> re_video::DecodeResult<()> {
        self.sample_tx.send(chunk.sample_idx).unwrap();

        self.sender
            .send(Ok(re_video::Frame {
                content: re_video::FrameContent {
                    data: Vec::new(),
                    width: 0,
                    height: 0,
                    format: re_video::PixelFormat::Rgb8Unorm,
                },
                info: re_video::FrameInfo {
                    is_sync: Some(chunk.is_sync),
                    sample_idx: Some(chunk.sample_idx),
                    frame_nr: Some(chunk.frame_nr),
                    presentation_timestamp: chunk.presentation_timestamp,
                    duration: chunk.duration,
                    latest_decode_timestamp: Some(chunk.decode_timestamp),
                },
            }))
            .unwrap();

        Ok(())
    }

    fn reset(
        &mut self,
        _video_descr: &re_video::VideoDataDescription,
    ) -> re_video::DecodeResult<()> {
        Ok(())
    }

    fn min_num_samples_to_enqueue_ahead(&self) -> usize {
        self.min_num_samples_to_enqueue_ahead
    }
}

struct TestVideoPlayer {
    video: VideoPlayer,
    sample_rx: Receiver<SampleIndex>,
    video_descr: VideoDataDescription,
    video_descr_source: Option<Box<dyn Fn() -> VideoDataDescription>>,
    time: f64,
}

impl TestVideoPlayer {
    fn from_descr(video_descr: VideoDataDescription) -> Self {
        let (sample_tx, sample_rx) = crossbeam::channel::unbounded();
        let video = VideoPlayer::new_with_encoder(
            VideoSampleDecoder::new("test_decoder".to_owned(), |sender| {
                Ok(Box::new(TestDecoder {
                    sample_tx,
                    min_num_samples_to_enqueue_ahead: 2,
                    sender,
                }))
            })
            .unwrap(),
        );

        Self {
            video,
            sample_rx,
            video_descr,
            video_descr_source: None,
            time: 0.0,
        }
    }

    fn from_stream(stream: SharablePlayableVideoStream) -> Self {
        let video_descr_source =
            Box::new(move || stream.read_arc().video_renderer.data_descr().clone());
        let mut this = Self::from_descr(video_descr_source());

        this.video_descr_source = Some(video_descr_source);

        this
    }

    fn play(&mut self, range: Range<f64>, time_step: f64) -> Result<(), VideoPlayerError> {
        self.play_with_buffer(range, time_step, &|_| &[])
    }

    fn play_with_buffer<'a>(
        &mut self,
        range: Range<f64>,
        time_step: f64,
        get_buffer: &dyn Fn(Tuid) -> &'a [u8],
    ) -> Result<(), VideoPlayerError> {
        if let Some(source) = &self.video_descr_source {
            self.video_descr = source();
        }
        self.time = range.start;
        for i in 0..((range.end - self.time) / time_step).next_down().floor() as i32 {
            let time = self.time + i as f64 * time_step;
            self.frame_at(time, get_buffer)?;
        }

        self.time = range.end;

        Ok(())
    }

    fn frame_at<'a>(
        &mut self,
        time: f64,
        get_buffer: &dyn Fn(Tuid) -> &'a [u8],
    ) -> Result<(), VideoPlayerError> {
        self.video.frame_at(
            Time::from_secs(time, re_video::Timescale::NANOSECOND),
            &self.video_descr,
            &mut |_, _| Ok(()),
            get_buffer,
        )?;

        Ok(())
    }

    #[track_caller]
    fn expect_decoded_samples(&self, samples: impl IntoIterator<Item = SampleIndex>) {
        eprintln!("  expect_decoded_samples…");

        let received = self.sample_rx.try_iter().collect::<Vec<_>>();
        let expected = samples.into_iter().collect::<Vec<_>>();

        if let Some((e, r)) = expected.iter().zip(received.iter()).find(|(a, b)| a != b) {
            panic!(
                "   Expected: {expected:?}\n   Received: {received:?}\nFirst Issue: expected {e}, got {r}"
            );
        }
    }

    fn set_sample(&mut self, idx: SampleIndex, mut sample: SampleMetadataState) {
        eprintln!("  set_sample {idx}…");

        if let Some(sample) = sample.sample_mut() {
            sample.frame_nr = idx as u32;
            sample.decode_timestamp = re_video::Time::from_secs(
                sample
                    .decode_timestamp
                    .into_secs(re_video::Timescale::NANOSECOND)
                    - 0.1,
                re_video::Timescale::NANOSECOND,
            );
        }

        match (
            self.video_descr.samples[idx]
                .sample()
                .is_some_and(|s| s.is_sync),
            sample.sample().is_some_and(|s| s.is_sync),
        ) {
            (false, false) | (true, true) => {}

            (true, false) => {
                let keyframe_idx = self
                    .video_descr
                    .keyframe_indices
                    .partition_point(|i| *i < idx);

                self.video_descr.keyframe_indices.remove(keyframe_idx);
            }

            (false, true) => {
                let keyframe_idx = self
                    .video_descr
                    .keyframe_indices
                    .partition_point(|i| *i < idx);

                self.video_descr.keyframe_indices.insert(keyframe_idx, idx);
            }
        }
        self.video_descr.samples[idx] = sample;

        super::update_sample_durations(
            &super::ChunkSampleRange {
                first_sample: idx,
                last_sample: idx,
            },
            &mut self.video_descr.samples,
        )
        .unwrap();
    }
}

fn unloaded() -> SampleMetadataState {
    SampleMetadataState::Unloaded(Tuid::new())
}

/// A P-Frame
fn frame(time: f64) -> SampleMetadataState {
    let time = Time::from_secs(time, re_video::Timescale::NANOSECOND);
    SampleMetadataState::Present(re_video::SampleMetadata {
        is_sync: false,
        decode_timestamp: time,
        presentation_timestamp: time,

        // Assigned later.
        frame_nr: 0,
        duration: None,

        // Not relevant for these tests.
        source_id: Tuid::new(),
        byte_span: re_video::Span { start: 0, len: 0 },
    })
}

fn keyframe(time: f64) -> SampleMetadataState {
    let time = Time::from_secs(time, re_video::Timescale::NANOSECOND);
    SampleMetadataState::Present(re_video::SampleMetadata {
        is_sync: true,
        decode_timestamp: time,
        presentation_timestamp: time,

        // Assigned later.
        frame_nr: 0,
        duration: None,

        // Not relevant for these tests.
        source_id: Tuid::new(),
        byte_span: re_video::Span { start: 0, len: 0 },
    })
}

fn create_video(
    samples: impl IntoIterator<Item = SampleMetadataState>,
) -> Result<TestVideoPlayer, VideoStreamProcessingError> {
    let mut samples: re_video::StableIndexDeque<SampleMetadataState> =
        samples.into_iter().collect();

    let mut keyframe_indices = Vec::new();
    for (idx, sample) in samples.iter_indexed_mut() {
        if let Some(sample) = sample.sample_mut() {
            sample.frame_nr = idx as u32;
            sample.decode_timestamp = re_video::Time::from_secs(
                sample
                    .decode_timestamp
                    .into_secs(re_video::Timescale::NANOSECOND)
                    - 0.1,
                re_video::Timescale::NANOSECOND,
            );
            if sample.is_sync {
                keyframe_indices.push(idx);
            }
        }
    }

    super::update_sample_durations(
        &super::ChunkSampleRange {
            first_sample: 0,
            last_sample: samples.next_index() - 1,
        },
        &mut samples,
    )?;

    let video_descr = VideoDataDescription {
        delivery_method: re_video::VideoDeliveryMethod::Stream {
            last_time_updated_samples: std::time::Instant::now(),
        },
        keyframe_indices,
        samples_statistics: re_video::SamplesStatistics::new(&samples),
        samples,

        // Unused for these tests.
        codec: re_video::VideoCodec::H265,
        encoding_details: None,
        mp4_tracks: Default::default(),
        timescale: None,
    };

    Ok(TestVideoPlayer::from_descr(video_descr))
}

fn test_simple_video(mut video: TestVideoPlayer, count: usize, dt: f64, max_time: f64) {
    re_log::setup_logging();

    video.play(0.0..max_time, dt).unwrap();

    video.expect_decoded_samples(0..count);

    // try again at 0.5x speed.

    video.play(0.0..max_time, dt * 0.5).unwrap();

    video.expect_decoded_samples(0..count);

    // and at 2x speed.

    video.play(0.0..max_time, dt * 2.0).unwrap();

    video.expect_decoded_samples(0..count);
}

#[test]
fn player_all_keyframes() {
    let count = 10;
    let dt = 0.1;
    let max_time = count as f64 * dt;
    let video = create_video((0..count).map(|t| keyframe(t as f64 * dt))).unwrap();

    test_simple_video(video, count, dt, max_time);
}

#[test]
fn player_one_keyframe() {
    let count = 10;
    let dt = 0.1;
    let max_time = count as f64 * dt;
    let video =
        create_video(once(keyframe(0.0)).chain((1..count).map(|t| frame(t as f64 * dt)))).unwrap();

    test_simple_video(video, count, dt, max_time);
}

#[test]
fn player_keyframes_then_frames() {
    let count = 50usize;
    let keyframe_range_size = 10;
    let dt = 0.1;
    let max_time = count as f64 * dt;
    let video = create_video((0..count).map(|t| {
        let time = t as f64 * dt;
        if t.is_multiple_of(keyframe_range_size) {
            keyframe(time)
        } else {
            frame(time)
        }
    }))
    .unwrap();

    test_simple_video(video, count, dt, max_time);
}

#[test]
fn player_irregular() {
    let samples = [
        keyframe(0.0),
        keyframe(0.1),
        frame(0.11),
        frame(0.12),
        frame(0.125),
        frame(0.13),
        keyframe(1.0),
        frame(2.0),
        frame(50.0),
        keyframe(1000.0),
        keyframe(2000.0),
        frame(2001.0),
        frame(2201.0),
        frame(2221.0),
    ];
    let count = samples.len();
    let video = create_video(samples).unwrap();

    test_simple_video(video, count, 0.1, 2500.0);
}

#[test]
fn player_unsorted() {
    let samples = [keyframe(0.0), keyframe(1.0), keyframe(2.0), keyframe(1.0)];
    let Err(err) = create_video(samples) else {
        panic!("Video creation shouldn't succeed for unordered samples");
    };

    assert!(
        matches!(err, VideoStreamProcessingError::OutOfOrderSamples),
        "Expected {} got {err}",
        VideoStreamProcessingError::OutOfOrderSamples
    );
}

#[track_caller]
fn assert_loading(err: Result<(), VideoPlayerError>) {
    let err = err.unwrap_err();
    assert!(
        matches!(
            err,
            VideoPlayerError::InsufficientSampleData(
                InsufficientSampleDataError::ExpectedSampleNotAvailable
            )
        ),
        "Expected {:?} got {err:?}",
        VideoPlayerError::InsufficientSampleData(
            InsufficientSampleDataError::ExpectedSampleNotAvailable
        )
    );
}

#[test]
fn player_with_unloaded() {
    #[track_caller]
    fn assert_loading(err: Result<(), VideoPlayerError>) {
        let err = err.unwrap_err();
        assert!(
            matches!(
                err,
                VideoPlayerError::InsufficientSampleData(
                    InsufficientSampleDataError::ExpectedSampleNotAvailable
                )
            ),
            "Expected {} got {err}",
            VideoPlayerError::InsufficientSampleData(
                InsufficientSampleDataError::ExpectedSampleNotAvailable
            )
        );
    }

    let mut video = create_video([
        keyframe(0.),
        frame(1.),
        frame(2.),
        frame(3.),
        unloaded(),
        unloaded(),
        unloaded(),
        unloaded(),
        keyframe(8.),
        frame(9.),
        frame(10.),
        frame(11.),
        keyframe(12.),
        frame(13.),
        frame(14.),
        frame(15.),
        unloaded(),
        unloaded(),
        unloaded(),
        unloaded(),
        keyframe(20.),
        frame(21.),
        frame(22.),
        frame(23.),
    ])
    .unwrap();

    video.expect_decoded_samples([]);

    video.play(0.0..3.0, 1.0).unwrap();
    video.expect_decoded_samples(0..3);

    assert_loading(video.play(4.0..8.0, 1.0));

    video.play(8.0..15.0, 1.0).unwrap();
    video.expect_decoded_samples(8..15);

    video.play(20.0..24.0, 1.0).unwrap();
    video.expect_decoded_samples(20..24);

    // Play & load progressively
    video.play(0.0..3.0, 1.0).unwrap();

    video.set_sample(4, keyframe(4.));
    video.set_sample(5, frame(5.));
    video.set_sample(6, frame(6.));
    video.set_sample(7, frame(7.));

    video.play(4.0..15.0, 1.0).unwrap();

    video.set_sample(16, keyframe(16.));
    video.set_sample(17, frame(17.));

    video.play(16.0..17.0, 1.0).unwrap();

    video.set_sample(18, frame(18.));
    video.set_sample(19, frame(19.));

    video.play(18.0..24.0, 1.0).unwrap();

    video.expect_decoded_samples(0..24);
}

impl TestVideoPlayer {
    fn play_store(
        &mut self,
        range: Range<f64>,
        time_step: f64,
        store: &re_entity_db::EntityDb,
    ) -> Result<(), VideoPlayerError> {
        let storage_engine = store.storage_engine();
        self.play_with_buffer(range, time_step, &|tuid| {
            let buffer = storage_engine
                .store()
                .chunk(&re_chunk::ChunkId::from_tuid(tuid))
                .and_then(|chunk| {
                    let raw = chunk.raw_component_array(
                        re_sdk_types::archetypes::VideoStream::descriptor_sample().component,
                    )?;

                    let (_offsets, buffer) = re_arrow_util::blob_arrays_offsets_and_buffer(raw)?;

                    Some(buffer.as_slice())
                });

            buffer.unwrap_or(&[])
        })
    }
}

fn build_manifest_with_unloaded_chunks(store_id: StoreId, chunks: &[Arc<Chunk>]) -> RrdManifest {
    let mut builder = RrdManifestBuilder::default();
    let mut byte_offset = 0u64;

    for chunk in chunks {
        let arrow_msg = chunk.to_arrow_msg().unwrap();
        let chunk_batch = re_sorbet::ChunkBatch::try_from(&arrow_msg.batch).unwrap();

        let chunk_byte_size = chunk.total_size_bytes();

        let byte_span = re_span::Span {
            start: byte_offset,
            len: chunk_byte_size,
        };

        builder
            .append(&chunk_batch, byte_span, chunk_byte_size)
            .unwrap();

        byte_offset += chunk_byte_size;
    }

    builder.build(store_id).unwrap()
}

const STREAM_ENTITY: &str = "/stream";
const TIMELINE_NAME: &str = "video";

fn unload_chunks(store: &EntityDb, cache: &mut super::VideoStreamCache, keep_range: Range<f64>) {
    let store_events = store.gc(&re_chunk_store::GarbageCollectionOptions {
        target: re_chunk_store::GarbageCollectionTarget::Everything,
        time_budget: std::time::Duration::from_secs(u64::MAX),
        protect_latest: 0,
        protected_time_ranges: std::iter::once((
            re_chunk::TimelineName::new(TIMELINE_NAME),
            AbsoluteTimeRange::new(
                TimeInt::from_secs(keep_range.start),
                TimeInt::from_secs(keep_range.end.next_down()),
            ),
        ))
        .collect(),
        furthest_from: None,
        perform_deep_deletions: false,
    });

    cache.on_store_events(&store_events.iter().collect::<Vec<_>>(), store);
}

fn load_chunks(store: &mut EntityDb, cache: &mut super::VideoStreamCache, chunks: &[Arc<Chunk>]) {
    let mut store_events = Vec::<re_chunk_store::ChunkStoreEvent>::new();

    for chunk in chunks {
        eprintln!("\nload: {}", chunk.id().short_string());
        store_events.extend(store.add_chunk(chunk).unwrap());
    }

    cache.on_store_events(&store_events.iter().collect::<Vec<_>>(), store);
}

fn codec_chunk() -> Chunk {
    let mut builder = Chunk::builder(STREAM_ENTITY);

    builder = builder.with_archetype(
        RowId::new(),
        [(
            Timeline::new_duration(TIMELINE_NAME),
            TimeInt::from_secs(0.0),
        )],
        &VideoStream::new(VideoCodec::AV1),
    );

    builder.build().unwrap()
}

fn video_chunks(
    keyframe: &[u8],
    inter_frame: &[u8],
    start_time: f64,
    dt: f64,
    gop_count: u64,
    samples_per_gop: u64,
) -> Chunk {
    let timeline = Timeline::new_duration(TIMELINE_NAME);
    let mut builder = Chunk::builder(STREAM_ENTITY);

    for i in 0..gop_count {
        let gop_start_time = start_time + (i * samples_per_gop) as f64 * dt;
        builder = builder.with_archetype(
            RowId::new(),
            [(timeline, TimeInt::from_secs(gop_start_time))],
            &VideoStream::update_fields().with_sample(keyframe),
        );

        for i in 1..samples_per_gop {
            let time = gop_start_time + i as f64 * dt;
            builder = builder.with_archetype(
                RowId::new(),
                [(timeline, TimeInt::from_secs(time))],
                &VideoStream::update_fields().with_sample(inter_frame),
            );
        }
    }

    builder.build().unwrap()
}

fn playable_stream(cache: &mut VideoStreamCache, store: &EntityDb) -> SharablePlayableVideoStream {
    cache
        .entry(
            store,
            &re_chunk::EntityPath::from(STREAM_ENTITY),
            TIMELINE_NAME.into(),
            re_video::DecodeSettings {
                hw_acceleration: Default::default(),
                ffmpeg_path: Some(std::path::PathBuf::from("/not/used")),
            },
        )
        .unwrap()
}

fn load_into_rrd_manifest(store: &mut EntityDb, chunks: &[Arc<Chunk>]) {
    let manifest = build_manifest_with_unloaded_chunks(store.store_id().clone(), chunks);

    store.add_rrd_manifest_message(manifest);
}

#[test]
fn cache_with_manifest() {
    let mut cache = VideoStreamCache::default();

    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let chunks: Vec<_> = (0..10)
        .map(|i| {
            video_chunks(
                AV1_TEST_KEYFRAME,
                AV1_TEST_INTER_FRAME,
                i as f64,
                0.25,
                1,
                4,
            )
        })
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

    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let chunks: Vec<_> = (0..10)
        .map(|i| {
            video_chunks(
                AV1_TEST_KEYFRAME,
                AV1_TEST_INTER_FRAME,
                i as f64,
                0.25,
                1,
                4,
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
    load_chunks(&mut store, &mut cache, &chunks[0..10]);

    player.play_store(0.0..10.0, 0.25, &store).unwrap();

    player.expect_decoded_samples(0..40);
}

// Test with more data per chunk to trigger splitting.
const BIG_AV1_KEYFRAME: &[u8] = include_bytes!("big_av1_keyframe");
const BIG_AV1_INTER_FRAME: &[u8] = include_bytes!("big_av1_inter_frame");

#[test]
fn cache_with_streaming_big() {
    let mut cache = VideoStreamCache::default();

    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let chunk_count = 4;
    let gops_per_chunk = 10;
    let samples_per_gop = 200;

    let dt = 0.1;

    let samples_per_chunk = gops_per_chunk * samples_per_gop;
    let sample_count = chunk_count * samples_per_chunk;
    let time_per_chunk = samples_per_chunk as f64 * dt;

    let chunks: Vec<_> = (0..chunk_count)
        .map(|i| {
            video_chunks(
                BIG_AV1_KEYFRAME,
                BIG_AV1_INTER_FRAME,
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

    player.expect_decoded_samples(0..sample_count as SampleIndex);
}

#[test]
fn cache_with_manifest_big() {
    let mut cache = VideoStreamCache::default();

    let mut store = EntityDb::new(StoreId::recording("test", "test"));

    let chunk_count = 4;
    let gops_per_chunk = 10;
    let samples_per_gop = 200;

    let dt = 0.1;
    let samples_per_chunk = gops_per_chunk * samples_per_gop;
    let time_per_chunk = samples_per_chunk as f64 * dt;

    let chunks: Vec<_> = (0..chunk_count)
        .map(|i| {
            video_chunks(
                BIG_AV1_KEYFRAME,
                BIG_AV1_INTER_FRAME,
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

    // Load all sample chunks.
    load_chunks(&mut store, &mut cache, &chunks[1..2]);

    player
        .play_store(time_per_chunk..time_per_chunk * 2.0 - dt, dt, &store)
        .unwrap();

    let samples_per_chunk = samples_per_chunk as usize;
    player.expect_decoded_samples(samples_per_chunk..samples_per_chunk * 2 - 1);
}
