use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

use crossbeam::channel::{unbounded, Receiver, Sender};

use super::{Chunk, Frame, OutputCallback, Result, SyncDecoder};

enum Command {
    Chunk(Chunk),
    Flush { on_done: Sender<()> },
    Reset,
    Stop,
}

#[derive(Clone)]
struct Comms {
    /// Set when it is time to die
    should_stop: Arc<AtomicBool>,

    /// Incremented on each call to [`AsyncDecoder::reset`].
    /// Decremented each time the decoder thread receives [`Command::Reset`].
    num_outstanding_resets: Arc<AtomicU64>,
}

impl Default for Comms {
    fn default() -> Self {
        Self {
            should_stop: Arc::new(AtomicBool::new(false)),
            num_outstanding_resets: Arc::new(AtomicU64::new(0)),
        }
    }
}

/// Runs a [`SyncDecoder`] in a background thread, for non-blocking video decoding.
pub struct AsyncDecoder {
    /// Where the decoding happens
    _thread: std::thread::JoinHandle<()>,

    /// Commands sent to the decoder thread.
    command_tx: Sender<Command>,

    /// Instant communication to the decoder thread (circumventing the command queue).
    comms: Comms,
}

impl AsyncDecoder {
    pub fn new(
        debug_name: String,
        mut sync_decoder: Box<dyn SyncDecoder + Send>,
        on_output: impl Fn(Result<Frame>) + Send + Sync + 'static,
    ) -> Self {
        re_tracing::profile_function!();

        let (command_tx, command_rx) = unbounded();
        let comms = Comms::default();

        let thread = std::thread::Builder::new()
            .name("av1_decoder".into())
            .spawn({
                let comms = comms.clone();
                move || {
                    econtext::econtext_data!("Video", debug_name.clone());

                    decoder_thread(sync_decoder.as_mut(), &comms, &command_rx, &on_output);
                    re_log::debug!("Closing decoder thread for {debug_name}");
                }
            })
            .expect("failed to spawn decoder thread");

        Self {
            _thread: thread,
            command_tx,
            comms,
        }
    }

    // NOTE: The interface is all `&mut self` to avoid certain types of races.
    pub fn decode(&mut self, chunk: Chunk) {
        re_tracing::profile_function!();
        self.command_tx.send(Command::Chunk(chunk)).ok();
    }

    /// Resets the decoder.
    ///
    /// This does not block, all chunks sent to `decode` before this point will be discarded.
    // NOTE: The interface is all `&mut self` to avoid certain types of races.
    pub fn reset(&mut self) {
        re_tracing::profile_function!();

        // Increment resets first…
        self.comms
            .num_outstanding_resets
            .fetch_add(1, Ordering::Release);

        // …so it is visible on the decoder thread when it gets the `Reset` command.
        self.command_tx.send(Command::Reset).ok();
    }

    /// Blocks until all pending frames have been decoded.
    // NOTE: The interface is all `&mut self` to avoid certain types of races.
    pub fn flush(&mut self) {
        re_tracing::profile_function!();
        let (tx, rx) = crossbeam::channel::bounded(0);
        self.command_tx.send(Command::Flush { on_done: tx }).ok();
        rx.recv().ok();
    }
}

impl Drop for AsyncDecoder {
    fn drop(&mut self) {
        re_tracing::profile_function!();

        // Set `should_stop` first…
        self.comms.should_stop.store(true, Ordering::Release);

        // …so it is visible on the decoder thread when it gets the `Stop` command.
        self.command_tx.send(Command::Stop).ok();

        // NOTE: we don't block here. The decoder thread will finish soon enough.
    }
}

fn decoder_thread(
    decoder: &mut dyn SyncDecoder,
    comms: &Comms,
    command_rx: &Receiver<Command>,
    on_output: &OutputCallback,
) {
    #![allow(clippy::debug_assert_with_mut_call)]

    while let Ok(command) = command_rx.recv() {
        if comms.should_stop.load(Ordering::Acquire) {
            re_log::debug!("Should stop");
            return;
        }

        // If we're waiting for a reset we should ignore all other commands until we receive it.
        let has_outstanding_reset = 0 < comms.num_outstanding_resets.load(Ordering::Acquire);

        match command {
            Command::Chunk(chunk) => {
                if !has_outstanding_reset {
                    decoder.submit_chunk(&comms.should_stop, chunk, on_output);
                }
            }
            Command::Flush { on_done } => {
                on_done.send(()).ok();
            }
            Command::Reset => {
                decoder.reset();
                comms.num_outstanding_resets.fetch_sub(1, Ordering::Release);
            }
            Command::Stop => {
                re_log::debug!("Stop");
                return;
            }
        }
    }

    re_log::debug!("Disconnected");
}
