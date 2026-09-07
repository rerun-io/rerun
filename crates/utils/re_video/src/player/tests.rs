use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::{AsyncDecoder, Chunk, DecodeError, Frame, FrameContent, FrameResult, Sender};

struct TestDecoder {
    output: Sender<FrameResult>,
    capacity: Arc<AtomicUsize>,
    resets: Arc<AtomicUsize>,
}

impl AsyncDecoder for TestDecoder {
    fn submit_chunk(&mut self, chunk: Chunk) -> crate::DecodeResult<()> {
        if self
            .capacity
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |capacity| {
                capacity.checked_sub(1)
            })
            .is_err()
        {
            return Err(DecodeError::Stalling);
        }
        self.output
            .send(Ok(Frame {
                content: FrameContent::Decoded(crate::DecodedFrameContent {
                    data: vec![0],
                    width: 1,
                    height: 1,
                    format: crate::PixelFormat::L8,
                }),
                info: FrameInfo {
                    is_sync: Some(chunk.is_sync),
                    frame_nr: Some(chunk.frame_nr),
                    source: Some(chunk.source),
                    presentation_timestamp: chunk.presentation_timestamp,
                    duration: chunk.duration,
                    latest_decode_timestamp: Some(chunk.decode_timestamp),
                },
            }))
            .unwrap();
        Ok(())
    }

    fn reset(&mut self, _description: &crate::VideoDataDescription) -> crate::DecodeResult<()> {
        self.resets.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

/// The player collects completed frames while its decoder input is full.
/// Once capacity becomes available, enqueueing resumes at the first unaccepted sample without a reset.
#[test]
fn stalling_preserves_enqueue_progress_and_processes_output() {
    let capacity = Arc::new(AtomicUsize::new(2));
    let resets = Arc::new(AtomicUsize::new(0));
    let decoder = VideoSampleDecoder::new("stalling test".to_owned(), |output| {
        Ok(Box::new(TestDecoder {
            output,
            capacity: capacity.clone(),
            resets: resets.clone(),
        }))
    })
    .unwrap();
    let mut player = VideoPlayer::<usize>::new_with_decoder(decoder);
    let samples = (0..8)
        .map(|index| {
            crate::SampleMetadataState::Present(crate::SampleMetadata {
                is_sync: index == 0,
                frame_nr: index,
                decode_timestamp: Time(i64::from(index)),
                presentation_timestamp: Time(i64::from(index)),
                duration: Some(Time(1)),
                source: VideoSource::Span(Span::from_start_len(u64::from(index), 1)),
            })
        })
        .collect();
    let description = crate::VideoDataDescription {
        codec: crate::VideoCodec::H264,
        encoding_details: None,
        timescale: None,
        delivery_method: VideoDeliveryMethod::new_stream(),
        keyframe_indices: vec![0],
        samples_statistics: crate::SamplesStatistics::new(&samples),
        samples,
        mp4_tracks: Default::default(),
    };
    let source = VideoSliceSource(&[0; 8]);
    let mut update = |output: &mut usize, frame: &Frame| {
        *output = frame.info.frame_nr.unwrap() as usize;
        Ok(())
    };

    let status = player
        .frame_at(Time(0), &description, &mut update, &source)
        .unwrap();
    assert_eq!(status.frame_info.unwrap().frame_nr, Some(0));
    assert_eq!(player.last_enqueued(), Some(1));
    let initial_resets = resets.load(Ordering::Relaxed);

    player
        .frame_at(Time(0), &description, &mut update, &source)
        .unwrap();
    assert_eq!(player.last_enqueued(), Some(1));
    assert_eq!(resets.load(Ordering::Relaxed), initial_resets);

    capacity.store(2, Ordering::Relaxed);
    let status = player
        .frame_at(Time(3), &description, &mut update, &source)
        .unwrap();
    assert_eq!(status.frame_info.unwrap().frame_nr, Some(3));
    assert_eq!(player.last_enqueued(), Some(3));
    assert_eq!(player.last_requested(), Some(3));
    assert_eq!(resets.load(Ordering::Relaxed), initial_resets);
    assert!(player.last_error.is_none());
}
