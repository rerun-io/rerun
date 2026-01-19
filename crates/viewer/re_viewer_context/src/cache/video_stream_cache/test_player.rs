use std::{iter::once, ops::Range};

use re_log_types::external::re_tuid::Tuid;
use re_renderer::video::{
    InsufficientSampleDataError, VideoPlayer, VideoPlayerError, VideoSampleDecoder,
};
use re_video::{
    AsyncDecoder, Receiver, SampleIndex, SampleMetadataState, Sender, Time, VideoDataDescription,
};

use crate::VideoStreamProcessingError;

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
    time: f64,
}

impl TestVideoPlayer {
    fn play(&mut self, range: Range<f64>, time_step: f64) -> Result<(), VideoPlayerError> {
        self.time = range.start;
        for i in 0..((range.end - self.time) / time_step).next_down().floor() as i32 {
            let time = self.time + i as f64 * time_step;
            self.frame_at(time)?;
        }

        self.time = range.end;

        Ok(())
    }

    fn frame_at(&mut self, time: f64) -> Result<(), VideoPlayerError> {
        self.video.frame_at(
            Time::from_secs(time, re_video::Timescale::NANOSECOND),
            &self.video_descr,
            &mut |_, _| Ok(()),
            &|_| &[],
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

fn skip() -> SampleMetadataState {
    SampleMetadataState::Skip(Tuid::new())
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
    let (sample_tx, sample_rx) = re_video::channel("test_player");
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

    Ok(TestVideoPlayer {
        video,
        sample_rx,
        video_descr,
        time: 0.0,
    })
}

fn test_simple_video(mut video: TestVideoPlayer, count: usize, dt: f64, max_time: f64) {
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

#[test]
fn player_with_skips() {
    let samples = [
        keyframe(0.0),
        frame(0.1),
        frame(0.2),
        skip(),
        keyframe(0.3),
        frame(0.4),
        skip(),
        skip(),
        keyframe(0.5),
        frame(0.6),
        skip(),
        skip(),
        skip(),
        frame(0.7),
        skip(),
        frame(0.8),
        skip(),
        keyframe(0.9),
        skip(),
        skip(),
        frame(1.0),
        skip(),
        skip(),
    ];

    let expected_indices: Vec<_> = samples
        .iter()
        .enumerate()
        .filter(|(_, s)| s.sample().is_some())
        .map(|(idx, _)| idx)
        .collect();

    let mut video = create_video(samples).unwrap();

    video.play(0.0..1.0, 0.1).unwrap();

    video.expect_decoded_samples(expected_indices);
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
