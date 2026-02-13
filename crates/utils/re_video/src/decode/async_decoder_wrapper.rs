use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::{AsyncDecoder, Chunk, Result};

#[cfg(with_dav1d)]
use crate::{VideoDataDescription, decode::FrameResult};

use crate::{Receiver, Sender};

enum Command {
    Chunk(Chunk),

    // Boxed, because `VideoDataDescription` is huge.
    Reset(Box<VideoDataDescription>),

    Stop,
}

impl re_byte_size::SizeBytes for Command {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::Chunk(chunk) => chunk.heap_size_bytes(),
            Self::Reset(video_data_description) => video_data_description.heap_size_bytes(),
            Self::Stop => 0,
        }
    }
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

/// Blocking decoder of video chunks.
#[cfg(with_dav1d)]
pub trait SyncDecoder {
    /// Submit some work and read the results.
    ///
    /// Stop early if `should_stop` is `true` or turns `true`.
    fn submit_chunk(
        &mut self,
        should_stop: &std::sync::atomic::AtomicBool,
        chunk: Chunk,
        output_sender: &Sender<FrameResult>,
    );

    /// Clear and reset everything
    fn reset(&mut self, video_data_description: &crate::VideoDataDescription);
}

/// Runs a [`SyncDecoder`] in a background thread, for non-blocking video decoding.
pub struct AsyncDecoderWrapper {
    /// Where the decoding happens
    _thread: std::thread::JoinHandle<()>,

    /// Commands sent to the decoder thread.
    command_tx: Sender<Command>,

    /// Instant communication to the decoder thread (circumventing the command queue).
    comms: Comms,
}

impl AsyncDecoderWrapper {
    pub fn new(
        debug_name: String,
        mut sync_decoder: Box<dyn SyncDecoder + Send>,
        output_sender: Sender<FrameResult>,
    ) -> Self {
        re_tracing::profile_function!();

        let (command_tx, command_rx) = crate::channel(format!("{debug_name}-channel"));
        let comms = Comms::default();

        let thread = std::thread::Builder::new()
            .name(format!("decoder of {debug_name}"))
            .spawn({
                let comms = comms.clone();
                move || {
                    econtext::econtext_data!("Video", debug_name.clone());

                    decoder_thread(sync_decoder.as_mut(), &comms, &command_rx, &output_sender);
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
}

impl AsyncDecoder for AsyncDecoderWrapper {
    // NOTE: The interface is all `&mut self` to avoid certain types of races.
    fn submit_chunk(&mut self, chunk: Chunk) -> Result<()> {
        re_tracing::profile_function!();
        self.command_tx.send(Command::Chunk(chunk)).ok();

        Ok(())
    }

    /// Resets the decoder.
    ///
    /// This does not block, all chunks sent to `decode` before this point will be discarded.
    // NOTE: The interface is all `&mut self` to avoid certain types of races.
    fn reset(&mut self, video_data_description: &VideoDataDescription) -> Result<()> {
        re_tracing::profile_function!();

        // Increment resets first…
        self.comms
            .num_outstanding_resets
            .fetch_add(1, Ordering::Release);

        // …so it is visible on the decoder thread when it gets the `Reset` command.
        self.command_tx
            .send(Command::Reset(Box::new(video_data_description.clone())))
            .ok();

        Ok(())
    }
}

impl Drop for AsyncDecoderWrapper {
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
    output_sender: &Sender<FrameResult>,
) {
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
                    decoder.submit_chunk(&comms.should_stop, chunk, output_sender);
                }
            }
            Command::Reset(video_data_description) => {
                decoder.reset(&video_data_description);
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
