use std::sync::Arc;

use re_mutex::Mutex;

use super::*;
use crate::{
    AsyncDecoder, Chunk, DecodeResult, DecodedFrameContent, Frame, FrameInfo, FrameResult,
    PixelFormat, SampleMetadata, SampleMetadataState, SamplesStatistics, Sender, StableIndexDeque,
    Timescale, VideoCodec, VideoDataDescription,
};

#[derive(Default)]
struct DecoderState {
    pending: Vec<Chunk>,
    resets: usize,
}

struct ManualDecoder(Arc<Mutex<DecoderState>>);

impl AsyncDecoder for ManualDecoder {
    fn submit_chunk(&mut self, chunk: Chunk) -> DecodeResult<()> {
        self.0.lock().pending.push(chunk);
        Ok(())
    }

    fn reset(&mut self, _description: &VideoDataDescription) -> DecodeResult<()> {
        let mut state = self.0.lock();
        state.pending.clear();
        state.resets += 1;
        Ok(())
    }
}

struct TestPlayer {
    player: VideoPlayer<Option<Time>>,
    description: VideoDataDescription,
    decoder: Arc<Mutex<DecoderState>>,
    decoded_frames: Sender<FrameResult>,
}

impl TestPlayer {
    fn new() -> Self {
        let decoder = Arc::new(Mutex::new(DecoderState::default()));
        let mut decoded_frames = None;
        let sample_decoder = VideoSampleDecoder::new("test".to_owned(), |sender| {
            decoded_frames = Some(sender);
            Ok(Box::new(ManualDecoder(Arc::clone(&decoder))))
        })
        .expect("manual decoder creation succeeds");
        Self {
            player: VideoPlayer::new_with_decoder(sample_decoder),
            description: VideoDataDescription {
                codec: VideoCodec::ImageSequence(Some("image/jpeg".into())),
                encoding_details: None,
                timescale: Some(Timescale::NANOSECOND),
                delivery_method: VideoDeliveryMethod::new_stream(),
                keyframe_indices: Vec::new(),
                samples: StableIndexDeque::new(),
                samples_statistics: SamplesStatistics::new(&StableIndexDeque::new()),
                mp4_tracks: Default::default(),
            },
            decoder,
            decoded_frames: decoded_frames.expect("decoder creation captures the output sender"),
        }
    }

    fn append_sample(&mut self, is_sync: bool) {
        let index = self.description.samples.next_index();
        if let Some(previous) = self
            .description
            .samples
            .back_mut()
            .and_then(|s| s.sample_mut())
        {
            previous.duration = Some(Time(1));
        }
        if is_sync {
            self.description.keyframe_indices.push(index);
        }
        let timestamp = Time(i64::try_from(index).expect("test index fits in i64"));
        self.description
            .samples
            .push_back(SampleMetadataState::Present(SampleMetadata {
                is_sync,
                frame_nr: u32::try_from(index).expect("test index fits in u32"),
                decode_timestamp: timestamp,
                presentation_timestamp: timestamp,
                duration: None,
                source: VideoSource::Span(Span::from_start_end(0, 1)),
            }));
        self.description.samples_statistics = SamplesStatistics::new(&self.description.samples);
        self.description.delivery_method = VideoDeliveryMethod::new_stream();
    }

    // Complete decoding between player updates, without threads or timing assumptions.
    fn finish_decoding(&self) {
        for chunk in std::mem::take(&mut self.decoder.lock().pending) {
            self.decoded_frames
                .send(Ok(Frame {
                    content: DecodedFrameContent {
                        data: vec![0],
                        width: 1,
                        height: 1,
                        format: PixelFormat::L8,
                    },
                    info: FrameInfo {
                        is_sync: Some(chunk.is_sync),
                        frame_nr: Some(chunk.frame_nr),
                        source: Some(chunk.source),
                        presentation_timestamp: chunk.presentation_timestamp,
                        duration: chunk.duration,
                        latest_decode_timestamp: Some(chunk.decode_timestamp),
                    },
                }))
                .expect("player is still receiving decoded frames");
        }
    }

    fn frame_at(&mut self, time: i64) -> PlayerFrameStatus {
        self.player
            .frame_at(
                Time(time),
                &self.description,
                &mut |output, frame| {
                    *output = Some(frame.info.presentation_timestamp);
                    Ok(())
                },
                &VideoSliceSource(&[0]),
            )
            .expect("test samples can be played")
    }

    fn displayed_time(&self) -> Option<Time> {
        self.player.output().copied().flatten()
    }

    fn warm_up(&mut self) {
        self.append_sample(true);
        self.frame_at(0);
        self.finish_decoding();
        self.frame_at(0);
        assert_eq!(self.displayed_time(), Some(Time(0)));
    }
}

#[test]
fn consecutive_live_keyframes_preserve_decoded_output() {
    let mut test = TestPlayer::new();
    test.warm_up();
    let initial_resets = test.decoder.lock().resets;
    for time in 1..32 {
        test.append_sample(true);
        let status = test.frame_at(time);
        assert_eq!(test.displayed_time(), Some(Time(time - 1)));
        assert!(!status.show_loading_indicator);
        assert_eq!(test.decoder.lock().resets, initial_resets);
        test.finish_decoding();
    }
    test.frame_at(31);
    assert_eq!(test.displayed_time(), Some(Time(31)));
}

#[test]
fn growing_live_gops_preserve_decoded_output() {
    let mut test = TestPlayer::new();
    test.description.codec = VideoCodec::AV1;
    test.warm_up();
    let initial_resets = test.decoder.lock().resets;
    for time in 1..32 {
        test.append_sample(time % 3 == 0);
        let status = test.frame_at(time);
        assert_eq!(test.displayed_time(), Some(Time(time - 1)));
        assert!(!status.show_loading_indicator);
        assert_eq!(test.decoder.lock().resets, initial_resets);
        test.finish_decoding();
    }
}

#[test]
fn skipping_unqueued_keyframes_resets_decoder() {
    let mut test = TestPlayer::new();
    test.warm_up();
    let initial_resets = test.decoder.lock().resets;
    test.append_sample(true);
    test.append_sample(true);
    test.frame_at(2);
    assert!(test.decoder.lock().resets > initial_resets);
    test.finish_decoding();
    test.frame_at(2);
    assert_eq!(test.displayed_time(), Some(Time(2)));
}

#[test]
fn backward_seek_resets_decoder() {
    let mut test = TestPlayer::new();
    test.warm_up();
    test.append_sample(true);
    test.frame_at(1);
    test.finish_decoding();
    test.frame_at(1);
    assert_eq!(test.displayed_time(), Some(Time(1)));
    let initial_resets = test.decoder.lock().resets;
    test.frame_at(0);
    assert!(test.decoder.lock().resets > initial_resets);
    test.finish_decoding();
    test.frame_at(0);
    assert_eq!(test.displayed_time(), Some(Time(0)));
}
