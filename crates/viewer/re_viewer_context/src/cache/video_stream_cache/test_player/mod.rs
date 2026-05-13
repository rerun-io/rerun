mod encoded_depth_image;
mod encoded_image;
mod video_stream;

use std::{iter::once, ops::Range, sync::Arc};

use crossbeam::channel::{Receiver, Sender};
use re_chunk::{Chunk, RowId, TimeInt, Timeline};
use re_entity_db::EntityDb;
use re_log_types::{AbsoluteTimeRange, external::re_tuid::Tuid};
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components::VideoCodec;
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
        re_quota_channel::send_crossbeam(&self.sample_tx, chunk.sample_idx).unwrap();

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

pub(super) struct TestVideoPlayer {
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
        // The fake decoder doesn't inspect the bytes, but `SampleMetadata::get`
        // returns `None` for empty buffers (treats it as "sample not found"),
        // so hand back a one-byte placeholder.
        self.play_with_buffer(range, time_step, &|_| &[0])
    }

    fn play_with_buffer<'a>(
        &mut self,
        range: Range<f64>,
        time_step: f64,
        get_buffer: &dyn Fn(re_video::VideoSource) -> &'a [u8],
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
        get_buffer: &dyn Fn(re_video::VideoSource) -> &'a [u8],
    ) -> Result<(), VideoPlayerError> {
        self.video.frame_at(
            Time::from_secs(time, re_video::Timescale::NANOSECOND),
            &self.video_descr,
            &mut |(), _| Ok(()),
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
        source: re_video::VideoSource::id(Tuid::new(), Tuid::new()),
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
        source: re_video::VideoSource::id(Tuid::new(), Tuid::new()),
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
pub(super) fn assert_loading(err: Result<(), VideoPlayerError>) {
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
        fetched.write().push(source.primary_id());

        &[0]
    }));
    assert_eq!(
        fetched.read().as_slice(),
        &[
            samples[2].source_primary_id(),
            samples[1].source_primary_id()
        ]
    );

    video.expect_decoded_samples(None);

    fetched.write().clear();
    assert_loading(video.play_with_buffer(4.0..6.0, 1.0, &|source| {
        fetched.write().push(source.primary_id());

        &[0]
    }));
    assert_eq!(
        fetched.read().as_slice(),
        &[
            // First keyframe at 4.0 from `request_keyframe_before`
            samples[4].source_primary_id(),
            // Then again keyframe at 4.0 when enqueueing it
            samples[4].source_primary_id(),
            // Then unloaded when pre-loading
            samples[5].source_primary_id()
        ]
    );

    video.expect_decoded_samples(std::iter::once(4));

    fetched.write().clear();
    assert_loading(video.play_with_buffer(10.0..12.0, 1.0, &|source| {
        fetched.write().push(source.primary_id());

        &[0]
    }));
    assert_eq!(
        fetched.read().as_slice(),
        &[
            // in `request_keyframe_before` (reversed)
            samples[10].source_primary_id(),
            samples[9].source_primary_id(),
            samples[8].source_primary_id(),
            samples[7].source_primary_id(),
            // in `enqueue_sample_range`
            samples[7].source_primary_id(),
            samples[8].source_primary_id(),
            samples[9].source_primary_id(),
            samples[10].source_primary_id(),
            // Then unloaded when pre-loading
            samples[11].source_primary_id(),
        ]
    );

    video.expect_decoded_samples(7..11);

    fetched.write().clear();
    assert_loading(video.play_with_buffer(12.0..14.0, 1.0, &|source| {
        let primary_id = source.primary_id();
        let i = samples
            .iter()
            .position(|c| c.source_primary_id() == primary_id)
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
        fetched.write().push(primary_id);

        &[0]
    }));
    assert_eq!(
        fetched.read().as_slice(),
        // Both in `request_keyframe_before` (reversed).
        &[
            samples[12].source_primary_id(),
            samples[11].source_primary_id()
        ]
    );

    video.expect_decoded_samples(None);
}

impl TestVideoPlayer {
    pub(super) fn play_store(
        &mut self,
        range: Range<f64>,
        time_step: f64,
        store: &re_entity_db::EntityDb,
    ) -> Result<(), VideoPlayerError> {
        self.play_store_with_component(
            range,
            time_step,
            store,
            re_sdk_types::archetypes::VideoStream::descriptor_sample().component,
        )
    }

    pub(super) fn play_store_with_component(
        &mut self,
        range: Range<f64>,
        time_step: f64,
        store: &re_entity_db::EntityDb,
        sample_component: re_sdk_types::ComponentIdentifier,
    ) -> Result<(), VideoPlayerError> {
        let storage_engine = store.storage_engine();
        let lookup = |source: re_video::VideoSource| -> Option<&[u8]> {
            let re_video::VideoSource::Id {
                id,
                sub_id: Some(sub_id),
            } = source
            else {
                return None;
            };
            let chunk = storage_engine
                .store()
                .physical_chunk(&re_chunk::ChunkId::from_tuid(id))?;
            let raw = chunk.raw_component_array(sample_component)?;
            let (offsets, buffer) = re_arrow_util::blob_arrays_offsets_and_buffer(raw)?;

            let row_idx = chunk.row_index_of(re_chunk::RowId::from_tuid(sub_id))?;
            let start = offsets[row_idx] as usize;
            let end = offsets[row_idx + 1] as usize;
            Some(&buffer.as_slice()[start..end])
        };
        self.play_with_buffer(range, time_step, &|source| lookup(source).unwrap_or(&[]))
    }
}

pub(super) const STREAM_ENTITY: &str = "/stream";
pub(super) const TIMELINE_NAME: &str = "video";

#[track_caller]
pub(super) fn unload_chunks(
    store: &EntityDb,
    cache: &mut super::VideoStreamCache,
    keep_range: Range<f64>,
) {
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

pub(super) fn load_chunks(
    store: &mut EntityDb,
    cache: &mut super::VideoStreamCache,
    chunks: &[Arc<Chunk>],
) {
    let mut store_events = Vec::<re_chunk_store::ChunkStoreEvent>::new();

    for chunk in chunks {
        store_events.extend(store.add_chunk(chunk).unwrap());
    }

    cache.on_store_events(&store_events.iter().collect::<Vec<_>>(), store);
}

pub(super) fn codec_chunk() -> Chunk {
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

pub(super) fn video_chunk(start_time: f64, dt: f64, gop_count: u64, samples_per_gop: u64) -> Chunk {
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

pub(super) fn playable_stream(
    cache: &mut VideoStreamCache,
    store: &EntityDb,
) -> SharablePlayableVideoStream {
    cache
        .video_entry(
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

pub(super) fn load_into_rrd_manifest(store: &mut EntityDb, chunks: &[Arc<Chunk>]) {
    let manifest = re_log_encoding::RrdManifest::build_in_memory_from_chunks(
        store.store_id().clone(),
        chunks.iter().map(|c| &**c),
    )
    .unwrap();

    store.add_rrd_manifest_message(manifest);
}

#[track_caller]
pub(super) fn assert_splits_happened(store: &EntityDb) {
    let engine = store.storage_engine();
    let store = engine.store();

    assert!(
        store
            .iter_physical_chunks()
            .any(|c| { store.descends_from_a_split(&c.id()) }),
        "This test is testing how the video cache handles splits, but no split happened"
    );
}
