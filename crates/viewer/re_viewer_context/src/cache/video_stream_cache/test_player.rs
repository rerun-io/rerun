use std::{iter::once, ops::Range, sync::Arc};

use crossbeam::channel::{Receiver, Sender};
use re_chunk::{Chunk, RowId, TimeInt, Timeline};
use re_entity_db::EntityDb;
use re_log_types::{AbsoluteTimeRange, StoreId, external::re_tuid::Tuid};
use re_sdk_types::{archetypes::VideoStream, components::VideoCodec};
use re_video::player::{VideoPlayer, VideoPlayerError, VideoSampleDecoder};
use re_video::{
    AV1_TEST_INTER_FRAME, AV1_TEST_KEYFRAME, AsyncDecoder, SampleIndex, SampleMetadataState, Time,
    VideoDataDescription,
};

use crate::{
    Cache as _, SharablePlayableVideoStream, VideoStreamCache, VideoStreamProcessingError,
};

struct TestDecoder {
    sender: re_video::Sender<Result<re_video::Frame, re_video::DecodeError>>,
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
    video: VideoPlayer<()>,
    sample_rx: Receiver<SampleIndex>,
    video_descr: VideoDataDescription,
    video_descr_source: Option<Box<dyn Fn() -> VideoDataDescription>>,
    time: f64,
}

impl TestVideoPlayer {
    fn from_descr(video_descr: VideoDataDescription) -> Self {
        #![expect(clippy::disallowed_methods)] // it's a test
        let (sample_tx, sample_rx) = crossbeam::channel::unbounded();
        let video = VideoPlayer::new_with_decoder(
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
        let received = self.sample_rx.try_iter().collect::<Vec<_>>();
        let expected = samples.into_iter().collect::<Vec<_>>();

        if let Some((e, r)) = expected.iter().zip(received.iter()).find(|(a, b)| a != b) {
            panic!(
                "   Expected: {expected:?}\n   Received: {received:?}\nFirst Issue: expected {e}, got {r}"
            );
        }
    }

    fn set_sample(&mut self, idx: SampleIndex, mut sample: SampleMetadataState) {
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

        super::update_sample_durations(idx..idx + 1, &mut self.video_descr.samples).unwrap();
    }
}

fn unloaded(time: f64) -> SampleMetadataState {
    let time = Time::from_secs(time, re_video::Timescale::NANOSECOND);
    SampleMetadataState::Unloaded {
        source_id: Tuid::new(),
        min_dts: time,
    }
}

/// An inter frame
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

    super::update_sample_durations(0..samples.next_index(), &mut samples)?;

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
        matches!(err, VideoPlayerError::UnloadedSampleData(_)),
        "Expected 'VideoPlayerError::UnloadedSampleData(_)' got '{err}'",
    );
}

#[test]
fn player_with_unloaded() {
    let mut video = create_video([
        keyframe(0.),
        frame(1.),
        frame(2.),
        frame(3.),
        unloaded(4.),
        unloaded(5.),
        unloaded(6.),
        unloaded(7.),
        keyframe(8.),
        frame(9.),
        frame(10.),
        frame(11.),
        keyframe(12.),
        frame(13.),
        frame(14.),
        frame(15.),
        unloaded(16.),
        unloaded(17.),
        unloaded(18.),
        unloaded(19.),
        keyframe(20.),
        frame(21.),
        frame(22.),
        frame(23.),
    ])
    .unwrap();

    video.play(0.0..3.0, 1.0).unwrap();
    video.expect_decoded_samples(0..3);

    assert_loading(video.play(4.0..8.0, 1.0));
    video.expect_decoded_samples(None);

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

#[test]
fn player_fetching_unloaded() {
    let samples = [
        unloaded(0.),
        unloaded(1.),
        frame(2.),
        unloaded(3.),
        keyframe(4.),
        unloaded(5.),
        frame(6.),
        keyframe(7.),
        frame(8.),
        frame(9.),
        frame(10.),
        unloaded(11.),
        frame(12.),
        frame(13.),
        frame(14.),
    ];

    let mut video = create_video(samples.clone()).unwrap();

    let fetched = parking_lot::RwLock::new(Vec::new());
    assert_loading(video.play_with_buffer(2.0..4.0, 1.0, &|source| {
        fetched.write().push(source);

        &[]
    }));
    assert_eq!(fetched.read().as_slice(), &[samples[1].source_id()]);

    video.expect_decoded_samples(None);

    fetched.write().clear();
    assert_loading(video.play_with_buffer(4.0..6.0, 1.0, &|source| {
        fetched.write().push(source);

        &[]
    }));
    assert_eq!(
        fetched.read().as_slice(),
        &[
            // First keyframe at 4.0 from `request_keyframe_before`
            samples[4].source_id(),
            // Then again keyframe at 4.0 when enqueueing it
            samples[4].source_id(),
            // Then unloaded when pre-loading
            samples[5].source_id()
        ]
    );

    video.expect_decoded_samples(std::iter::once(4));

    fetched.write().clear();
    assert_loading(video.play_with_buffer(10.0..12.0, 1.0, &|source| {
        fetched.write().push(source);

        &[]
    }));
    assert_eq!(
        fetched.read().as_slice(),
        &[
            // in `request_keyframe_before`
            samples[7].source_id(),
            samples[8].source_id(),
            samples[9].source_id(),
            samples[10].source_id(),
            // in `enqueue_sample_range`
            samples[7].source_id(),
            samples[8].source_id(),
            samples[9].source_id(),
            samples[10].source_id(),
            // Then unloaded when pre-loading
            samples[11].source_id(),
        ]
    );

    video.expect_decoded_samples(7..11);

    fetched.write().clear();
    assert_loading(video.play_with_buffer(12.0..14.0, 1.0, &|source| {
        let i = samples
            .iter()
            .position(|c| c.source_id() == source)
            .unwrap();
        eprintln!(
            "\n#{i}\n{}",
            std::backtrace::Backtrace::capture()
                .to_string()
                .lines()
                .filter(|l| l.contains("player"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        fetched.write().push(source);

        &[]
    }));
    assert_eq!(
        fetched.read().as_slice(),
        // Both in `request_keyframe_before`.
        &[samples[11].source_id(), samples[12].source_id()]
    );

    video.expect_decoded_samples(None);
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
                .physical_chunk(&re_chunk::ChunkId::from_tuid(tuid))
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

const STREAM_ENTITY: &str = "/stream";
const TIMELINE_NAME: &str = "video";

#[track_caller]
fn unload_chunks(store: &EntityDb, cache: &mut super::VideoStreamCache, keep_range: Range<f64>) {
    let loaded_chunks_before = store.storage_engine().store().num_physical_chunks();
    let store_events = store.gc(&re_chunk_store::GarbageCollectionOptions {
        target: re_chunk_store::GarbageCollectionTarget::Everything,
        time_budget: std::time::Duration::from_secs(u64::MAX),
        protect_latest: 0,
        protected_chunks: Default::default(),
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

    let loaded_chunks_after = store.storage_engine().store().num_physical_chunks();

    assert!(
        loaded_chunks_before > loaded_chunks_after,
        "Expected some chunks to be gc'd"
    );

    cache.on_store_events(&store_events.iter().collect::<Vec<_>>(), store);
}

fn load_chunks(store: &mut EntityDb, cache: &mut super::VideoStreamCache, chunks: &[Arc<Chunk>]) {
    let mut store_events = Vec::<re_chunk_store::ChunkStoreEvent>::new();

    for chunk in chunks {
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

fn video_chunk(start_time: f64, dt: f64, gop_count: u64, samples_per_gop: u64) -> Chunk {
    let timeline = Timeline::new_duration(TIMELINE_NAME);
    let mut builder = Chunk::builder(STREAM_ENTITY);

    for i in 0..gop_count {
        let gop_start_time = start_time + (i * samples_per_gop) as f64 * dt;
        builder = builder.with_archetype(
            RowId::new(),
            [(timeline, TimeInt::from_secs(gop_start_time))],
            &VideoStream::update_fields().with_sample(AV1_TEST_KEYFRAME),
        );

        for i in 1..samples_per_gop {
            let time = gop_start_time + i as f64 * dt;
            builder = builder.with_archetype(
                RowId::new(),
                [(timeline, TimeInt::from_secs(time))],
                &VideoStream::update_fields().with_sample(AV1_TEST_INTER_FRAME),
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
    let manifest = re_log_encoding::RrdManifest::build_in_memory_from_chunks(
        store.store_id().clone(),
        chunks.iter().map(|c| &**c),
    )
    .unwrap();

    store.add_rrd_manifest_message(manifest);
}

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

    // Try dropping chunks at the end.
    unload_chunks(&store, &mut cache, 15.0..20.0);

    player.play_store(15.0..20.0 - dt, dt, &store).unwrap();

    player.expect_decoded_samples(60..80);
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

    assert_loading(player.play_store(0.0..3.0, 0.25, &store));
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

#[track_caller]
fn assert_splits_happened(store: &EntityDb) {
    let engine = store.storage_engine();
    let store = engine.store();

    assert!(
        store
            .iter_physical_chunks()
            .any(|c| { store.descends_from_a_split(&c.id()) }),
        "This test is testing how the video cache handles splits, but no split happened"
    );
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

    player.expect_decoded_samples(0..sample_count as SampleIndex);

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

/// Test that chunks arriving out of temporal order are handled correctly
/// via delta re-merge (the `handle_out_of_order_chunk` path).
///
/// Loads chunks in non-chronological order so that a later-arriving chunk
/// has timestamps that fall before existing samples, triggering the
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
            .0
            .contains_key(&crate::cache::video_stream_cache::VideoStreamKey {
                entity_path: re_chunk::EntityPath::from(STREAM_ENTITY).hash(),
                timeline: re_chunk::TimelineName::new(TIMELINE_NAME)
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

/// Test for out-of-order chunk arrival that should not trigger a cache reset,
/// followed by compaction.
///
/// This tests for the scenario where a `ChunkSampleRange` has
/// less samples than the amount of samples it spans.
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
            .0
            .contains_key(&crate::cache::video_stream_cache::VideoStreamKey {
                entity_path: re_chunk::EntityPath::from(STREAM_ENTITY).hash(),
                timeline: re_chunk::TimelineName::new(TIMELINE_NAME)
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
