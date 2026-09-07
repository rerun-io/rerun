use super::*;
use crate::{Time, VideoCodec, VideoDeliveryMethod, VideoSource};

fn chunk(sample_idx: usize) -> Chunk {
    Chunk {
        sample_idx,
        frame_nr: sample_idx as _,
        is_sync: true,
        data: Vec::new(),
        source: VideoSource::Span(re_span::Span::from_start_len(sample_idx as u64, 1)),
        decode_timestamp: Time(i64::try_from(sample_idx).unwrap()),
        presentation_timestamp: Time(i64::try_from(sample_idx).unwrap()),
        duration: None,
    }
}

fn description() -> VideoDataDescription {
    VideoDataDescription {
        codec: VideoCodec::H264,
        encoding_details: None,
        timescale: None,
        delivery_method: VideoDeliveryMethod::new_stream(),
        keyframe_indices: Vec::new(),
        samples: crate::StableIndexDeque::new(),
        samples_statistics: crate::SamplesStatistics::new(&crate::StableIndexDeque::new()),
        mp4_tracks: Default::default(),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Chunk(usize),
    Reset,
    EndOfVideo,
}

struct TestDecoder {
    ready: Arc<AtomicBool>,
    events: Sender<Event>,
    polled: Sender<()>,
}

impl SyncDecoder for TestDecoder {
    fn submit_chunk(
        &mut self,
        _should_stop: &AtomicBool,
        chunk: Chunk,
        _output: &crate::Sender<FrameResult>,
    ) {
        self.events
            .try_send(Event::Chunk(chunk.sample_idx))
            .unwrap();
    }

    fn reset(&mut self, _description: &VideoDataDescription) {
        self.events.try_send(Event::Reset).unwrap();
    }

    fn end_of_video(&mut self, _output: &crate::Sender<FrameResult>) {
        self.events.try_send(Event::EndOfVideo).unwrap();
    }
}

impl WorkerDecoder for TestDecoder {
    fn poll(&mut self, _output: &crate::Sender<FrameResult>) -> Result<bool> {
        let ready = self.ready.load(Ordering::Acquire);
        let _send_error = self.polled.try_send(());
        Ok(ready)
    }
}

fn setup() -> (GpuDecoderWorker, Receiver<Input>, ControlReceiver) {
    let (input, input_rx) = bounded(INPUT_CAPACITY);
    let control = Arc::new(Mutex::new(Control::default()));
    let (wake, wake_rx) = bounded(1);
    let control_rx = ControlReceiver {
        pending: control.clone(),
        wake: wake_rx,
    };
    (
        GpuDecoderWorker {
            input,
            control,
            wake,
            should_stop: Arc::new(AtomicBool::new(false)),
            generation: 0,
            sequence: 0,
        },
        input_rx,
        control_rx,
    )
}

/// A full input channel rejects the next sample without advancing its sequence.
/// Control messages remain available and the sample can be accepted after capacity is released.
#[test]
fn full_input_reports_stalling() {
    let (mut worker, input, control) = setup();
    for index in 0..INPUT_CAPACITY {
        worker.submit_chunk(chunk(index)).unwrap();
    }
    assert!(matches!(
        worker.submit_chunk(chunk(INPUT_CAPACITY)),
        Err(DecodeError::Stalling)
    ));
    assert_eq!(worker.sequence, INPUT_CAPACITY as u64);
    worker.end_of_video().unwrap();
    assert!(control.take().end_of_video.is_some());
    worker.reset(&description()).unwrap();
    assert!(control.take().reset.is_some());
    input.try_recv().unwrap();
    worker.submit_chunk(chunk(INPUT_CAPACITY)).unwrap();
    assert_eq!(worker.sequence, INPUT_CAPACITY as u64 + 1);
    drop(worker);
    assert!(control.take().stop);
}

/// A full wake channel keeps the latest reset and its end-of-video request in shared state.
/// Stop remains recorded even when a wake is already queued.
#[test]
fn controls_remain_available_when_wake_channel_is_full() {
    let (mut worker, _input, control) = setup();
    worker.reset(&description()).unwrap();
    worker.end_of_video().unwrap();
    worker.reset(&description()).unwrap();
    assert_eq!(control.wake.len(), 1);
    assert!(worker.control.lock().end_of_video.is_none());
    worker.end_of_video().unwrap();
    let generation = worker.generation;
    let sequence = worker.sequence;
    drop(worker);
    assert_eq!(control.wake.len(), 1);
    let pending = control.take();
    assert_eq!(pending.reset.unwrap().0, generation);
    assert_eq!(pending.end_of_video, Some((generation, sequence)));
    assert!(pending.stop);
    assert!(control.wake.is_empty());
}

/// Pending samples stay in the input channel while the decoder has no capacity.
/// The end-of-video command waits for every accepted sample to be submitted.
#[test]
fn capacity_and_end_of_video_ordering() {
    let (mut worker, input, control) = setup();
    for index in 0..INPUT_CAPACITY {
        worker.submit_chunk(chunk(index)).unwrap();
    }
    worker.end_of_video().unwrap();
    let ready = Arc::new(AtomicBool::new(false));
    let (events, received) = bounded(16);
    let (polled, polls) = bounded(1);
    let mut decoder = TestDecoder {
        ready: ready.clone(),
        events,
        polled,
    };
    let (output, _output_rx) = crate::channel("worker test");
    let stop = worker.should_stop.clone();
    std::thread::scope(|scope| {
        scope.spawn(|| run(&mut decoder, &input, &control, &stop, &output));
        polls.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(input.len(), INPUT_CAPACITY);
        assert!(received.is_empty());
        ready.store(true, Ordering::Release);
        for index in 0..INPUT_CAPACITY {
            assert_eq!(
                received.recv_timeout(Duration::from_secs(5)).unwrap(),
                Event::Chunk(index)
            );
        }
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::EndOfVideo
        );
        drop(worker);
    });
}

/// Reset is handled while the input channel is full and the decoder has no capacity.
/// Samples accepted before the reset are discarded and newer samples are decoded.
#[test]
fn reset_while_full() {
    let (mut worker, input, control) = setup();
    for index in 0..INPUT_CAPACITY {
        worker.submit_chunk(chunk(index)).unwrap();
    }
    worker.end_of_video().unwrap();
    worker.reset(&description()).unwrap();
    let ready = Arc::new(AtomicBool::new(false));
    let (events, received) = bounded(16);
    let (polled, _polls) = bounded(1);
    let mut decoder = TestDecoder {
        ready: ready.clone(),
        events,
        polled,
    };
    let (output, _output_rx) = crate::channel("worker reset test");
    let stop = worker.should_stop.clone();
    std::thread::scope(|scope| {
        scope.spawn(|| run(&mut decoder, &input, &control, &stop, &output));
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::Reset
        );
        assert_eq!(input.len(), INPUT_CAPACITY);
        ready.store(true, Ordering::Release);
        // A control message records when all accepted input has been handled.
        worker.end_of_video().unwrap();
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::EndOfVideo
        );
        worker.reset(&description()).unwrap();
        worker.submit_chunk(chunk(INPUT_CAPACITY)).unwrap();
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::Reset
        );
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::Chunk(INPUT_CAPACITY)
        );
        drop(worker);
    });
}
