//! The platform audio output device.
//!
//! On Linux, ALSA is loaded at runtime with `dlopen`, so binaries start on machines
//! without `libasound` and only audio playback is unavailable there.
//! On macOS we drive an `AudioQueue` ourselves.
//! Everywhere else `tinyaudio` provides the device.

cfg_select! {
    target_os = "linux" => {
        mod alsa;
        use alsa as backend;
    }
    target_os = "macos" => {
        mod audio_queue;
        use audio_queue as backend;
    }
    _ => {
        mod tinyaudio_backend;
        use tinyaudio_backend as backend;
    }
}

mod device;

pub use device::{DeviceStatus, OutputDevice};

/// Describes the stream the device should open.
#[derive(Clone, Copy, Debug)]
pub struct OutputDeviceParameters {
    pub sample_rate: u32,
    pub num_channels: usize,

    /// Frames per callback. More means higher latency and fewer glitches.
    pub frames_per_callback: usize,
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum OutputError {
    #[error("Failed to load the system audio library: {0}")]
    LibraryLoad(String),

    #[error("Audio device error: {0}")]
    Device(String),
}

/// Fills an interleaved `f32` buffer of exactly `frames_per_callback * num_channels` samples.
pub type FillCallback = Box<dyn FnMut(&OutputDeviceParameters, &mut [f32]) + Send + 'static>;

/// Reports that an output device stopped after an unrecoverable error.
type FailureCallback = Box<dyn FnOnce(OutputError) + Send + 'static>;
