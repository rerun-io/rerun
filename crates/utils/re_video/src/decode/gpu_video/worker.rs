use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossbeam::channel::{Receiver, Sender, TrySendError, bounded};
use re_mutex::Mutex;

use super::{GpuSyncDecoder, SyncDecoder};
use crate::VideoDataDescription;
use crate::decode::{AsyncDecoder, Chunk, DecodeError, FrameResult, Result};

#[cfg(test)]
mod tests;

const INPUT_CAPACITY: usize = 4;

struct Input {
    generation: u64,
    sequence: u64,
    chunk: Chunk,
}

#[derive(Default)]
struct Control {
    reset: Option<(u64, Box<VideoDataDescription>)>,
    end_of_video: Option<(u64, u64)>,
    stop: bool,
}

struct ControlReceiver {
    pending: Arc<Mutex<Control>>,
    wake: Receiver<()>,
}

impl ControlReceiver {
    fn take(&self) -> Control {
        // Consume wake messages before taking the pending state.
        while self.wake.try_recv().is_ok() {}
        std::mem::take(&mut *self.pending.lock())
    }
}

/// Keeps accepted input in a bounded channel until GPU frame slots are available.
pub(super) struct GpuDecoderWorker {
    input: Sender<Input>,
    control: Arc<Mutex<Control>>,
    wake: Sender<()>,
    should_stop: Arc<AtomicBool>,
    generation: u64,
    sequence: u64,
}

impl GpuDecoderWorker {
    pub(super) fn new(
        debug_name: String,
        mut decoder: GpuSyncDecoder,
        output: crate::Sender<FrameResult>,
    ) -> Self {
        let (input, input_rx) = bounded(INPUT_CAPACITY);
        let control = Arc::new(Mutex::new(Control::default()));
        let (wake, wake_rx) = bounded(1);
        let control_rx = ControlReceiver {
            pending: control.clone(),
            wake: wake_rx,
        };
        let should_stop = Arc::new(AtomicBool::new(false));
        let stop = should_stop.clone();
        std::thread::Builder::new()
            .name(format!("decoder of {debug_name}"))
            .spawn(move || {
                econtext::econtext_data!("Video", debug_name);
                run(&mut decoder, &input_rx, &control_rx, &stop, &output);
            })
            .expect("failed to spawn decoder thread");
        Self {
            input,
            control,
            wake,
            should_stop,
            generation: 0,
            sequence: 0,
        }
    }
}

impl AsyncDecoder for GpuDecoderWorker {
    fn submit_chunk(&mut self, chunk: Chunk) -> Result<()> {
        let sequence = self.sequence + 1;
        match self.input.try_send(Input {
            generation: self.generation,
            sequence,
            chunk,
        }) {
            Ok(()) => {
                self.sequence = sequence;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(DecodeError::Stalling),
            Err(TrySendError::Disconnected(_)) => Ok(()),
        }
    }

    fn reset(&mut self, description: &VideoDataDescription) -> Result<()> {
        self.generation += 1;
        {
            let mut control = self.control.lock();
            control.reset = Some((self.generation, Box::new(description.clone())));
            control.end_of_video = None;
        }
        let _send_error = self.wake.try_send(());
        Ok(())
    }

    fn end_of_video(&mut self) -> Result<()> {
        self.control.lock().end_of_video = Some((self.generation, self.sequence));
        let _send_error = self.wake.try_send(());
        Ok(())
    }
}

impl Drop for GpuDecoderWorker {
    fn drop(&mut self) {
        self.should_stop.store(true, Ordering::Release);
        self.control.lock().stop = true;
        let _send_error = self.wake.try_send(());
    }
}

trait WorkerDecoder: SyncDecoder {
    fn poll(&mut self, output: &crate::Sender<FrameResult>) -> Result<bool>;
}

impl WorkerDecoder for GpuSyncDecoder {
    fn poll(&mut self, output: &crate::Sender<FrameResult>) -> Result<bool> {
        let (ready, frames) = self
            .decoder
            .poll()
            .map_err(|err| DecodeError::GpuVideo(Arc::new(err)))?;
        self.emit_frames(frames, output);
        Ok(ready)
    }
}

#[derive(Default)]
struct WorkerState {
    generation: u64,
    sequence: u64,
    end_of_video: Option<u64>,
    failed: bool,
}

impl WorkerState {
    fn controls(&mut self, decoder: &mut impl WorkerDecoder, control: &ControlReceiver) -> bool {
        let pending = control.take();
        if pending.stop {
            return false;
        }
        if let Some((generation, description)) = pending.reset {
            decoder.reset(&description);
            self.generation = generation;
            self.end_of_video = None;
            self.failed = false;
        }
        if let Some((generation, sequence)) = pending.end_of_video
            && generation == self.generation
        {
            self.end_of_video = Some(sequence);
        }
        true
    }
}

fn run(
    decoder: &mut impl WorkerDecoder,
    input: &Receiver<Input>,
    control: &ControlReceiver,
    should_stop: &AtomicBool,
    output: &crate::Sender<FrameResult>,
) {
    let mut state = WorkerState::default();
    loop {
        if should_stop.load(Ordering::Acquire) {
            return;
        }

        // Control messages are handled before accepting more input.
        if !state.controls(decoder, control) {
            return;
        }

        let ready = !state.failed
            && match decoder.poll(output) {
                Ok(ready) => ready,
                Err(err) => {
                    let _send_error = output.send(Err(err));
                    state.failed = true;
                    false
                }
            };

        if ready
            && state
                .end_of_video
                .is_some_and(|last| state.sequence >= last)
        {
            decoder.end_of_video(output);
            state.end_of_video = None;
        }

        if ready && let Ok(next) = input.try_recv() {
            // A reset sent before this input is applied before decoding it.
            if !state.controls(decoder, control) {
                return;
            }
            state.sequence = next.sequence;
            if next.generation == state.generation {
                decoder.submit_chunk(should_stop, next.chunk, output);
            }
            continue;
        }

        // Wake for controls immediately and poll GPU completion at a bounded interval.
        let mut select = crossbeam::channel::Select::new();
        select.recv(&control.wake);
        if ready {
            select.recv(input);
        }
        let _ready = select.ready_timeout(Duration::from_millis(1));
    }
}
