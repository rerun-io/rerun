//! Audio output, fed by the [`Mixer`].

use std::sync::Arc;

use re_mutex::Mutex;

use crate::mixer::{Mixer, OUTPUT_CHANNELS, OUTPUT_SAMPLE_RATE};
use crate::output::{DeviceStatus, OutputDevice, OutputDeviceParameters, OutputError};
use crate::{AudioBuffer, StreamId, StreamRequest};

/// Frames the device asks for per callback.
///
/// The device calls back whenever it needs more samples, independent of the UI frame rate.
/// Natively that happens on a dedicated audio thread. On the web it happens on the main thread
/// with only two buffers queued, so a UI frame longer than one buffer leaves a gap in the audio.
/// A larger buffer tolerates slower frames, at the cost of latency.
const FRAMES_PER_CALLBACK: usize = if cfg!(target_arch = "wasm32") {
    4 * 1024
} else {
    2 * 1024
};

/// Mixes streams and plays them on the default output device.
///
/// This is an immediate-mode API: callers describe what should be audible by calling
/// [`Self::request`] for every stream that should be heard, then call
/// [`Self::commit_requests`] or [`Self::discard_requests`] at the end of the update.
/// A stream omitted from a committed update ramps down to silence within milliseconds and is dropped.
/// The player keeps its own cursor per stream, driven by the audio clock, and only
/// jumps when the requested position drifts too far from it.
///
/// The output device is opened lazily on the first request, because browsers only allow
/// audio output after a user gesture.
///
/// Each player opens its own output device, so an application should create one
/// and share it between everything that plays audio.
pub struct AudioPlayer {
    mixer: Arc<Mutex<Mixer>>,

    /// `None` until the first request.
    device: Mutex<Option<OutputDevice>>,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self {
            mixer: Arc::new(Mutex::new(Mixer::default())),
            device: Mutex::new(None),
        }
    }
}

impl AudioPlayer {
    /// Stage a stream for the current update.
    ///
    /// Call this once per update for as long as the stream should be heard, then call
    /// [`Self::commit_requests`] or [`Self::discard_requests`]. If the same stream is
    /// requested more than once in an update, the last request wins.
    ///
    /// Does nothing if there is no working output device, see [`Self::device_status`].
    pub fn request(&self, request: StreamRequest) {
        if self.ensure_device().is_ok() {
            self.mixer.lock().request(request);
        }
    }

    /// Stage a buffer to play from start to end once, at normal speed and full volume.
    ///
    /// The one-shot starts on [`Self::commit_requests`], replacing any stream with the same `id`,
    /// and is cancelled by [`Self::discard_requests`].
    ///
    /// Does nothing if there is no working output device, see [`Self::device_status`].
    pub fn play_once(&self, id: StreamId, buffer: Arc<AudioBuffer>) {
        if self.ensure_device().is_ok() {
            self.mixer.lock().play_once(id, buffer);
        }
    }

    /// Commit all staged requests as the complete desired set of persistent streams.
    pub fn commit_requests(&self) {
        self.mixer.lock().commit_requests();
    }

    /// Discard all requests staged by the current update.
    pub fn discard_requests(&self) {
        self.mixer.lock().discard_requests();
    }

    /// Whether the output device is working.
    ///
    /// `None` until the first request, and while the device is still opening.
    /// A device that stopped after an unrecoverable error is reported as failed,
    /// which is terminal for this player.
    pub fn device_status(&self) -> Option<Result<(), OutputError>> {
        match self.device.lock().as_ref()?.status() {
            DeviceStatus::Opening => None,
            DeviceStatus::Running => Some(Ok(())),
            DeviceStatus::Failed(err) => Some(Err(err)),
        }
    }

    /// Starts opening the output device on first use.
    ///
    /// Returns `Ok` while the device is opening or running, so callers can queue audio
    /// before it is up, and `Err` once it is known to have failed.
    fn ensure_device(&self) -> Result<(), OutputError> {
        let mut device = self.device.lock();
        let device = device.get_or_insert_with(|| {
            let params = OutputDeviceParameters {
                sample_rate: OUTPUT_SAMPLE_RATE,
                num_channels: OUTPUT_CHANNELS,
                frames_per_callback: FRAMES_PER_CALLBACK,
            };
            let mixer = self.mixer.clone();
            let fill = Box::new(move |_params: &OutputDeviceParameters, out: &mut [f32]| {
                mixer.lock().fill(out);
            });
            OutputDevice::open(params, fill)
        });
        match device.status() {
            DeviceStatus::Opening | DeviceStatus::Running => Ok(()),
            DeviceStatus::Failed(err) => Err(err),
        }
    }
}
